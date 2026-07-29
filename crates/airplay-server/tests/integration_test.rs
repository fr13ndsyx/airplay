//! 集成测试 —— 启动 ControlServer，通过 TCP 验证 RTSP/HTTP 请求-响应。
//!
//! 测试策略：
//! 1. 启动 ControlServer（绑定 0.0.0.0:0，获取随机端口）
//! 2. TCP 连接到该端口，发送 fixtures 中的真实请求字节
//! 3. 验证响应状态码、Content-Type、body 结构
//!
//! 注意：pair-verify / fp-setup 的 fixtures 来自真实 iPhone 抓包，
//! 但我们服务器的密钥不同，因此只验证响应结构（状态码 + 非空 body），
//! 不验证密码学正确性。

use std::sync::Arc;

use airplay_server::config::AirPlayConfig;
use airplay_server::consumer::AirPlayConsumer;
use airplay_server::control::server::ControlServer;
use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use airplay_protocol::stream_info::{AudioStreamInfo, VideoStreamInfo};

/// 空实现 consumer。
struct NoopConsumer;

#[async_trait]
impl AirPlayConsumer for NoopConsumer {
    async fn on_video_format(&self, _: VideoStreamInfo) {}
    async fn on_video(&self, _: &[u8]) {}
    async fn on_video_src_disconnect(&self) {}
    async fn on_audio_format(&self, _: AudioStreamInfo) {}
    async fn on_audio(&self, _: &[u8]) {}
    async fn on_audio_src_disconnect(&self) {}
}

/// 读取 fixture 文件。
fn read_fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../airplay-protocol/tests/fixtures/one_mirroring_app/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {}", name, e))
}

/// 发送请求字节并读取完整响应。
///
/// 返回响应字节（含状态行 + headers + body）。
async fn send_request(port: u16, request_bytes: &[u8]) -> Vec<u8> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("failed to connect");

    stream.write_all(request_bytes).await.expect("write failed");
    stream.flush().await.expect("flush failed");

    let mut response = Vec::new();
    // 读取足够多的字节（给一个较大的上限）
    let mut buf = [0u8; 8192];
    loop {
        match tokio::time::timeout(
            std::time::Duration::from_secs(2),
            stream.read(&mut buf),
        )
        .await
        {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => {
                response.extend_from_slice(&buf[..n]);
                // 如果响应已完整（包含 \r\n\r\n 且 body 长度匹配），可以提前退出
                if is_response_complete(&response) {
                    break;
                }
            }
            Ok(Err(e)) => panic!("read error: {}", e),
            Err(_) => break, // 超时，返回已有数据
        }
    }
    response
}

