//! AirPlay 控制请求处理器 —— RTSP/HTTP 路由分发。

use std::sync::{Arc, Mutex};

use airplay_protocol::plist_util;
use airplay_protocol::stream_info::MediaStreamInfo;

use crate::config::AirPlayConfig;
use crate::consumer::AirPlayConsumer;
use crate::rtsp_codec::{RtspRequest, RtspResponse};
use crate::session::{Session, SessionManager};
use crate::video::server::VideoServer;
use crate::audio::server::AudioServer;
use crate::audio::server_ctrl::AudioControlServer;

/// 处理 AirPlay 控制请求（RTSP / HTTP 混合）。
///
/// 注意 `consumer` 使用 `Arc<dyn AirPlayConsumer>` 而非 `&dyn`，因为 SETUP 时
/// 需要把 consumer clone 给 VideoServer / AudioServer。
pub async fn handle_request(
    request: &RtspRequest,
    session_manager: &SessionManager,
    config: &AirPlayConfig,
    consumer: Arc<dyn AirPlayConsumer>,
) -> RtspResponse {
    if request.is_rtsp {
        handle_rtsp(request, session_manager, config, consumer).await
    } else {
        handle_http(request, consumer).await
    }
}

/// 构造 RTSP 200 响应：回填 `CSeq` 与 `Server` 头。
fn create_rtsp_response(request: &RtspRequest) -> RtspResponse {
    let mut resp = RtspResponse::ok(true);
    if let Some(cseq) = request.cseq() {
        resp = resp.header("CSeq", cseq);
    }
    resp = resp.header("Server", "AirTunes/220.68");
    resp
}

/// 从请求中解析会话 ID 并获取（或懒创建）会话。
fn resolve_session(
    request: &RtspRequest,
    session_manager: &SessionManager,
) -> Arc<Mutex<Session>> {
    let session_id = request.session_id().unwrap_or("");
    session_manager.get_or_create(session_id)
}

/// RTSP 请求路由。
async fn handle_rtsp(
    request: &RtspRequest,
    session_manager: &SessionManager,
    config: &AirPlayConfig,
    consumer: Arc<dyn AirPlayConsumer>,
) -> RtspResponse {
    match request.method.as_str() {
        "SETUP" => handle_setup(request, session_manager, consumer).await,
        "GET_PARAMETER" => create_rtsp_response(request).body(b"volume: 0.000000\r\n".to_vec()),
        "FLUSH" => create_rtsp_response(request),
        "RECORD" => create_rtsp_response(request)
            .header("Audio-Latency", "11025")
            .header("Audio-Jack-Status", "connected; type=analog"),
        "SET_PARAMETER" => create_rtsp_response(request)
            .header("Audio-Jack-Status", "connected; type=analog"),
        "TEARDOWN" => handle_teardown(request, session_manager, consumer).await,
        "GET" if request.path == "/info" => {
            let plist_config = plist_util::AirPlayConfig {
                width: config.width as i32,
                height: config.height as i32,
                fps: config.fps as f32,
            };
            match plist_util::prepare_info_response(&plist_config) {
                Ok(body) => create_rtsp_response(request)
                    .header("Content-Type", "application/x-apple-binary-plist")
                    .body(body),
                Err(e) => {
                    tracing::warn!("prepare_info_response error: {}", e);
                    RtspResponse::new(500, "Internal Server Error", true)
                }
            }
        }
        "POST" => match request.path.as_str() {
            "/pair-setup" => {
                let session_arc = resolve_session(request, session_manager);
                let session = session_arc.lock().unwrap();
                let body = session.airplay.pair_setup();
                drop(session);
                create_rtsp_response(request).body(body)
            }
            "/pair-verify" => {
                let session_arc = resolve_session(request, session_manager);
                let mut session = session_arc.lock().unwrap();
                match session.airplay.pair_verify(&request.body) {
                    Ok(body) => {
                        drop(session);
                        create_rtsp_response(request).body(body)
                    }
                    Err(e) => {
                        drop(session);
                        tracing::warn!("pair-verify error: {}", e);
                        RtspResponse::new(500, "Internal Server Error", true)
                    }
                }
            }
            "/fp-setup" => {
                let session_arc = resolve_session(request, session_manager);
                let mut session = session_arc.lock().unwrap();
                match session.airplay.fairplay_setup(&request.body) {
                    Ok(body) => {
                        drop(session);
                        create_rtsp_response(request).body(body)
                    }
                    Err(e) => {
                        drop(session);
                        tracing::warn!("fairplay-setup error: {}", e);
                        RtspResponse::new(500, "Internal Server Error", true)
                    }
                }
            }
            "/feedback" => create_rtsp_response(request),
            "/audioMode" => create_rtsp_response(request),
            _ => RtspResponse::not_implemented(true),
        },
        _ => RtspResponse::not_implemented(true),
    }
}

