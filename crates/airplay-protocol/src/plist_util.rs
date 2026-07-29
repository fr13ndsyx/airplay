#![allow(clippy::pedantic)]

use plist::{Dictionary, Value};
use thiserror::Error;

/// 仅声明当前接收器实际支持的屏幕镜像能力。
///
/// 从原 Apple TV 能力 `0x1E5A7FFFF7` 中移除 Video、Photo、HLS 与 Slideshow
/// 四个低位能力，保留 FairPlay、Screen、ScreenRotate 以及音频相关高位能力。
/// 这样 iPhone 相册不会切换到"扩展播放"模式发送 1920×1080 横向画布，
/// 而是保持纯镜像，手机上怎么显示，接收端就怎么显示。
pub const MIRRORING_FEATURES: i64 = 0x1E5A7FFFC4;

/// Bonjour TXT 记录使用的双段十六进制能力值。
pub const MIRRORING_FEATURES_TXT: &str = "0x5A7FFFC4,0x1E";

/// 旧版 `/server-info` 使用的低 32 位镜像能力：FairPlay + Screen。
pub const LEGACY_MIRRORING_FEATURES: i64 = 0x44;

#[derive(Debug, Error)]
pub enum PlistError {
    #[error("plist serialization error: {0}")]
    Serialize(#[from] plist::Error),
}

/// AirPlay 配置
#[derive(Debug, Clone)]
pub struct AirPlayConfig {
    pub width: i32,
    pub height: i32,
    pub fps: f32,
}

/// 播放信息
#[derive(Debug, Clone)]
pub struct PlaybackInfo {
    pub duration: f64,
    pub position: f64,
}

fn int_val(n: i64) -> Value {
    Value::Integer(n.into())
}

fn real_val(n: f64) -> Value {
    Value::Real(n)
}

fn str_val(s: &str) -> Value {
    Value::String(s.to_string())
}

fn bool_val(b: bool) -> Value {
    Value::Boolean(b)
}

fn dict_val(d: Dictionary) -> Value {
    Value::Dictionary(d)
}

fn arr_val(v: Vec<Value>) -> Value {
    Value::Array(v)
}

pub fn prepare_info_response(config: &AirPlayConfig) -> Result<Vec<u8>, PlistError> {
    let mut audio_format_100 = Dictionary::new();
    audio_format_100.insert("audioInputFormats".to_string(), int_val(67108860));
    audio_format_100.insert("audioOutputFormats".to_string(), int_val(67108860));
    audio_format_100.insert("type".to_string(), int_val(100));

    let mut audio_format_101 = Dictionary::new();
    audio_format_101.insert("audioInputFormats".to_string(), int_val(67108860));
    audio_format_101.insert("audioOutputFormats".to_string(), int_val(67108860));
    audio_format_101.insert("type".to_string(), int_val(101));

    let audio_formats = arr_val(vec![dict_val(audio_format_100), dict_val(audio_format_101)]);

    let mut audio_latency_100 = Dictionary::new();
    audio_latency_100.insert("audioType".to_string(), str_val("default"));
    audio_latency_100.insert("inputLatencyMicros".to_string(), bool_val(false));
    audio_latency_100.insert("type".to_string(), int_val(100));

    let mut audio_latency_101 = Dictionary::new();
    audio_latency_101.insert("audioType".to_string(), str_val("default"));
    audio_latency_101.insert("inputLatencyMicros".to_string(), bool_val(false));
    audio_latency_101.insert("type".to_string(), int_val(101));

    let audio_latencies = arr_val(vec![dict_val(audio_latency_100), dict_val(audio_latency_101)]);

    let mut display = Dictionary::new();
    display.insert("features".to_string(), int_val(14));
    display.insert("height".to_string(), int_val(config.height as i64));
    display.insert("heightPhysical".to_string(), bool_val(false));
    display.insert("heightPixels".to_string(), int_val(config.height as i64));
    display.insert("maxFPS".to_string(), real_val(config.fps as f64));
    display.insert("overscanned".to_string(), bool_val(false));
    display.insert("refreshRate".to_string(), int_val(60));
    display.insert("rotation".to_string(), bool_val(false));
    display.insert("uuid".to_string(), str_val("e5f7a68d-7b0f-4305-984b-974f677a150b"));
    display.insert("width".to_string(), int_val(config.width as i64));
    display.insert("widthPhysical".to_string(), bool_val(false));
    display.insert("widthPixels".to_string(), int_val(config.width as i64));

    let displays = arr_val(vec![dict_val(display)]);

    let mut response = Dictionary::new();
    response.insert("audioFormats".to_string(), audio_formats);
    response.insert("audioLatencies".to_string(), audio_latencies);
    response.insert("displays".to_string(), displays);
    response.insert("features".to_string(), int_val(MIRRORING_FEATURES));
    response.insert("keepAliveSendStatsAsBody".to_string(), int_val(1));
    response.insert("model".to_string(), str_val("AppleTV3,2"));
    response.insert("name".to_string(), str_val("Apple TV"));
    response.insert("pi".to_string(), str_val("b08f5a79-db29-4384-b456-a4784d9e6055"));
    response.insert("sourceVersion".to_string(), str_val("220.68"));
    response.insert("statusFlags".to_string(), int_val(68));
    response.insert("vv".to_string(), int_val(2));

    let mut buf = Vec::new();
    plist::to_writer_binary(&mut buf, &Value::Dictionary(response))?;
    Ok(buf)
}

pub fn prepare_setup_audio_response(data_port: i32, control_port: i32) -> Result<Vec<u8>, PlistError> {
    let mut data_stream = Dictionary::new();
    data_stream.insert("dataPort".to_string(), int_val(data_port as i64));
    data_stream.insert("type".to_string(), int_val(96));
    data_stream.insert("controlPort".to_string(), int_val(control_port as i64));

    let streams = arr_val(vec![dict_val(data_stream)]);

    let mut response = Dictionary::new();
    response.insert("streams".to_string(), streams);

    let mut buf = Vec::new();
    plist::to_writer_binary(&mut buf, &Value::Dictionary(response))?;
    Ok(buf)
}

pub fn prepare_setup_video_response(data_port: i32, event_port: i32, timing_port: i32) -> Result<Vec<u8>, PlistError> {
    let mut data_stream = Dictionary::new();
    data_stream.insert("dataPort".to_string(), int_val(data_port as i64));
    data_stream.insert("type".to_string(), int_val(110));

    let streams = arr_val(vec![dict_val(data_stream)]);

    let mut response = Dictionary::new();
    response.insert("streams".to_string(), streams);
    response.insert("eventPort".to_string(), int_val(event_port as i64));
    response.insert("timingPort".to_string(), int_val(timing_port as i64));

    let mut buf = Vec::new();
    plist::to_writer_binary(&mut buf, &Value::Dictionary(response))?;
    Ok(buf)
}

pub fn prepare_server_info_response() -> Vec<u8> {
    let mut response = Dictionary::new();
    response.insert("features".to_string(), int_val(LEGACY_MIRRORING_FEATURES));
    response.insert("protovers".to_string(), real_val(1.0));
    response.insert("srcvers".to_string(), real_val(101.28));

    let mut buf = Vec::new();
    plist::to_writer_xml(&mut buf, &Value::Dictionary(response))
        .expect("failed to serialize server info plist to XML");
    buf
}

pub fn prepare_playback_info_response(playback_info: &PlaybackInfo) -> Vec<u8> {
    let mut loaded_time_ranges = Dictionary::new();
    loaded_time_ranges.insert("duration".to_string(), real_val(playback_info.duration));
    loaded_time_ranges.insert("start".to_string(), real_val(0.0));

    let mut seekable_time_ranges = Dictionary::new();
    seekable_time_ranges.insert("duration".to_string(), real_val(playback_info.duration));
    seekable_time_ranges.insert("start".to_string(), real_val(0.0));

    let mut response = Dictionary::new();
    response.insert("duration".to_string(), real_val(playback_info.duration));
    response.insert("loadedTimeRanges".to_string(), arr_val(vec![dict_val(loaded_time_ranges)]));
    response.insert("playbackBufferEmpty".to_string(), bool_val(true));
    response.insert("playbackBufferFull".to_string(), bool_val(false));
    response.insert("playbackLikelyToKeepUp".to_string(), bool_val(true));
    response.insert("position".to_string(), real_val(playback_info.position));
    response.insert("rate".to_string(), int_val(1));
    response.insert("readyToPlay".to_string(), bool_val(true));
    response.insert("seekableTimeRanges".to_string(), arr_val(vec![dict_val(seekable_time_ranges)]));

    let mut buf = Vec::new();
    plist::to_writer_xml(&mut buf, &Value::Dictionary(response))
        .expect("failed to serialize playback info plist to XML");
    buf
}

pub fn prepare_event_request(session_id: &str, list_uri: &str) -> Vec<u8> {
    let mut headers = Dictionary::new();
    headers.insert("X-Playback-Session-Id".to_string(), str_val(session_id));

    let mut request = Dictionary::new();
    request.insert("FCUP_Response_ClientInfo".to_string(), int_val(0));
    request.insert("FCUP_Response_ClientRef".to_string(), int_val(0));
    request.insert("FCUP_Response_Headers".to_string(), dict_val(headers));
    request.insert("FCUP_Response_RequestID".to_string(), int_val(0));
    request.insert("FCUP_Response_URL".to_string(), str_val(list_uri));
    request.insert("sessionID".to_string(), int_val(1));

    let mut wrapper = Dictionary::new();
    wrapper.insert("request".to_string(), dict_val(request));
    wrapper.insert("sessionID".to_string(), int_val(1));
    wrapper.insert("type".to_string(), str_val("unhandledURLRequest"));

    let mut buf = Vec::new();
    plist::to_writer_xml(&mut buf, &Value::Dictionary(wrapper))
        .expect("failed to serialize event request plist to XML");
    buf
}
