//! H264 转储消费者 —— 将接收到的视频 NAL 单元写入 .h264 文件。
//!
//! 用于 MVP 验证：启动 AirPlayServer，iPhone 连接镜像后生成 dump.h264 文件，
//! 可用 ffplay 播放验证。

use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::consumer::{AirPlayConsumer, PlaybackInfo};
use airplay_protocol::stream_info::{AudioStreamInfo, VideoStreamInfo};

/// 将视频 NAL 单元写入文件的消费者。
///
/// 其他回调（音频、HLS）均为空实现，仅用于 MVP 验证视频通路。
pub struct H264DumpConsumer {
    file: Arc<Mutex<File>>,
}

impl H264DumpConsumer {
    /// 创建消费者，打开（或覆盖）指定路径的文件。
    ///
    /// # Errors
    /// 文件创建/打开失败时返回 `std::io::Error`。
    pub fn new(path: &str) -> std::io::Result<Self> {
        let file = File::create(path)?;
        Ok(Self {
            file: Arc::new(Mutex::new(file)),
        })
    }
}

#[async_trait]
impl AirPlayConsumer for H264DumpConsumer {
    async fn on_video_format(&self, _video_stream_info: VideoStreamInfo) {
        tracing::info!("video format received: {:?}", _video_stream_info);
    }

    async fn on_video(&self, bytes: &[u8]) {
        if let Ok(mut file) = self.file.lock() {
            if let Err(e) = file.write_all(bytes) {
                tracing::warn!("failed to write video data: {}", e);
            }
        }
    }

    async fn on_video_src_disconnect(&self) {
        tracing::info!("video source disconnected");
    }

    async fn on_audio_format(&self, _audio_stream_info: AudioStreamInfo) {
        tracing::info!("audio format received: {:?}", _audio_stream_info);
    }

    async fn on_audio(&self, _bytes: &[u8]) {
        // MVP 阶段不处理音频
    }

    async fn on_audio_src_disconnect(&self) {
        tracing::info!("audio source disconnected");
    }

    async fn playback_info(&self) -> PlaybackInfo {
        PlaybackInfo::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[tokio::test]
    async fn test_h264_dump_writes_video_data() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_h264_dump.h264");
        let path_str = path.to_str().unwrap();

        let consumer = H264DumpConsumer::new(path_str).unwrap();

        // 写入测试数据（模拟 Annex-B NAL 单元）
        let nal_unit = [0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x0a];
        consumer.on_video(&nal_unit).await;

        let nal_unit2 = [0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x80, 0x40];
        consumer.on_video(&nal_unit2).await;

        // 验证文件内容
        let mut file = File::open(path_str).unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();

        assert_eq!(buffer.len(), 16);
        assert_eq!(&buffer[..4], &[0x00, 0x00, 0x00, 0x01]);
        assert_eq!(&buffer[4..8], &[0x67, 0x42, 0x00, 0x0a]);
        assert_eq!(&buffer[8..12], &[0x00, 0x00, 0x00, 0x01]);

        // 清理
        std::fs::remove_file(path_str).ok();
    }

    #[tokio::test]
    async fn test_h264_dump_other_methods_no_panic() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_h264_dump_empty.h264");
        let path_str = path.to_str().unwrap();

        let consumer = H264DumpConsumer::new(path_str).unwrap();

        // 这些方法不应 panic
        consumer
            .on_video_format(VideoStreamInfo::new("test".to_string()))
            .await;
        consumer.on_video_src_disconnect().await;
        consumer
            .on_audio_format(AudioStreamInfo::builder().build())
            .await;
        consumer.on_audio(&[0x01, 0x02, 0x03]).await;
        consumer.on_audio_src_disconnect().await;
        let pb = consumer.playback_info().await;
        assert_eq!(pb.duration, 0.0);
        assert_eq!(pb.position, 0.0);

        std::fs::remove_file(path_str).ok();
    }
}