/// SETUP 请求处理：解析流类型，启动对应的子服务器，返回端口信息。
///
/// 关键是启动 VideoServer / AudioServer 并把它们的真实端口写进 SETUP 响应，
/// 否则 iPhone 不知道往哪发数据。
async fn handle_setup(
    request: &RtspRequest,
    session_manager: &SessionManager,
    consumer: Arc<dyn AirPlayConsumer>,
) -> RtspResponse {
    let session_arc = resolve_session(request, session_manager);
    // 锁定 session 执行同步协议操作，提取流信息后立即释放锁
    let stream_info = {
        let mut session = session_arc.lock().unwrap();
        match session.airplay.rtsp_setup(&request.body) {
            Ok(info) => info,
            Err(e) => {
                tracing::warn!("rtsp setup error: {}", e);
                return RtspResponse::new(500, "Internal Server Error", true);
            }
        }
    };
    // 不持有 session 锁时调用 consumer 异步方法
    match stream_info {
        Some(MediaStreamInfo::Audio(audio_info)) => {
            consumer.on_audio_format(audio_info).await;
            // 启动音频服务器和控制服务器，获取真实端口
            let (data_port, control_port) = match start_audio_servers(
                &session_arc,
                consumer.clone(),
            ).await {
                Ok(ports) => ports,
                Err(e) => {
                    tracing::warn!("failed to start audio servers: {}", e);
                    return RtspResponse::new(500, "Internal Server Error", true);
                }
            };
            match plist_util::prepare_setup_audio_response(data_port, control_port) {
                Ok(body) => create_rtsp_response(request)
                    .header("Content-Type", "application/x-apple-binary-plist")
                    .body(body),
                Err(e) => {
                    tracing::warn!("prepare_setup_audio_response error: {}", e);
                    RtspResponse::new(500, "Internal Server Error", true)
                }
            }
        }
        Some(MediaStreamInfo::Video(video_info)) => {
            consumer.on_video_format(video_info).await;
            // 启动视频服务器，获取真实端口
            let data_port = match start_video_server(
                &session_arc,
                consumer.clone(),
            ).await {
                Ok(port) => port,
                Err(e) => {
                    tracing::warn!("failed to start video server: {}", e);
                    return RtspResponse::new(500, "Internal Server Error", true);
                }
            };
            // eventPort 和 timingPort：eventPort 是 control server 端口，
            // timingPort 为 0（不使用）。MVP 阶段保持一致。
            match plist_util::prepare_setup_video_response(data_port, 0, 0) {
                Ok(body) => create_rtsp_response(request)
                    .header("Content-Type", "application/x-apple-binary-plist")
                    .body(body),
                Err(e) => {
                    tracing::warn!("prepare_setup_video_response error: {}", e);
                    RtspResponse::new(500, "Internal Server Error", true)
                }
            }
        }
        None => create_rtsp_response(request),
    }
}

/// 启动视频服务器并返回绑定的端口。
///
/// 创建 VideoServer 实例并存入 Session，启动监听后返回端口。
async fn start_video_server(
    session_arc: &Arc<Mutex<Session>>,
    consumer: Arc<dyn AirPlayConsumer>,
) -> anyhow::Result<i32> {
    // 先创建 VideoServer（需要 session 引用）
    let video_server = VideoServer::new(session_arc.clone(), consumer);
    // 启动并获取端口
    let port = video_server.start().await?;
    // 存入 session
    let mut session = session_arc.lock().unwrap();
    session.video_server = Some(video_server);
    Ok(port as i32)
}

