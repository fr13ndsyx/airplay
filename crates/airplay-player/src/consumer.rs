//! `AirPlayConsumer` 实现 —— 将 AirPlay 音视频数据转发给 GStreamer。
//!
//! `GstPlayerConsumer` 持有 `mpsc::Sender` 的克隆（包裹在 `Mutex` 中以保证 `Sync`），
//! 实现 `AirPlayConsumer` trait 后可直接传给 `AirPlayServer`。
//!
//! GstPlayer 的 AirPlayConsumer 实现。

use std::sync::{mpsc, Mutex};

use async_trait::async_trait;

use airplay_protocol::stream_info::{AudioStreamInfo, VideoStreamInfo};
use airplay_server::consumer::{AirPlayConsumer, PlaybackInfo};

use crate::player::Command;

/// GStreamer 消费者 —— 桥接 `AirPlayConsumer` trait 和 `GstPlayer`。
///
/// 通过 `GstPlayer::consumer()` 创建，克隆了 sender 用于异步推送数据。
/// `Mutex<Sender>` 保证 `Send + Sync`（`mpsc::Sender` 本身是 `Send` 但非 `Sync`）。
pub struct GstPlayerConsumer {
    tx: Mutex<mpsc::Sender<Command>>,
    frame_count: std::sync::atomic::AtomicU64,
    dump_file: Mutex<Option<std::fs::File>>,
}

impl GstPlayerConsumer {
    /// 从 sender 创建消费者（由 `GstPlayer::consumer()` 调用）。
    pub(crate) fn new(tx: mpsc::Sender<Command>) -> Self {
        // 尝试创建 dump 文件
        let dump_file = Mutex::new(
            std::fs::File::create("dump.h264").ok()
        );
        Self {
            tx: Mutex::new(tx),
            frame_count: std::sync::atomic::AtomicU64::new(0),
            dump_file,
        }
    }

    /// 发送命令（内部辅助方法）。
    fn send(&self, cmd: Command) {
        if let Err(e) = self.tx.lock().expect("tx mutex poisoned").send(cmd) {
            tracing::error!("GStreamer 命令发送失败（线程已退出？）: {}", e);
        }
    }
}

#[async_trait]
impl AirPlayConsumer for GstPlayerConsumer {
    async fn on_video_format(&self, _video_stream_info: VideoStreamInfo) {
        tracing::info!("on_video_format: {:?}", _video_stream_info);
        self.send(Command::StartVideo);
    }

    async fn on_video(&self, bytes: &[u8]) {
        let count = self.frame_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // 前 5 帧打印详细信息
        if count < 5 {
            let preview: Vec<u8> = bytes.iter().take(16).cloned().collect();
            tracing::info!(
                "on_video 帧#{}: 大小={} 字节, 前16字节: {:02X?}",
                count,
                bytes.len(),
                preview
            );
        }
        // dump 到文件
        if let Ok(mut file) = self.dump_file.lock() {
            if let Some(ref mut f) = *file {
                use std::io::Write;
                let _ = f.write_all(bytes);
            }
        }
        self.send(Command::VideoData(bytes.to_vec()));
    }

    async fn on_video_src_disconnect(&self) {
        tracing::info!("视频源断开");
        self.frame_count.store(0, std::sync::atomic::Ordering::Relaxed);
        self.send(Command::StopVideo);
    }

    async fn on_audio_format(&self, audio_stream_info: AudioStreamInfo) {
        tracing::info!("on_audio_format: {:?}", audio_stream_info);
        match audio_stream_info.compression_type {
            Some(ct) => self.send(Command::StartAudio(ct)),
            None => tracing::warn!("音频格式缺少 compression_type"),
        }
    }

    async fn on_audio(&self, bytes: &[u8]) {
        self.send(Command::AudioData(bytes.to_vec()));
    }

    async fn on_audio_src_disconnect(&self) {
        tracing::info!("音频源断开");
        self.send(Command::StopAudio);
    }

    async fn playback_info(&self) -> PlaybackInfo {
        // MVP 阶段不支持 HLS / 进度查询
        PlaybackInfo::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GstPlayer;

    #[tokio::test]
    async fn test_consumer_video_flow() {
        let player = GstPlayer::new().expect("create player");
        let consumer = player.consumer();

        // on_video_format → StartVideo
        let video_info = VideoStreamInfo::new("test-conn-id".into());
        consumer.on_video_format(video_info).await;

        // on_video → push data
        let fake_h264 = [0x00, 0x00, 0x00, 0x01, 0x67, 0x42];
        consumer.on_video(&fake_h264).await;

        // 给 GStreamer 线程时间处理
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // on_video_src_disconnect → StopVideo
        consumer.on_video_src_disconnect().await;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn test_consumer_audio_flow() {
        let player = GstPlayer::new().expect("create player");
        let consumer = player.consumer();

        // on_audio_format → StartAudio(ALAC)
        let audio_info = airplay_protocol::stream_info::AudioStreamInfo::builder()
            .compression_type(
                airplay_protocol::stream_info::CompressionType::Alac,
            )
            .build();
        consumer.on_audio_format(audio_info).await;

        // on_audio → push data
        let fake_audio = [0x00u8; 16];
        consumer.on_audio(&fake_audio).await;

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // on_audio_src_disconnect → StopAudio
        consumer.on_audio_src_disconnect().await;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn test_consumer_playback_info() {
        let player = GstPlayer::new().expect("create player");
        let consumer = player.consumer();

        let info = consumer.playback_info().await;
        assert_eq!(info.duration, 0.0);
        assert_eq!(info.position, 0.0);
    }
}
