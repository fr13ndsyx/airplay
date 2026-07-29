//! airplay-cli —— egui GUI + 系统托盘的 AirPlay 接收端。
//!
//! 双击即用的桌面应用：
//! - 系统托盘常驻，右键菜单控制
//! - egui 主窗口作为控制面板
//! - GStreamer 视频独立原生窗口
//! - 关闭主窗口 → 最小化到托盘

mod app;
mod server_task;
mod status;
mod status_consumer;
mod tray;

use std::sync::Arc;

use airplay_player::GstPlayer;
use airplay_server::consumer::AirPlayConsumer;
use status::AppStatus;
use tokio::sync::{mpsc, watch};
use tracing_subscriber::EnvFilter;

use crate::status_consumer::StatusConsumer;

fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("airplay-cli 启动中...");

    // 在主线程创建 GstPlayer（D3D11 元素需要主线程）
    let player = GstPlayer::new()?;
    tracing::info!("GStreamer 播放器已创建（主线程）");

    // 创建 tokio runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    // 创建通道
    let (status_tx, status_rx) = watch::channel(AppStatus::Stopped);
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

    // spawn server task（传入主线程创建的 player）
    runtime.spawn(server_task::run_server(status_tx, cmd_rx, player));

    // 创建系统托盘（在主线程，Windows 托盘需要主线程消息循环）
    let _tray = tray::TrayState::new(cmd_tx.clone())?;

    // 运行 eframe（阻塞主线程）
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "airplay-rs",
        native_options,
        Box::new(move |cc| -> Box<dyn eframe::App> {
            Box::new(app::AirPlayApp::new(cc, status_rx, cmd_tx))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe 运行失败: {}", e))?;

    // eframe 退出后，发送 Shutdown + 关闭 runtime
    tracing::info!("eframe 退出，正在关闭...");
    runtime.shutdown_timeout(std::time::Duration::from_secs(2));

    Ok(())
}
