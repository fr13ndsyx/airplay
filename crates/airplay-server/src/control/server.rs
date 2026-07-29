//! AirPlay 控制服务器 —— 接受 TCP 连接并分发给 ControlHandler。

use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Decoder;
use tracing::{info, warn};

use crate::config::AirPlayConfig;
use crate::consumer::AirPlayConsumer;
use crate::rtsp_codec::RtspCodec;
use crate::session::SessionManager;

use super::handler;

/// AirPlay 控制服务器。
///
/// 监听一个 TCP 端口，接受 AirPlay 客户端的 RTSP/HTTP 混合请求。
/// 每个连接在独立的 tokio 任务中处理，通过 `handler::handle_request` 路由。
pub struct ControlServer {
    session_manager: Arc<SessionManager>,
    config: Arc<AirPlayConfig>,
    consumer: Arc<dyn AirPlayConsumer>,
    port: AtomicU16,
    shutdown: Arc<AtomicBool>,
}

impl ControlServer {
    /// 创建一个新的控制服务器实例。
    pub fn new(config: Arc<AirPlayConfig>, consumer: Arc<dyn AirPlayConsumer>) -> Self {
        Self {
            session_manager: Arc::new(SessionManager::new()),
            config,
            consumer,
            port: AtomicU16::new(0),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 启动控制服务器，返回绑定的端口。
    ///
    /// 绑定 `0.0.0.0:0` 让操作系统分配空闲端口，然后 spawn accept 循环。
    pub async fn start(&self) -> Result<u16> {
        let listener = TcpListener::bind("0.0.0.0:0").await?;
        let port = listener.local_addr()?.port();
        self.port.store(port, Ordering::SeqCst);
        info!("control server listening on port {}", port);

        let shutdown = self.shutdown.clone();
        let session_manager = self.session_manager.clone();
        let config = self.config.clone();
        let consumer = self.consumer.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((stream, addr)) => {
                                info!("control connection from {}", addr);
                                let sm = session_manager.clone();
                                let cfg = config.clone();
                                let con = consumer.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handle_connection(stream, sm, cfg, con).await {
                                        warn!("control connection error: {}", e);
                                    }
                                });
                            }
                            Err(e) => {
                                warn!("control accept error: {}", e);
                            }
                        }
                    }
                    _ = shutdown_signal(shutdown.clone()) => {
                        info!("control server shutting down");
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

    /// 停止控制服务器（设置 shutdown 标志，accept 循环将在下次检查时退出）。
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

/// 处理单个控制连接。
///
/// 循环读取并解码 RTSP/HTTP 请求，交给 `handler::handle_request` 处理，
/// 然后将响应写回。EOF 或错误时退出循环。
async fn handle_connection(
    stream: TcpStream,
    session_manager: Arc<SessionManager>,
    config: Arc<AirPlayConfig>,
    consumer: Arc<dyn AirPlayConsumer>,
) -> Result<()> {
    let mut stream = stream;
    let mut codec = RtspCodec;
    let mut buf = BytesMut::new();

    loop {
        // 解码一个完整请求，不足时继续从流中读取
        let request = loop {
            if let Some(req) = codec.decode(&mut buf)? {
                break req;
            }
            let n = stream.read_buf(&mut buf).await?;
            if n == 0 {
                if buf.is_empty() {
                    return Ok(());
                }
                warn!("connection closed with {} bytes of partial data", buf.len());
                return Ok(());
            }
        };

        let response =
            handler::handle_request(&request, &session_manager, &config, consumer.clone()).await;
        let bytes = response.to_bytes();
        stream.write_all(&bytes).await?;
    }
}
