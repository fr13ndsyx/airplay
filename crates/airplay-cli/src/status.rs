//! UI 与 Server 之间的状态同步。
//!
//! 使用 `tokio::sync::watch` 广播服务器状态，
//! 使用 `tokio::sync::mpsc::unbounded_channel` 传递 UI 命令。

use tokio::sync::{mpsc, watch};

/// 应用状态（由 server task 广播给 UI）。
#[derive(Clone, Debug)]
pub enum AppStatus {
    /// 已停止。
    Stopped,
    /// 启动中。
    Starting,
    /// 运行中，监听指定端口。
    Running { port: u16 },
    /// iPhone 已连接，正在投屏。
    Connected,
    /// iPhone 已断开。
    Disconnected { port: u16 },
    /// 错误。
    Error(String),
}

/// UI → Server 的命令。
#[derive(Clone, Debug)]
pub enum ServerCommand {
    /// 启动服务器。
    Start,
    /// 停止服务器。
    Stop,
    /// 关闭整个 server task。
    Shutdown,
    /// 设置音量 (0.0 ~ 1.0)。
    SetVolume(f32),
}

/// 状态广播 sender。
pub type StatusTx = watch::Sender<AppStatus>;
/// 状态广播 receiver。
pub type StatusRx = watch::Receiver<AppStatus>;
/// 命令 sender。
pub type CmdTx = mpsc::UnboundedSender<ServerCommand>;
/// 命令 receiver。
pub type CmdRx = mpsc::UnboundedReceiver<ServerCommand>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_watch_channel_send_recv() {
        let (tx, mut rx) = watch::channel(AppStatus::Stopped);
        // 初始值
        let status = rx.borrow().clone();
        assert!(matches!(status, AppStatus::Stopped));

        // 发送新状态
        tx.send(AppStatus::Starting).unwrap();
        assert!(rx.changed().await.is_ok());
        let status = rx.borrow().clone();
        assert!(matches!(status, AppStatus::Starting));

        // Running 状态
        tx.send(AppStatus::Running { port: 7000 }).unwrap();
        assert!(rx.changed().await.is_ok());
        let status = rx.borrow().clone();
        match status {
            AppStatus::Running { port } => assert_eq!(port, 7000),
            _ => panic!("expected Running"),
        }
    }

    #[tokio::test]
    async fn test_mpsc_unbounded_channel() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        tx.send(ServerCommand::Start).unwrap();
        tx.send(ServerCommand::Stop).unwrap();

        let cmd1 = rx.recv().await;
        let cmd2 = rx.recv().await;
        assert!(matches!(cmd1, Some(ServerCommand::Start)));
        assert!(matches!(cmd2, Some(ServerCommand::Stop)));
    }

    #[test]
    fn test_status_clone_debug() {
        let s = AppStatus::Error("test".into());
        let s_clone = s.clone();
        let debug = format!("{:?}", s_clone);
        assert!(debug.contains("test"));
    }
}