/// 简单判断响应是否完整（找到 \r\n\r\n 并验证 Content-Length）。
fn is_response_complete(data: &[u8]) -> bool {
    let header_end = match find_subsequence(data, b"\r\n\r\n") {
        Some(pos) => pos,
        None => return false,
    };
    let header_str = match std::str::from_utf8(&data[..header_end]) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let content_length: usize = header_str
        .lines()
        .find_map(|line| {
            let line = line.to_lowercase();
            line.strip_prefix("content-length:")
                .and_then(|s| s.trim().parse().ok())
        })
        .unwrap_or(0);
    let body_start = header_end + 4;
    data.len() >= body_start + content_length
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// 启动 ControlServer 并返回端口。
async fn start_server() -> (u16, Arc<ControlServer>) {
    let config = Arc::new(AirPlayConfig::default());
    let consumer = Arc::new(NoopConsumer) as Arc<dyn AirPlayConsumer>;
    let server = Arc::new(ControlServer::new(config, consumer));
    let port = server.start().await.expect("failed to start server");
    (port, server)
}

// ============================================================
// 测试用例
// ============================================================

#[tokio::test]
async fn test_info_request_returns_binary_plist() {
    let (port, _server) = start_server().await;
    let request = read_fixture("01_RTSP_GET_info_request.bin");
    let response = send_request(port, &request).await;

    let response_str = String::from_utf8_lossy(&response);
    assert!(
        response_str.starts_with("RTSP/1.0 200 OK\r\n"),
        "expected 200 OK, got: {}",
        &response_str[..response_str.len().min(100)]
    );
    assert!(
        response_str.contains("Content-Type: application/x-apple-binary-plist"),
        "expected binary plist content type"
    );
    // body 应以 "bplist" 开头
    if let Some(pos) = find_subsequence(&response, b"\r\n\r\n") {
        let body = &response[pos + 4..];
        assert!(
            body.starts_with(b"bplist"),
            "body should start with bplist, got: {:?}",
            &body[..body.len().min(10)]
        );
    } else {
        panic!("no header/body separator found");
    }
}

#[tokio::test]
async fn test_server_info_http_returns_xml_plist() {
    let (port, _server) = start_server().await;
    let request = read_fixture("12_HTTP_GET_server_info_request.bin");
    let response = send_request(port, &request).await;

    let response_str = String::from_utf8_lossy(&response);
    assert!(
        response_str.starts_with("HTTP/1.1 200 OK\r\n"),
        "expected HTTP 200 OK"
    );
    assert!(
        response_str.contains("Content-Type: text/x-apple-plist+xml"),
        "expected XML plist content type"
    );
    // body 应包含 <?xml
    if let Some(pos) = find_subsequence(&response, b"\r\n\r\n") {
        let body = &response[pos + 4..];
        let body_str = std::str::from_utf8(body).unwrap_or("");
        assert!(body_str.starts_with("<?xml"), "body should be XML plist");
    }
}

#[tokio::test]
async fn test_pair_setup_returns_32_byte_public_key() {
    let (port, _server) = start_server().await;

    // 构造 pair-setup POST 请求
    let request = b"POST /pair-setup RTSP/1.0\r\nCSeq: 1\r\nX-Apple-Session-ID: test-integration-1\r\nContent-Length: 0\r\n\r\n";
    let response = send_request(port, request).await;

    let response_str = String::from_utf8_lossy(&response);
    assert!(
        response_str.starts_with("RTSP/1.0 200 OK\r\n"),
        "expected 200 OK, got: {}",
        &response_str[..response_str.len().min(50)]
    );

    if let Some(pos) = find_subsequence(&response, b"\r\n\r\n") {
        let body = &response[pos + 4..];
        // Ed25519 公钥为 32 字节
        assert_eq!(
            body.len(),
            32,
            "pair-setup should return 32-byte Ed25519 public key, got {} bytes",
            body.len()
        );
    } else {
        panic!("no body found");
    }
}

#[tokio::test]
async fn test_get_parameter_returns_volume() {
    let (port, _server) = start_server().await;
    let request = read_fixture("07_RTSP_GET_PARAMETER_request.bin");
    let response = send_request(port, &request).await;

    let response_str = String::from_utf8_lossy(&response);
    assert!(
        response_str.starts_with("RTSP/1.0 200 OK\r\n"),
        "expected 200 OK"
    );
    // body 应包含 "volume: 0.000000"
    assert!(
        response_str.contains("volume: 0.000000"),
        "expected volume in response"
    );
}

#[tokio::test]
async fn test_record_returns_audio_latency() {
    let (port, _server) = start_server().await;
    let request = read_fixture("08_RTSP_RECORD_request.bin");
    let response = send_request(port, &request).await;

    let response_str = String::from_utf8_lossy(&response);
    assert!(
        response_str.starts_with("RTSP/1.0 200 OK\r\n"),
        "expected 200 OK"
    );
    assert!(
        response_str.contains("Audio-Latency:"),
        "expected Audio-Latency header"
    );
    assert!(
        response_str.contains("Audio-Jack-Status:"),
        "expected Audio-Jack-Status header"
    );
}

#[tokio::test]
async fn test_flush_returns_ok() {
    let (port, _server) = start_server().await;
    let request = read_fixture("11_RTSP_FLUSH_request.bin");
    let response = send_request(port, &request).await;

    let response_str = String::from_utf8_lossy(&response);
    assert!(
        response_str.starts_with("RTSP/1.0 200 OK\r\n"),
        "expected 200 OK"
    );
}

#[tokio::test]
async fn test_feedback_returns_ok() {
    let (port, _server) = start_server().await;
    let request = read_fixture("15_RTSP_POST_feedback_request.bin");
    let response = send_request(port, &request).await;

    let response_str = String::from_utf8_lossy(&response);
    assert!(
        response_str.starts_with("RTSP/1.0 200 OK\r\n"),
        "expected 200 OK, got: {}",
        &response_str[..response_str.len().min(50)]
    );
}

#[tokio::test]
async fn test_audio_mode_returns_ok() {
    let (port, _server) = start_server().await;
    let request = read_fixture("14_RTSP_POST_audio_mode_request.bin");
    let response = send_request(port, &request).await;

    let response_str = String::from_utf8_lossy(&response);
    assert!(
        response_str.starts_with("RTSP/1.0 200 OK\r\n"),
        "expected 200 OK"
    );
}

#[tokio::test]
async fn test_fp_setup_returns_response() {
    let (port, _server) = start_server().await;
    // fp-setup 第一步（fixture 04）
    let request = read_fixture("04_RTSP_POST_fp_setup_request.bin");
    let response = send_request(port, &request).await;

    let response_str = String::from_utf8_lossy(&response);
    // fp-setup 可能返回 200（成功）或 500（密钥不匹配），但应有响应
    assert!(
        response_str.starts_with("RTSP/1.0 "),
        "expected RTSP response, got: {}",
        &response_str[..response_str.len().min(20)]
    );
    // body 应非空（FairPlay 响应数据）
    if let Some(pos) = find_subsequence(&response, b"\r\n\r\n") {
        let body = &response[pos + 4..];
        assert!(!body.is_empty(), "fp-setup response body should not be empty");
    }
}

#[tokio::test]
async fn test_pair_verify_step1_returns_response() {
    let (port, _server) = start_server().await;
    // pair-verify 第一步（fixture 02）
    let request = read_fixture("02_RTSP_POST_pair_verify_request.bin");
    let response = send_request(port, &request).await;

    let response_str = String::from_utf8_lossy(&response);
    assert!(
        response_str.starts_with("RTSP/1.0 "),
        "expected RTSP response"
    );
    // pair-verify step 1 应返回 200 + 非空 body（服务器公钥 + 签名）
    if let Some(pos) = find_subsequence(&response, b"\r\n\r\n") {
        let body = &response[pos + 4..];
        assert!(
            !body.is_empty(),
            "pair-verify step 1 response body should not be empty"
        );
    }
}

#[tokio::test]
async fn test_unknown_rtsp_returns_501() {
    let (port, _server) = start_server().await;
    let request = b"FOO /bar RTSP/1.0\r\nCSeq: 1\r\n\r\n";
    let response = send_request(port, request).await;

    let response_str = String::from_utf8_lossy(&response);
    assert!(
        response_str.starts_with("RTSP/1.0 501 Not Implemented\r\n"),
        "expected 501, got: {}",
        &response_str[..response_str.len().min(50)]
    );
}

#[tokio::test]
async fn test_unknown_http_returns_404() {
    let (port, _server) = start_server().await;
    let request = b"GET /nonexistent HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let response = send_request(port, request).await;

    let response_str = String::from_utf8_lossy(&response);
    assert!(
        response_str.starts_with("HTTP/1.1 404 Not Found\r\n"),
        "expected 404, got: {}",
        &response_str[..response_str.len().min(50)]
    );
}

#[tokio::test]
async fn test_multiple_requests_on_same_connection() {
    let (port, _server) = start_server().await;

    // 在同一连接上发送两个请求
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connect failed");

    // 第一个请求：GET /info
    let req1 = b"GET /info RTSP/1.0\r\nCSeq: 1\r\nContent-Length: 0\r\n\r\n";
    stream.write_all(req1).await.expect("write 1 failed");
    stream.flush().await.expect("flush 1 failed");

    // 读取第一个响应
    let mut buf = vec![0u8; 8192];
    let mut total = Vec::new();
    loop {
        match tokio::time::timeout(
            std::time::Duration::from_secs(2),
            stream.read(&mut buf),
        )
        .await
        {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => {
                total.extend_from_slice(&buf[..n]);
                if is_response_complete(&total) {
                    break;
                }
            }
            Ok(Err(_)) => break,
            Err(_) => break,
        }
    }
    let resp1_str = String::from_utf8_lossy(&total);
    assert!(resp1_str.starts_with("RTSP/1.0 200 OK\r\n"), "first request should return 200");

    // 第二个请求：GET_PARAMETER
    let req2 = b"GET_PARAMETER rtsp://test/1 RTSP/1.0\r\nCSeq: 2\r\n\r\n";
    stream.write_all(req2).await.expect("write 2 failed");
    stream.flush().await.expect("flush 2 failed");

    // 读取第二个响应
    total.clear();
    loop {
        match tokio::time::timeout(
            std::time::Duration::from_secs(2),
            stream.read(&mut buf),
        )
        .await
        {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => {
                total.extend_from_slice(&buf[..n]);
                if is_response_complete(&total) {
                    break;
                }
            }
            Ok(Err(_)) => break,
            Err(_) => break,
        }
    }
    let resp2_str = String::from_utf8_lossy(&total);
    assert!(resp2_str.contains("volume: 0.000000"), "second request should return volume");
}
