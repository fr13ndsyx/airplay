//! 带状态广播的 Consumer 包装器。
//!
//! 包装 `GstPlayerConsumer`，在 iPhone 连接/断开时广播 `AppStatus` 给 UI。

use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use airplay_protocol::stream_info::{AudioStreamInfo, VideoStreamInfo};
use airplay_server::consumer::{AirPlayConsumer, PlaybackInfo};

use crate::status::{AppStatus, StatusTx};

/// 带状态广播的 Consumer。
///
/// 包装内部 consumer：
/// - `on_video_format` → 广播 `Connected`
/// - `on_video_src_disconnect` → 广播 `Disconnected { port }`
///
/// port 通过 `Arc<AtomicU16>` 共享，server 启动后更新。
pub struct StatusConsumer {
    inner: Arc<dyn AirPlayConsumer>,
    status_tx: StatusTx,
    port: Arc<AtomicU16>,
}

impl StatusConsumer {
    /// 创建包装器（port 初始为 0，server 启动后通过 `set_port` 更新）。
    pub fn new(inner: Arc<dyn AirPlayConsumer>, status_tx: StatusTx) -> Self {
        Self {
            inner,
            status_tx,
            port: Arc::new(AtomicU16::new(0)),
        }
    }

    /// 获取 port 的共享引用（用于 server 启动后更新）。
    pub fn port_handle(&self) -> Arc<AtomicU16> {
        Arc::clone(&self.port)
    }

    /// 设置端口（server 启动后调用）。
    pub fn set_port(&self, port: u16) {
        self.port.store(port, Ordering::SeqCst);
    }
}

#[async_trait]
impl AirPlayConsumer for StatusConsumer {
    async fn on_video_format(&self, video_stream_info: VideoStreamInfo) {
        tracing::info!("iPhone 已连接，开始投屏");
        let _ = self.status_tx.send(AppStatus::Connected);
        self.inner.on_video_format(video_stream_info).await;
    }

    async fn on_video(&self, bytes: &[u8]) {
        self.inner.on_video(bytes).await;
    }

    async fn on_video_src_disconnect(&self) {
        tracing::info!("iPhone 断开连接");
        let port = self.port.load(Ordering::SeqCst);
        let _ = self.status_tx.send(AppStatus::Disconnected { port });
        self.inner.on_video_src_disconnect().await;
    }

    async fn on_audio_format(&self, audio_stream_info: AudioStreamInfo) {
        self.inner.on_audio_format(audio_stream_info).await;
    }

    async fn on_audio(&self, bytes: &[u8]) {
        self.inner.on_audio(bytes).await;
    }

    async fn on_audio_src_disconnect(&self) {
        self.inner.on_audio_src_disconnect().await;
    }

    async fn playback_info(&self) -> PlaybackInfo {
        self.inner.playback_info().await
    }
}
