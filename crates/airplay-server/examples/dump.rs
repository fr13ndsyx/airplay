//! MVP 闭环示例 —— 启动 AirPlay 服务器，将视频 NAL 写入 dump.h264。
//!
//! 用法：
//! ```sh
//! cargo run --example dump
//! ```
//!
//! 启动后：
//! 1. 在 0.0.0.0:0（随机端口）监听 ControlServer
//! 2. 通过 mDNS 广播 `_airplay._tcp` + `_raop._tcp` 服务
//! 3. iPhone 在同一 Wi-Fi 下选择 "airplay-rs" 设备镜像
//! 4. 视频数据将写入当前目录的 `dump.h264`
//! 5. 按 Ctrl+C 停止
//!
//! 验证：
//! ```sh
//! ffplay dump.h264
//! ```

use std::sync::Arc;

use airplay_server::config::AirPlayConfig;
use airplay_server::consumer::AirPlayConsumer;
use airplay_server::h264_dump::H264DumpConsumer;
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

    tracing::info!("airplay-rs dump example starting...");

    // 创建 H264 转储消费者
    let dump_path = "dump.h264";
    let consumer = H264DumpConsumer::new(dump_path)?;
    tracing::info!("video will be dumped to: {}", dump_path);

    let consumer = Arc::new(consumer) as Arc<dyn AirPlayConsumer>;

    // 创建并启动服务器
    let config = AirPlayConfig::default();
    let mut server = AirPlayServer::new(config, consumer);

    let port = server.start().await?;
    tracing::info!("========================================");
    tracing::info!(" AirPlay server ready!");
    tracing::info!(" Server name: airplay-rs");
    tracing::info!(" Control port: {}", port);
    tracing::info!(" Output file: {}", dump_path);
    tracing::info!("========================================");
    tracing::info!("Connect your iPhone to the same Wi-Fi,");
    tracing::info!("open Screen Mirroring and select 'airplay-rs'.");
    tracing::info!("Press Ctrl+C to stop.");

    // 等待 Ctrl+C
    server.wait_for_shutdown().await;

    // 停止服务器
    server.stop();
    tracing::info!("dump.h264 written. Play with: ffplay {}", dump_path);

    Ok(())
}
