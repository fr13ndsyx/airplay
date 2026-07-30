//! airplay-cli —— 纯命令行 AirPlay 接收端。
//!
//! 启动后自动开启 AirPlay 服务，iPhone 即可发现并投屏。
//! 视频画面由 GStreamer 在独立原生窗口渲染；本进程在控制台输出日志。
//!
//! 退出方式：
//! - 按 Ctrl+C 优雅关闭
//! - 直接关闭控制台窗口
//!
//! 环境变量：
//! - `RUST_LOG`：日志级别，默认 `info`（如 `RUST_LOG=debug`）

mod server_task;
mod status;
mod status_consumer;

use airplay_player::GstPlayer;
use status::AppStatus;
use tokio::signal;
use tokio::sync::{mpsc, watch};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("airplay-cli 启动中...");
    println!("==============================");
    println!("  airplay-rs  (CLI 模式)");
    println!("==============================");
    println!("iPhone 操作：");
    println!("  1. 确保 iPhone 与电脑在同一 Wi-Fi");
    println!("  2. 打开控制中心 → 屏幕镜像");
    println!("  3. 选择 'airplay-rs-mirror'");
    println!("  4. 按 Ctrl+C 退出本程序");
    println!();

    // 在主线程创建 GstPlayer（D3D11 元素需要主线程）
    let player = GstPlayer::new()?;
    tracing::info!("GStreamer 播放器已创建（主线程）");

    // 创建通道
    let (status_tx, mut status_rx) = watch::channel(AppStatus::Stopped);
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

    // spawn server task（传入主线程创建的 player）
    tokio::spawn(server_task::run_server(status_tx.clone(), cmd_rx, player));

    // 自动启动服务
    let _ = cmd_tx.send(status::ServerCommand::Start);

    // 监听状态变化，打印到控制台
    let status_printer = tokio::spawn(async move {
        let mut last = AppStatus::Stopped;
        while status_rx.changed().await.is_ok() {
            let cur = status_rx.borrow().clone();
            // 仅在状态实际变化时打印
            let changed = match (&last, &cur) {
                (AppStatus::Running { port: p1 }, AppStatus::Running { port: p2 }) => p1 != p2,
                (AppStatus::Disconnected { port: p1 }, AppStatus::Disconnected { port: p2 }) => {
                    p1 != p2
                }
                _ => true,
            };
            if changed {
                print_status(&cur);
                last = cur;
            }
        }
    });

    // 等待 Ctrl+C
    match signal::ctrl_c().await {
        Ok(()) => tracing::info!("收到 Ctrl+C，正在关闭..."),
        Err(e) => tracing::warn!("无法监听 Ctrl+C: {}", e),
    }

    // 通知 server task 关闭
    let _ = cmd_tx.send(status::ServerCommand::Shutdown);
    // 给 server task 一点时间清理
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    status_printer.abort();

    tracing::info!("airplay-cli 已退出");
    Ok(())
}

/// 打印当前状态到控制台。
fn print_status(status: &AppStatus) {
    match status {
        AppStatus::Stopped => println!("[状态] 已停止"),
        AppStatus::Starting => println!("[状态] 启动中..."),
        AppStatus::Running { port } => {
            println!("[状态] 等待 iPhone 连接 (端口 {})...", port);
        }
        AppStatus::Connected => println!("[状态] iPhone 已连接，正在投屏"),
        AppStatus::Disconnected { port } => {
            println!("[状态] iPhone 已断开 (端口 {})", port);
        }
        AppStatus::Error(msg) => println!("[状态] 错误: {}", msg),
    }
}