/// 启动音频服务器和控制服务器，返回 (data_port, control_port)。
async fn start_audio_servers(
    session_arc: &Arc<Mutex<Session>>,
    consumer: Arc<dyn AirPlayConsumer>,
) -> anyhow::Result<(i32, i32)> {
    let audio_server = AudioServer::new(session_arc.clone(), consumer);
    let data_port = audio_server.start().await?;

    let audio_control_server = AudioControlServer::new();
    let control_port = audio_control_server.start().await?;

    let mut session = session_arc.lock().unwrap();
    session.audio_server = Some(audio_server);
    session.audio_control_server = Some(audio_control_server);
    Ok((data_port as i32, control_port as i32))
}

/// TEARDOWN 请求处理：通知 consumer 断开，停止子服务器。
async fn handle_teardown(
    request: &RtspRequest,
    session_manager: &SessionManager,
    consumer: Arc<dyn AirPlayConsumer>,
) -> RtspResponse {
    let session_arc = resolve_session(request, session_manager);
    let stream_info = {
        let mut session = session_arc.lock().unwrap();
        match session.airplay.rtsp_teardown(&request.body) {
            Ok(info) => info,
            Err(e) => {
                tracing::warn!("rtsp teardown error: {}", e);
                return RtspResponse::new(500, "Internal Server Error", true);
            }
        }
    };
    match stream_info {
        Some(MediaStreamInfo::Audio(_)) => {
            consumer.on_audio_src_disconnect().await;
        }
        Some(MediaStreamInfo::Video(_)) => {
            consumer.on_video_src_disconnect().await;
        }
        None => {
            consumer.on_audio_src_disconnect().await;
            consumer.on_video_src_disconnect().await;
        }
    }
    {
        let mut session = session_arc.lock().unwrap();
        session.stop_servers();
    }
    create_rtsp_response(request)
}

