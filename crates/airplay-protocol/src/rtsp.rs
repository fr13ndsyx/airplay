#![allow(clippy::pedantic)]

use crate::stream_info::{
    AudioFormat, AudioStreamInfo, CompressionType, MediaStreamInfo, VideoStreamInfo,
};
use std::io::Cursor;
use plist::{Dictionary, Value};
use thiserror::Error;
use tracing::{error, warn};

/// RTSP SETUP / TEARDOWN 请求解析错误。
#[derive(Debug, Error)]
pub enum RtspError {
    #[error("plist parse error: {0}")]
    Plist(#[from] plist::Error),
    #[error("invalid plist structure: expected dictionary")]
    NotDictionary,
    #[error("invalid stream structure: expected array")]
    NotArray,
    #[error("invalid stream entry: expected dictionary")]
    StreamNotDictionary,
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    #[error("invalid field type for: {0}")]
    InvalidFieldType(&'static str),
}

/// RTSP 协议处理器。
pub struct RTSP {
    ekey: Option<Vec<u8>>,
    eiv: Option<Vec<u8>>,
    stream_connection_id: Option<String>,
}

impl RTSP {
    pub fn new() -> Self {
        Self {
            ekey: None,
            eiv: None,
            stream_connection_id: None,
        }
    }

    /// 解析 RTSP SETUP 请求的 plist 负载。
    pub fn setup(&mut self, payload: &[u8]) -> Result<Option<MediaStreamInfo>, RtspError> {
        let value = Value::from_reader(Cursor::new(payload))?;
        let dict = value.as_dictionary().ok_or(RtspError::NotDictionary)?;

        if dict.contains_key("ekey") || dict.contains_key("eiv") {
            self.ekey = dict
                .get("ekey")
                .and_then(|v| v.as_data())
                .map(|d| d.to_vec());
            self.eiv = dict
                .get("eiv")
                .and_then(|v| v.as_data())
                .map(|d| d.to_vec());
            return Ok(None);
        }

        if dict.contains_key("streams") {
            return Ok(self.get_media_stream_info(dict)?);
        }

        Ok(None)
    }

    /// 解析 RTSP TEARDOWN 请求的 plist 负载。
    pub fn teardown(&mut self, payload: &[u8]) -> Result<Option<MediaStreamInfo>, RtspError> {
        let value = Value::from_reader(Cursor::new(payload))?;
        let dict = value.as_dictionary().ok_or(RtspError::NotDictionary)?;
        if dict.contains_key("streams") {
            return Ok(self.get_media_stream_info(dict)?);
        }
        Ok(None)
    }

    /// 从 plist 字典中提取媒体流信息。
    fn get_media_stream_info(
        &mut self,
        dict: &Dictionary,
    ) -> Result<Option<MediaStreamInfo>, RtspError> {
        let streams = dict
            .get("streams")
            .ok_or(RtspError::MissingField("streams"))?
            .as_array()
            .ok_or(RtspError::NotArray)?;

        if streams.len() > 1 {
            warn!("Request contains more than one stream info");
        }

        let stream = streams
            .first()
            .ok_or(RtspError::MissingField("streams[0]"))?
            .as_dictionary()
            .ok_or(RtspError::StreamNotDictionary)?;

        let type_val = stream
            .get("type")
            .ok_or(RtspError::MissingField("type"))?
            .as_signed_integer()
            .ok_or(RtspError::InvalidFieldType("type"))?;

        match type_val as i32 {
            110 => {
                // video stream
                // 无符号长整型转字符串
                // 实测不同 iOS 版本会发送不同类型：NSNumber 或 NSString。
                // 这里兼容三种类型：String / 有符号整数 / 无符号整数。
                if let Some(v) = stream.get("streamConnectionID") {
                    let conn_id = if let Some(s) = v.as_string() {
                        s.to_string()
                    } else if let Some(u) = v.as_unsigned_integer() {
                        u.to_string()
                    } else if let Some(i) = v.as_signed_integer() {
                        // 负值会被当作无符号处理
                        (i as u64).to_string()
                    } else {
                        return Err(RtspError::InvalidFieldType("streamConnectionID"));
                    };
                    self.stream_connection_id = Some(conn_id);
                }
                let conn_id = self.stream_connection_id.clone().unwrap_or_default();
                Ok(Some(MediaStreamInfo::Video(VideoStreamInfo {
                    stream_connection_id: conn_id,
                })))
            }
            96 => {
                // audio stream
                let mut builder = AudioStreamInfo::builder();
                if let Some(ct_val) = stream.get("ct") {
                    if let Some(ct_code) = ct_val.as_signed_integer() {
                        if let Some(ct) = CompressionType::from_code(ct_code as u64) {
                            builder = builder.compression_type(ct);
                        }
                    }
                }
                if let Some(af_val) = stream.get("audioFormat") {
                    // audioFormatCode 从 stream 中读取
                    // 先按 int 读取再符号扩展为 long。这里按有符号整数读取后转 u64，
                    // 对非负值保持位模式不变（code 最大 0x100000000，可由 i64 表示）。
                    if let Some(af_code) = af_val.as_signed_integer() {
                        if let Some(af) = AudioFormat::from_code(af_code as u64) {
                            builder = builder.audio_format(af);
                        }
                    }
                }
                if let Some(spf_val) = stream.get("spf") {
                    if let Some(spf) = spf_val.as_signed_integer() {
                        builder = builder.samples_per_frame(spf as i32);
                    }
                }
                Ok(Some(MediaStreamInfo::Audio(builder.build())))
            }
            other => {
                error!("Unknown stream type: {}", other);
                Ok(None)
            }
        }
    }

    pub fn stream_connection_id(&self) -> Option<&str> {
        self.stream_connection_id.as_deref()
    }

    pub fn ekey(&self) -> Option<&[u8]> {
        self.ekey.as_deref()
    }

    pub fn eiv(&self) -> Option<&[u8]> {
        self.eiv.as_deref()
    }
}

impl Default for RTSP {
    fn default() -> Self {
        Self::new()
    }
}
