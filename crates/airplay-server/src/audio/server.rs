//! AirPlay 音频服务器 —— UDP 接收，RTP 解析 + 重排 + FairPlay 解密后转发消费者。
//!
//! 与视频服务器（TCP）不同，音频走 UDP：每个数据报独立，无连接概念。
//! 服务器维护单个 `AudioReorderBuffer`，按 RTP 序列号重排后解密、转发。

use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use tokio::net::UdpSocket;
use tracing::{debug, info, warn};

use crate::consumer::AirPlayConsumer;
use crate::session::Session;

use super::reorder::AudioReorderBuffer;
use super::rtp::parse_audio_packet;

/// UDP 接收缓冲区大小（AirPlay 音频帧通常远小于此值）。
const RECV_BUF_SIZE: usize = 8192;

/// AirPlay 音频服务器。
///
/// 绑定一个 UDP 端口，循环接收 RTP 音频包：解析 → 重排 → 解密 → 转发消费者。
/// 重排缓冲区在 recv 任务内持有（UDP 无连接，整条流共享一个缓冲区）。
pub struct AudioServer {
    /// 所属会话（持有 AirPlay facade 用于 FairPlay 解密）。
    session: Arc<Mutex<Session>>,
    /// 媒体消费者（接收解密后的音频数据）。
    consumer: Arc<dyn AirPlayConsumer>,
    /// 绑定的端口（启动前为 0）。
    port: AtomicU16,
    /// shutdown 标志。
    shutdown: Arc<AtomicBool>,
}

impl AudioServer {
    /// 创建新的音频服务器实例。
    pub fn new(session: Arc<Mutex<Session>>, consumer: Arc<dyn AirPlayConsumer>) -> Self {
        Self {
            session,
            consumer,
            port: AtomicU16::new(0),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 启动音频服务器，返回绑定的端口。
    ///
    /// 绑定 `0.0.0.0:0` 让操作系统分配空闲端口，然后 spawn recv 循环。
    pub async fn start(&self) -> Result<u16> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        let port = socket.local_addr()?.port();
        self.port.store(port, Ordering::SeqCst);
        info!("audio server listening on port {}", port);

        let shutdown = self.shutdown.clone();
        let session = self.session.clone();
        let consumer = self.consumer.clone();

        tokio::spawn(async move {
            let mut reorder_buf = AudioReorderBuffer::new();
            let mut buf = vec![0u8; RECV_BUF_SIZE];

            loop {
                tokio::select! {
                    recv_result = socket.recv_from(&mut buf) => {
                        match recv_result {
                            Ok((n, _addr)) => {
                                if n == 0 {
                                    continue;
                                }
                                handle_datagram(&buf[..n], &mut reorder_buf, &session, &consumer).await;
                            }
                            Err(e) => {
                                warn!("audio recv error: {}", e);
                            }
                        }
                    }
                    _ = shutdown_signal(shutdown.clone()) => {
                        info!("audio server shutting down");
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

    /// 停止音频服务器（设置 shutdown 标志，recv 循环将在下次检查时退出）。
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

/// 等待 shutdown 标志被设置。
async fn shutdown_signal(shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// 处理单个 UDP 数据报：解析 RTP → 重排 → 解密 → 转发。
async fn handle_datagram(
    data: &[u8],
    reorder_buf: &mut AudioReorderBuffer,
    session: &Arc<Mutex<Session>>,
    consumer: &Arc<dyn AirPlayConsumer>,
) {
    let packet = match parse_audio_packet(data) {
        Some(p) => p,
        None => {
            debug!("audio packet too short ({} bytes), ignored", data.len());
            return;
        }
    };

    let seq = packet.header.sequence_number;
    let drained = reorder_buf.push(packet);

    for mut pkt in drained {
        // 持锁解密，然后立即释放（不跨 await 持有锁）
        {
            let mut s = match session.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    warn!("audio session mutex poisoned: {}", e);
                    return;
                }
            };
            // 先取长度，再传可变引用（避免同时可变/不可变借用）
            let len = pkt.payload.len();
            if let Err(e) = s.airplay.decrypt_audio(&mut pkt.payload, len) {
                warn!("audio decrypt error (seq={}): {}", seq, e);
                return;
            }
        }
        // 锁已释放，安全地 await 消费者
        consumer.on_audio(&pkt.payload).await;
    }
}