/// HTTP 请求路由。
async fn handle_http(request: &RtspRequest, consumer: Arc<dyn AirPlayConsumer>) -> RtspResponse {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/server-info") => {
            let body = plist_util::prepare_server_info_response();
            RtspResponse::ok(false)
                .header("Content-Type", "text/x-apple-plist+xml")
                .body(body)
        }
        ("POST", "/reverse") => {
            let upgrade = request.header("Upgrade").unwrap_or("").to_string();
            RtspResponse::new(101, "Switching Protocols", false)
                .header("Upgrade", &upgrade)
                .header("Connection", "Upgrade")
        }
        ("POST", "/play") => RtspResponse::not_implemented(false),
        ("POST", "/rate") => RtspResponse::ok(false),
        ("GET", "/playback-info") => {
            let pb = consumer.playback_info().await;
            let plist_pb = plist_util::PlaybackInfo {
                duration: pb.duration,
                position: pb.position,
            };
            let body = plist_util::prepare_playback_info_response(&plist_pb);
            RtspResponse::ok(false)
                .header("Content-Type", "text/x-apple-plist+xml")
                .body(body)
        }
        ("POST", "/action") => RtspResponse::ok(false),
        ("POST", "/getProperty") => RtspResponse::ok(false),
        ("POST", "/setProperty") => RtspResponse::ok(false),
        ("POST", "/scrub") => RtspResponse::ok(false),
        ("POST", "/stop") => RtspResponse::ok(false),
        ("GET", path) if path.starts_with("/playlist") => RtspResponse::not_implemented(false),
        _ => RtspResponse::new(404, "Not Found", false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use airplay_protocol::stream_info::{AudioStreamInfo, VideoStreamInfo};
    use async_trait::async_trait;
    use bytes::Bytes;

    /// 空实现 consumer，用于测试。
    struct NoopConsumer;

    #[async_trait]
    impl AirPlayConsumer for NoopConsumer {
        async fn on_video_format(&self, _info: VideoStreamInfo) {}
        async fn on_video(&self, _bytes: &[u8]) {}
        async fn on_video_src_disconnect(&self) {}
        async fn on_audio_format(&self, _info: AudioStreamInfo) {}
        async fn on_audio(&self, _bytes: &[u8]) {}
        async fn on_audio_src_disconnect(&self) {}
    }

    fn make_rtsp_request(method: &str, path: &str) -> RtspRequest {
        RtspRequest {
            method: method.to_string(),
            path: path.to_string(),
            version: "RTSP/1.0".to_string(),
            is_rtsp: true,
            headers: vec![("CSeq".to_string(), "0".to_string())],
            body: Bytes::new(),
        }
    }

    fn make_http_request(method: &str, path: &str) -> RtspRequest {
        RtspRequest {
            method: method.to_string(),
            path: path.to_string(),
            version: "HTTP/1.1".to_string(),
            is_rtsp: false,
            headers: vec![],
            body: Bytes::new(),
        }
    }

    #[tokio::test]
    async fn test_get_info_returns_binary_plist() {
        let config = AirPlayConfig::default();
        let sm = SessionManager::new();
        let consumer: Arc<dyn AirPlayConsumer> = Arc::new(NoopConsumer);

        let request = make_rtsp_request("GET", "/info");

        let response = handle_request(&request, &sm, &config, consumer).await;
        assert_eq!(response.status_code, 200);
        assert!(response.is_rtsp);
        assert!(!response.body.is_empty());
        // binary plist 以 "bplist" 开头
        assert_eq!(&response.body[..6], b"bplist");
        // 应包含 CSeq 和 Server 头
        assert!(response
            .headers
            .iter()
            .any(|(k, v)| k == "CSeq" && v == "0"));
        assert!(response
            .headers
            .iter()
            .any(|(k, v)| k == "Server" && v == "AirTunes/220.68"));
        // 应包含 Content-Type
        assert!(response
            .headers
            .iter()
            .any(|(k, v)| k == "Content-Type" && v == "application/x-apple-binary-plist"));
    }

    #[tokio::test]
    async fn test_unknown_rtsp_returns_501() {
        let config = AirPlayConfig::default();
        let sm = SessionManager::new();
        let consumer: Arc<dyn AirPlayConsumer> = Arc::new(NoopConsumer);

        let request = make_rtsp_request("FOO", "/bar");

        let response = handle_request(&request, &sm, &config, consumer).await;
        assert_eq!(response.status_code, 501);
        assert!(response.is_rtsp);
    }

    #[tokio::test]
    async fn test_get_server_info_http() {
        let config = AirPlayConfig::default();
        let sm = SessionManager::new();
        let consumer: Arc<dyn AirPlayConsumer> = Arc::new(NoopConsumer);

        let request = make_http_request("GET", "/server-info");

        let response = handle_request(&request, &sm, &config, consumer).await;
        assert_eq!(response.status_code, 200);
        assert!(!response.is_rtsp);
        // XML plist 包含 <?xml
        let body_str = std::str::from_utf8(&response.body).unwrap();
        assert!(body_str.starts_with("<?xml"));
        assert!(response
            .headers
            .iter()
            .any(|(k, v)| k == "Content-Type" && v == "text/x-apple-plist+xml"));
    }

    #[tokio::test]
    async fn test_unknown_http_returns_404() {
        let config = AirPlayConfig::default();
        let sm = SessionManager::new();
        let consumer: Arc<dyn AirPlayConsumer> = Arc::new(NoopConsumer);

        let request = make_http_request("GET", "/nonexistent");

        let response = handle_request(&request, &sm, &config, consumer).await;
        assert_eq!(response.status_code, 404);
        assert!(!response.is_rtsp);
    }

    #[tokio::test]
    async fn test_get_parameter_returns_volume() {
        let config = AirPlayConfig::default();
        let sm = SessionManager::new();
        let consumer: Arc<dyn AirPlayConsumer> = Arc::new(NoopConsumer);

        let request = make_rtsp_request("GET_PARAMETER", "rtsp://test/1");

        let response = handle_request(&request, &sm, &config, consumer).await;
        assert_eq!(response.status_code, 200);
        assert_eq!(response.body, b"volume: 0.000000\r\n");
    }

    #[tokio::test]
    async fn test_play_youtube_returns_501() {
        let config = AirPlayConfig::default();
        let sm = SessionManager::new();
        let consumer: Arc<dyn AirPlayConsumer> = Arc::new(NoopConsumer);

        let request = make_http_request("POST", "/play");

        let response = handle_request(&request, &sm, &config, consumer).await;
        assert_eq!(response.status_code, 501);
    }
}
