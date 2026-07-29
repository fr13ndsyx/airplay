//! AirPlay 音频控制服务器 —— UDP 桩，仅记录收到的数据包。
//!
//! AirPlay 协议中音频控制通道用于音量控制等带外信令。MVP 阶段仅接收并记录，
//! 不做实际处理。

use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::net::UdpSocket;
use tracing::{debug, info, warn};

/// UDP 接收缓冲区大小。
const RECV_BUF_SIZE: usize = 8192;

/// AirPlay 音频控制服务器（桩实现）。
///
/// 绑定一个 UDP 端口，循环接收数据包并以 debug 级别记录，不做进一步处理。
pub struct AudioControlServer {
    /// 绑定的端口（启动前为 0）。
    port: AtomicU16,
    /// shutdown 标志。
    shutdown: Arc<AtomicBool>,
}

impl AudioControlServer {
    /// 创建新的音频控制服务器实例。
    pub fn new() -> Self {
        Self {
            port: AtomicU16::new(0),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 启动音频控制服务器，返回绑定的端口。
    ///
    /// 绑定 `0.0.0.0:0` 让操作系统分配空闲端口，然后 spawn recv 循环。
    pub async fn start(&self) -> Result<u16> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        let port = socket.local_addr()?.port();
        self.port.store(port, Ordering::SeqCst);
        info!("audio control server listening on port {}", port);

        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            let mut buf = vec![0u8; RECV_BUF_SIZE];

            loop {
                tokio::select! {
                    recv_result = socket.recv_from(&mut buf) => {
                        match recv_result {
                            Ok((n, addr)) => {
                                debug!(
                                    "audio control packet from {}: {} bytes",
                                    addr, n
                                );
                            }
                            Err(e) => {
                                warn!("audio control recv error: {}", e);
                            }
                        }
                    }
                    _ = shutdown_signal(shutdown.clone()) => {
                        info!("audio control server shutting down");
                        break;
                    }
                }
            }
        });

        Ok(port)
    }

    /// 返回当前绑定的端口（启动前为 0）。
    pub fn port(&self) -> u16 {
        self.port.load(Ordering::SeqCst)
    }

    /// 停止音频控制服务器（设置 shutdown 标志，recv 循环将在下次检查时退出）。
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

impl Default for AudioControlServer {
    fn default() -> Self {
        Self::new()
    }
}

/// 等待 shutdown 标志被设置。
async fn shutdown_signal(shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
