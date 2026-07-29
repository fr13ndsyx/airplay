//! 实时播放示例 —— 启动 AirPlay 服务器，GStreamer 实时播放音视频。
//!
//! 用法：
//! ```sh
//! cargo run -p airplay-player --example play
//! ```
//!
//! 启动后：
//! 1. 在 0.0.0.0:0（随机端口）监听 ControlServer
//! 2. 通过 mDNS 广播 `_airplay._tcp` + `_raop._tcp` 服务
//! 3. iPhone 在同一 Wi-Fi 下选择 "airplay-rs" 设备镜像
//! 4. GStreamer 自动弹出视频窗口 + 播放音频
//! 5. 按 Ctrl+C 停止
//!
//! 对比 Phase 2 的 `dump.rs`：
//! - 不再写文件，直接实时播放
//! - 视频使用 D3D11 硬件解码（零拷贝）
//! - 音频支持 ALAC / AAC-ELD

use std::sync::Arc;

use airplay_player::GstPlayer;
use airplay_server::config::AirPlayConfig;
use airplay_server::consumer::AirPlayConsumer;
use airplay_server::server::AirPlayServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("airplay-rs play example starting...");

    // 创建 GStreamer 播放器（内部启动专用 GStreamer 线程）
    let player = GstPlayer::new()?;
    tracing::info!("GStreamer 播放器已创建（3 条管线：视频 / ALAC / AAC-ELD）");

    // 创建消费者，桥接 AirPlayConsumer → GstPlayer
    let consumer = player.consumer();
    let consumer = Arc::new(consumer) as Arc<dyn AirPlayConsumer>;

    // 创建并启动服务器
    let config = AirPlayConfig::default();
    let mut server = AirPlayServer::new(config, consumer);

    let port = server.start().await?;
    tracing::info!("========================================");
    tracing::info!(" AirPlay server ready!");
    tracing::info!(" Server name: airplay-rs");
    tracing::info!(" Control port: {}", port);
    tracing::info!(" Video: D3D11 硬解 + d3d11videosink");
    tracing::info!(" Audio: ALAC / AAC-ELD → autoaudiosink");
    tracing::info!("========================================");
    tracing::info!("Connect your iPhone to the same Wi-Fi,");
    tracing::info!("open Screen Mirroring and select 'airplay-rs'.");
    tracing::info!("Press Ctrl+C to stop.");

    // 等待 Ctrl+C
    server.wait_for_shutdown().await;

    // 停止服务器
    server.stop();
    tracing::info!("服务器已停止，GStreamer 管线正在清理...");

    // player 在此处 drop，触发 GstPlayer::shutdown()
    // → 发送 Shutdown 命令 → GStreamer 线程退出 → 所有管线 set_state(Null)

    Ok(())
}
