//! AirPlay 视频服务器 —— 接受 TCP 连接，解码视频包，FairPlay 解密 + NAL 重写后转发给消费者。

use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use bytes::BytesMut;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Decoder;
use tracing::{debug, info, warn};

use crate::consumer::AirPlayConsumer;
use crate::session::Session;

use super::decoder::{VideoCodec, VideoPacket};
use super::nal::{extract_sps_pps, rewrite_avcc_to_annexb};

/// AirPlay 视频服务器。
///
/// 监听一个 TCP 端口，接受 AirPlay 客户端的视频流连接。
/// 每个连接在独立的 tokio 任务中处理：解码 → 解密 → NAL 重写 → 转发消费者。
pub struct VideoServer {
    /// 所属会话（持有 AirPlay facade 用于 FairPlay 解密）。
    session: Arc<Mutex<Session>>,
    /// 媒体消费者（接收解密后的视频数据）。
    consumer: Arc<dyn AirPlayConsumer>,
    /// 绑定的端口（启动前为 0）。
    port: AtomicU16,
    /// shutdown 标志。
    shutdown: Arc<AtomicBool>,
}

impl VideoServer {
    /// 创建新的视频服务器实例。
    pub fn new(session: Arc<Mutex<Session>>, consumer: Arc<dyn AirPlayConsumer>) -> Self {
        Self {
            session,
            consumer,
            port: AtomicU16::new(0),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 启动视频服务器，返回绑定的端口。
    ///
    /// 绑定 `0.0.0.0:0` 让操作系统分配空闲端口，然后 spawn accept 循环。
    pub async fn start(&self) -> Result<u16> {
        let listener = TcpListener::bind("0.0.0.0:0").await?;
        let port = listener.local_addr()?.port();
        self.port.store(port, Ordering::SeqCst);
        info!("video server listening on port {}", port);

        let shutdown = self.shutdown.clone();
        let session = self.session.clone();
        let consumer = self.consumer.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((stream, addr)) => {
                                info!("video connection from {}", addr);
                                let s = session.clone();
                                let c = consumer.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handle_connection(stream, s, c).await {
                                        warn!("video connection error: {}", e);
                                    }
                                });
                            }
                            Err(e) => {
                                warn!("video accept error: {}", e);
                            }
                        }
                    }
                    _ = shutdown_signal(shutdown.clone()) => {
                        info!("video server shutting down");
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

    /// 停止视频服务器（设置 shutdown 标志，accept 循环将在下次检查时退出）。
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

/// 处理单个视频连接。
///
/// 循环读取并解码视频包，按 payloadType 分发：
/// - 类型 0：FairPlay 解密 → AVCC→Annex-B 重写 → `consumer.on_video()`
/// - 类型 1：提取 SPS/PPS → `consumer.on_video()`
///
/// EOF 或错误时退出循环。
async fn handle_connection(
    stream: TcpStream,
    session: Arc<Mutex<Session>>,
    consumer: Arc<dyn AirPlayConsumer>,
) -> Result<()> {
    let mut stream = stream;
    let mut codec = VideoCodec;
    let mut buf = BytesMut::new();

    // 每个新视频 TCP 连接开始时重置解密器。
    // VideoDecryptor 是有状态的 AES-CTR 流密码，若 iPhone 重连视频端口，
    // 旧解密器的计数器已推进到错误位置，新连接必须从计数器 0 开始。
    {
        let mut s = session.lock().expect("session mutex poisoned");
        s.reset_video_decryptor();
    }
    info!("视频连接已重置解密器，准备接收数据");

    loop {
        // 解码缓冲区中所有完整帧
        loop {
            match codec.decode(&mut buf)? {
                Some(packet) => {
                    handle_packet(packet, &session, &consumer).await?;
                }
                None => break,
            }
        }

        // 从流中读取更多数据
        let n = stream.read_buf(&mut buf).await?;
        if n == 0 {
            if buf.is_empty() {
                return Ok(());
            }
            warn!(
                "video connection closed with {} bytes of partial data",
                buf.len()
            );
            return Ok(());
        }
    }
}

/// 处理单个视频包。
async fn handle_packet(
    packet: VideoPacket,
    session: &Arc<Mutex<Session>>,
    consumer: &Arc<dyn AirPlayConsumer>,
) -> Result<()> {
    match packet.payload_type {
        0 => {
            // 加密 NAL：解密 → 重写 → 转发
            let mut payload = packet.payload;
            {
                // 持锁解密，然后立即释放（不跨 await 持有锁）
                let mut s = session.lock().expect("session mutex poisoned");
                s.airplay.decrypt_video(&mut payload)?;
            }
            // 诊断：检查解密后的 AVCC 长度前缀是否合理
            if payload.len() >= 4 {
                let avcc_len = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
                let nal_header = if payload.len() >= 5 { payload[4] } else { 0 };
                let forbidden = (nal_header & 0x80) != 0;
                let nal_type = nal_header & 0x1F;
                // AVCC 长度前缀应该等于 payload.len() - 4（单 NAL）或小于 payload.len()
                let len_valid = avcc_len > 0 && avcc_len + 4 <= payload.len();
                if !len_valid || forbidden {
                    tracing::warn!(
                        "解密诊断: payload={}字节, avcc_len={}, nal_header=0x{:02X}, forbidden={}, nal_type={}, len_valid={}",
                        payload.len(), avcc_len, nal_header, forbidden, nal_type, len_valid
                    );
                }
            }
            rewrite_avcc_to_annexb(&mut payload);
            consumer.on_video(&payload).await;
        }
        1 => {
            // AVCDecoderConfigurationRecord：提取 SPS/PPS
            if let Some(sps_pps) = extract_sps_pps(&packet.payload) {
                consumer.on_video(&sps_pps).await;
            } else {
                debug!("failed to extract SPS/PPS from AVCDecoderConfigurationRecord");
            }
        }
        _ => {
            // 不应到达此处（类型 5 等已在解码层跳过）
            debug!(
                payload_type = packet.payload_type,
                "unexpected video packet type in handler"
            );
        }
    }
    Ok(())
}
