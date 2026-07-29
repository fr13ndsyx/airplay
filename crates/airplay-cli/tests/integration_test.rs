//! 集成测试 —— 验证 airplay-cli 关键流程。
//!
//! 测试范围：
//! - 状态通道（watch）收发
//! - 命令通道（mpsc）收发
//! - AppStatus 状态转换

use airplay_cli::status::{AppStatus, ServerCommand};
use tokio::sync::{mpsc, watch};

#[tokio::test]
async fn test_status_channel() {
    // 验证 watch 通道的发送和接收
    let (tx, rx) = watch::channel(AppStatus::Stopped);

    // 初始状态
    let status = rx.borrow().clone();
    assert!(matches!(status, AppStatus::Stopped));

    // 发送 Running 状态
    tx.send(AppStatus::Running { port: 12345 }).unwrap();
    let status = rx.borrow().clone();
    assert!(matches!(status, AppStatus::Running { port: 12345 }));

    // 发送 Error 状态
    tx.send(AppStatus::Error("test error".to_string())).unwrap();
    let status = rx.borrow().clone();
    if let AppStatus::Error(msg) = status {
        assert_eq!(msg, "test error");
    } else {
        panic!("expected Error status");
    }
}

#[tokio::test]
async fn test_server_command_channel() {
    // 验证 mpsc 通道的命令发送和接收（用 async recv 避免 blocking_recv 死锁）
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerCommand>();

    tx.send(ServerCommand::Start).unwrap();
    tx.send(ServerCommand::Stop).unwrap();
    tx.send(ServerCommand::Shutdown).unwrap();

    assert!(matches!(rx.recv().await, Some(ServerCommand::Start)));
    assert!(matches!(rx.recv().await, Some(ServerCommand::Stop)));
    assert!(matches!(rx.recv().await, Some(ServerCommand::Shutdown)));
    // drop tx 后 recv 返回 None
    drop(tx);
    assert!(rx.recv().await.is_none());
}

#[tokio::test]
async fn test_status_transitions() {
    // 验证状态转换逻辑
    let (tx, rx) = watch::channel(AppStatus::Stopped);

    // Stopped → Starting
    tx.send(AppStatus::Starting).unwrap();
    let status = rx.borrow().clone();
    assert!(matches!(status, AppStatus::Starting));

    // Starting → Running
    tx.send(AppStatus::Running { port: 6831 }).unwrap();
    let status = rx.borrow().clone();
    if let AppStatus::Running { port } = status {
        assert_eq!(port, 6831);
    } else {
        panic!("expected Running status");
    }

    // Running → Stopped
    tx.send(AppStatus::Stopped).unwrap();
    let status = rx.borrow().clone();
    assert!(matches!(status, AppStatus::Stopped));
}

#[test]
fn test_command_sender_clone() {
    // 验证 CmdTx 可以 clone（托盘和 UI 都需要发送命令）
    let (tx, _rx) = mpsc::unbounded_channel::<ServerCommand>();
    let tx2 = tx.clone();

    // 两个 sender 都能发送（不接收，验证不 panic）
    tx.send(ServerCommand::Start).unwrap();
    tx2.send(ServerCommand::Stop).unwrap();
}
