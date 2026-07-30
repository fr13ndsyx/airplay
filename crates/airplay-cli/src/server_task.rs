//! Server task —— 封装 AirPlayServer + GstPlayer 为 tokio task。
//!
//! 通过 `cmd_rx` 接收 UI 命令，通过 `status_tx` 广播状态。
//! 使用 `Option` 维护 server/player 生命周期。
//!
//! 注意：不调用 `server.wait_for_shutdown()`（它会阻塞等待 Ctrl+C），
//! 而是循环 `cmd_rx.recv()` 等待 UI 命令。

use std::sync::atomic::Ordering;
use std::sync::Arc;

use airplay_player::GstPlayer;
use airplay_server::config::AirPlayConfig;
use airplay_server::consumer::AirPlayConsumer;
use airplay_server::server::AirPlayServer;

use crate::status::{AppStatus, CmdRx, ServerCommand, StatusTx};
use crate::status_consumer::StatusConsumer;

/// 运行 server task。
///
/// 接收 Start/Stop/Shutdown 命令，维护 server 与 player 生命周期。
/// player 由主线程创建后传入（D3D11 元素需要主线程创建）。
pub async fn run_server(status_tx: StatusTx, mut cmd_rx: CmdRx, player: GstPlayer) {
    tracing::info!("server task 启动");

    // player 由主线程创建，整个生命周期由本 task 管理
    // 用 Option 包装以便 Stop 时 drop 重建（实际不重建，只 drop）
    let mut player: Option<GstPlayer> = Some(player);
    let mut server: Option<AirPlayServer> = None;

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            ServerCommand::Start => {
                // 已在运行则忽略
                if server.is_some() {
                    tracing::warn!("服务器已在运行，忽略 Start 命令");
                    continue;
                }

                let _ = status_tx.send(AppStatus::Starting);

                match start_server(&mut player, status_tx.clone()).await {
                    Ok((new_server, port)) => {
                        server = Some(new_server);
                        let _ = status_tx.send(AppStatus::Running { port });
                        tracing::info!("服务器已启动，端口 {}", port);
                    }
                    Err(e) => {
                        tracing::error!("启动服务器失败: {}", e);
                        let _ = status_tx.send(AppStatus::Error(e.to_string()));
                        // 启动失败时清理 player
                        player = None;
                    }
                }
            }
            ServerCommand::Stop => {
                if let Some(mut s) = server.take() {
                    s.stop();
                    tracing::info!("服务器已停止");
                } else {
                    tracing::warn!("服务器未运行，忽略 Stop 命令");
                }
                // 保留 player，仅停止 server（pipelines 已通过 StopVideo/StopAudio 命令清理）
                let _ = status_tx.send(AppStatus::Stopped);
            }
            ServerCommand::SetVolume(vol) => {
                if let Some(ref p) = player {
                    if let Err(e) = p.set_volume(vol) {
                        tracing::warn!("设置音量失败: {}", e);
                    }
                }
            }
            ServerCommand::Shutdown => {
                tracing::info!("收到 Shutdown 命令，清理并退出");
                if let Some(mut s) = server.take() {
                    s.stop();
                }
                // player 在函数结束时自动 drop
                let _ = status_tx.send(AppStatus::Stopped);
                break;
            }
        }
    }

    tracing::info!("server task 退出");
}

/// 创建 AirPlayServer 并启动。
///
/// player 由主线程创建后传入，本函数只创建 consumer 包装和 server。
/// 使用 `StatusConsumer` 包装 GstPlayerConsumer，在 iPhone 连接/断开时广播状态给 UI。
async fn start_server(
    player: &mut Option<GstPlayer>,
    status_tx: StatusTx,
) -> anyhow::Result<(AirPlayServer, u16)> {
    // 取出 player（Start 时才使用）
    let p = player
        .take()
        .ok_or_else(|| anyhow::anyhow!("player 不存在（可能已停止）"))?;

    // 创建内部消费者，桥接 AirPlayConsumer → GstPlayer
    let inner_consumer = p.consumer();
    let inner_consumer = Arc::new(inner_consumer) as Arc<dyn AirPlayConsumer>;

    // 用 StatusConsumer 包装（广播连接状态给 UI）
    let status_consumer = StatusConsumer::new(inner_consumer, status_tx);
    // 获取 port 共享句柄，server 启动后更新
    let port_handle = status_consumer.port_handle();

    // 创建并启动服务器，传入 wrapped consumer
    let config = AirPlayConfig::default();
    let wrapped = Arc::new(status_consumer) as Arc<dyn AirPlayConsumer>;
    let mut server = AirPlayServer::new(config, wrapped);
    let port = server.start().await?;

    // 更新 StatusConsumer 的 port（用于断开时广播 Disconnected { port }）
    port_handle.store(port, Ordering::SeqCst);

    // 保存 player 到外层（server 运行期间保持存活）
    *player = Some(p);

    Ok((server, port))
}
