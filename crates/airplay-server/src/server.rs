//! AirPlay 服务器入口 —— 聚合 ControlServer + Bonjour。

use std::sync::Arc;

use anyhow::Result;
use tokio::signal;

use crate::bonjour::Bonjour;
use crate::config::AirPlayConfig;
use crate::consumer::AirPlayConsumer;
use crate::control::server::ControlServer;

/// AirPlay 服务器。
///
/// 聚合 ControlServer（RTSP/HTTP 控制通道）和 Bonjour（mDNS 服务注册）。
pub struct AirPlayServer {
    config: Arc<AirPlayConfig>,
    control_server: ControlServer,
    bonjour: Bonjour,
}

impl AirPlayServer {
    /// 创建 AirPlay 服务器。
    pub fn new(config: AirPlayConfig, consumer: Arc<dyn AirPlayConsumer>) -> Self {
        let config = Arc::new(config);
        let bonjour = Bonjour::new(
            config.server_name.clone(),
            config.device_id.clone(),
        );
        let control_server = ControlServer::new(config.clone(), consumer);
        Self {
            config,
            control_server,
            bonjour,
        }
    }

    /// 启动 AirPlay 服务器。
    ///
    /// 1. 启动 ControlServer（TCP 绑定，获取端口）
    /// 2. 用端口启动 Bonjour（mDNS 注册 `_airplay._tcp` + `_raop._tcp`）
    /// 3. 等待 Ctrl+C 信号
    ///
    /// 返回 ControlServer 的监听端口。
    pub async fn start(&mut self) -> Result<u16> {
        let port = self.control_server.start().await?;
        tracing::info!("control server listening on port {}", port);

        self.bonjour.start(port)?;
        tracing::info!("bonjour service registered: {} (port {})", self.config.server_name, port);

        Ok(port)
    }

    /// 等待停止信号（Ctrl+C）。
    pub async fn wait_for_shutdown(&self) {
        match signal::ctrl_c().await {
            Ok(()) => tracing::info!("received Ctrl+C, shutting down"),
            Err(e) => tracing::warn!("failed to listen for Ctrl+C: {}", e),
        }
    }

    /// 停止 AirPlay 服务器。
    pub fn stop(&mut self) {
        self.bonjour.stop();
        self.control_server.stop();
        tracing::info!("airplay server stopped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use airplay_protocol::stream_info::{AudioStreamInfo, VideoStreamInfo};
    use crate::consumer::PlaybackInfo;

    /// 空实现的消费者，用于测试。
    struct DummyConsumer;

    #[async_trait]
    impl AirPlayConsumer for DummyConsumer {
        async fn on_video_format(&self, _: VideoStreamInfo) {}
        async fn on_video(&self, _: &[u8]) {}
        async fn on_video_src_disconnect(&self) {}
        async fn on_audio_format(&self, _: AudioStreamInfo) {}
        async fn on_audio(&self, _: &[u8]) {}
        async fn on_audio_src_disconnect(&self) {}
        async fn playback_info(&self) -> PlaybackInfo {
            PlaybackInfo::default()
        }
    }

    #[test]
    fn test_airplay_server_new() {
        let config = AirPlayConfig::default();
        let consumer = Arc::new(DummyConsumer) as Arc<dyn AirPlayConsumer>;
        let server = AirPlayServer::new(config, consumer);
        assert_eq!(server.config.server_name, "airplay-rs");
        assert_eq!(server.control_server.port(), 0);
    }
}
