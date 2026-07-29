//! RTSP / HTTP 请求解析与响应构造。
//!
//! 由于 `httparse` 仅支持 `HTTP/1.x`，本模块使用手写解析器同时支持 `RTSP/1.0` 与 `HTTP/1.1`。
//! AirPlay 控制通道在同一 TCP 连接上混合使用 RTSP（镜像）与 HTTP（信息查询）请求。

use std::str;

use bytes::{Bytes, BytesMut};
use tokio_util::codec::Decoder;

/// 解析后的 RTSP / HTTP 请求。
#[derive(Debug, Clone)]
pub struct RtspRequest {
    /// 请求方法：`GET`、`POST`、`SETUP`、`RECORD`、`TEARDOWN` 等。
    pub method: String,
    /// 请求路径：`/info` 或 `rtsp://host/session-id`。
    pub path: String,
    /// 协议版本字符串：`RTSP/1.0` 或 `HTTP/1.1`。
    pub version: String,
    /// 是否为 RTSP 请求（`version` 以 `RTSP` 开头）。
    pub is_rtsp: bool,
    /// 请求头列表（保持插入顺序，支持重复头）。
    pub headers: Vec<(String, String)>,
    /// 请求体（Content-Length 字节，可能为空）。
    pub body: Bytes,
}

impl RtspRequest {
    /// 按名称查找请求头（大小写不敏感），返回第一个匹配项。
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// 返回 `Content-Length` 头的值（若存在）。
    pub fn content_length(&self) -> Option<usize> {
        self.header("Content-Length")?.parse().ok()
    }

    /// 返回 `CSeq` 头的值（RTSP 序列号，若存在）。
    pub fn cseq(&self) -> Option<&str> {
        self.header("CSeq")
    }

    /// 返回 `X-Apple-Session-ID` 或 `Active-Remote` 头的值（用于会话管理）。
    pub fn session_id(&self) -> Option<&str> {
        self.header("X-Apple-Session-ID")
            .or_else(|| self.header("Active-Remote"))
    }
}

/// RTSP / HTTP 响应构造器。
#[derive(Debug, Clone)]
pub struct RtspResponse {
    /// HTTP 状态码。
    pub status_code: u16,
    /// 状态文本：`OK`、`Not Implemented` 等。
    pub status_text: String,
    /// 是否使用 RTSP 协议版本前缀。
    pub is_rtsp: bool,
    /// 响应头列表。
    pub headers: Vec<(String, String)>,
    /// 响应体。
    pub body: Vec<u8>,
}

impl RtspResponse {
    /// 创建一个新的响应，默认 `OK`。
    pub fn ok(is_rtsp: bool) -> Self {
        Self::new(200, "OK", is_rtsp)
    }

    /// 创建一个 501 Not Implemented 响应。
    pub fn not_implemented(is_rtsp: bool) -> Self {
        Self::new(501, "Not Implemented", is_rtsp)
    }

    /// 创建一个新响应。
    pub fn new(status_code: u16, status_text: &str, is_rtsp: bool) -> Self {
        Self {
            status_code,
            status_text: status_text.to_string(),
            is_rtsp,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// 添加一个响应头（builder 风格）。
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }

    /// 设置响应体（builder 风格）。
    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    /// 序列化为字节（含状态行 + 头 + 空行 + 体）。
    pub fn to_bytes(&self) -> Vec<u8> {
        let version = if self.is_rtsp { "RTSP/1.0" } else { "HTTP/1.1" };
        let mut out = String::new();
        out.push_str(&format!(
            "{} {} {}\r\n",
            version, self.status_code, self.status_text
        ));
        for (k, v) in &self.headers {
            out.push_str(&format!("{}: {}\r\n", k, v));
        }
        if !self.body.is_empty() {
            out.push_str(&format!("Content-Length: {}\r\n", self.body.len()));
        }
        out.push_str("\r\n");

        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

/// RTSP / HTTP 解码器，实现 `tokio_util::codec::Decoder`。
///
/// 逐帧从 `BytesMut` 缓冲区解析出完整的 `RtspRequest`（头 + 体）。
#[derive(Debug, Clone, Default)]
pub struct RtspCodec;

impl Decoder for RtspCodec {
    type Item = RtspRequest;
    type Error = std::io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // 1. 找到 \r\n\r\n（头结束位置）
        let header_end = match find_subsequence(buf, b"\r\n\r\n") {
            Some(pos) => pos,
            None => return Ok(None),
        };

        // 2. 解析头部分
        let header_bytes = &buf[..header_end];
        let header_str = str::from_utf8(header_bytes).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?;

        let mut lines = header_str.split("\r\n");
        let first_line = lines.next().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "empty request line")
        })?;

        let mut parts = first_line.splitn(3, ' ');
        let method = parts
            .next()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "no method"))?
            .to_string();
        let path = parts
            .next()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "no path"))?
            .to_string();
        let version = parts
            .next()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "no version"))?
            .to_string();
        let is_rtsp = version.starts_with("RTSP");

        let mut headers = Vec::new();
        for line in lines {
            if let Some(colon) = line.find(':') {
                let key = line[..colon].trim().to_string();
                let value = line[colon + 1..].trim().to_string();
                headers.push((key, value));
            }
        }

        // 3. 查找 Content-Length
        let content_length: usize = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("Content-Length"))
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(0);

        // 4. 检查 body 是否完整
        let body_start = header_end + 4; // 跳过 \r\n\r\n
        let total_len = body_start + content_length;
        if buf.len() < total_len {
            return Ok(None); // body 未完整，等待更多数据
        }

        // 5. 提取 body 并消费缓冲区
        let body = buf
            .split_to(total_len)
            .freeze()
            .slice(body_start..total_len);

        Ok(Some(RtspRequest {
            method,
            path,
            version,
            is_rtsp,
            headers,
            body,
        }))
    }
}

/// 在 haystack 中查找 needle 的位置。
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 读取 fixture 二进制文件。
    fn read_fixture(name: &str) -> Vec<u8> {
        let path = format!(
            "{}/../airplay-protocol/tests/fixtures/one_mirroring_app/{}",
            env!("CARGO_MANIFEST_DIR"),
            name
        );
        std::fs::read(&path)
            .unwrap_or_else(|e| panic!("failed to read fixture {}: {}", name, e))
    }

    #[test]
    fn test_parse_rtsp_get_info_request() {
        let data = read_fixture("01_RTSP_GET_info_request.bin");
        let mut buf = BytesMut::from(&data[..]);
        let mut codec = RtspCodec;
        let req = codec.decode(&mut buf).unwrap().expect("should parse");

        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/info");
        assert_eq!(req.version, "RTSP/1.0");
        assert!(req.is_rtsp);
        assert_eq!(req.cseq(), Some("0"));
        assert_eq!(req.content_length(), Some(70));
        assert_eq!(req.body.len(), 70);
        assert_eq!(req.session_id(), Some("1589992423"));
        assert!(!buf.is_empty() == false); // 全部消费
    }

    #[test]
    fn test_parse_rtsp_setup_request() {
        let data = read_fixture("06_RTSP_SETUP_request.bin");
        let mut buf = BytesMut::from(&data[..]);
        let mut codec = RtspCodec;
        let req = codec.decode(&mut buf).unwrap().expect("should parse");

        assert_eq!(req.method, "SETUP");
        assert!(req.path.starts_with("rtsp://"));
        assert_eq!(req.version, "RTSP/1.0");
        assert!(req.is_rtsp);
        assert_eq!(req.header("Content-Type"), Some("application/x-apple-binary-plist"));
    }

    #[test]
    fn test_parse_http_get_server_info_request() {
        let data = read_fixture("12_HTTP_GET_server_info_request.bin");
        let mut buf = BytesMut::from(&data[..]);
        let mut codec = RtspCodec;
        let req = codec.decode(&mut buf).unwrap().expect("should parse");

        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/server-info");
        assert_eq!(req.version, "HTTP/1.1");
        assert!(!req.is_rtsp);
        assert_eq!(req.content_length(), Some(0));
        assert!(req.body.is_empty());
        assert_eq!(
            req.header("X-Apple-Session-ID"),
            Some("bfd9ef08-6a73-493a-b853-aa0968f2d58f")
        );
    }

    #[test]
    fn test_parse_partial_request_returns_none() {
        let mut buf = BytesMut::from(&b"GET /info RTSP/1.0\r\nContent-Length: 5\r\n\r\nab"[..]);
        let mut codec = RtspCodec;
        // 只有 2 字节 body，但 Content-Length=5 → 不完整
        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_split_across_calls() {
        let data = read_fixture("01_RTSP_GET_info_request.bin");
        let mid = data.len() / 2;

        let mut buf = BytesMut::new();
        let mut codec = RtspCodec;

        // 第一次只给前半部分 → None
        buf.extend_from_slice(&data[..mid]);
        assert!(codec.decode(&mut buf).unwrap().is_none());

        // 给完整数据 → 解析成功
        buf.extend_from_slice(&data[mid..]);
        let req = codec.decode(&mut buf).unwrap().expect("should parse");
        assert_eq!(req.method, "GET");
        assert_eq!(req.body.len(), 70);
    }

    #[test]
    fn test_parse_multiple_requests_in_one_buffer() {
        let data1 = read_fixture("01_RTSP_GET_info_request.bin");
        let data2 = read_fixture("07_RTSP_GET_PARAMETER_request.bin");

        let mut buf = BytesMut::new();
        buf.extend_from_slice(&data1);
        buf.extend_from_slice(&data2);

        let mut codec = RtspCodec;
        let req1 = codec.decode(&mut buf).unwrap().expect("first request");
        let req2 = codec.decode(&mut buf).unwrap().expect("second request");

        assert_eq!(req1.method, "GET");
        assert_eq!(req1.path, "/info");
        assert_eq!(req2.method, "GET_PARAMETER");
        assert!(req2.path.starts_with("rtsp://"));
    }

    #[test]
    fn test_response_to_bytes_rtsp() {
        let resp = RtspResponse::ok(true)
            .header("CSeq", "0")
            .header("Content-Type", "application/x-apple-binary-plist")
            .body(vec![0x62, 0x70, 0x6c, 0x69, 0x73, 0x74]); // "bplist"

        let bytes = resp.to_bytes();
        let text = str::from_utf8(&bytes[..bytes.len() - 6]).unwrap();
        assert!(text.starts_with("RTSP/1.0 200 OK\r\n"));
        assert!(text.contains("CSeq: 0\r\n"));
        assert!(text.contains("Content-Type: application/x-apple-binary-plist\r\n"));
        assert!(text.contains("Content-Length: 6\r\n"));
        assert!(text.ends_with("\r\n\r\n"));
    }

    #[test]
    fn test_response_to_bytes_http() {
        let resp = RtspResponse::ok(false);
        let bytes = resp.to_bytes();
        let text = str::from_utf8(&bytes).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.ends_with("\r\n\r\n"));
    }

    #[test]
    fn test_response_not_implemented() {
        let resp = RtspResponse::not_implemented(true);
        let bytes = resp.to_bytes();
        let text = str::from_utf8(&bytes).unwrap();
        assert!(text.starts_with("RTSP/1.0 501 Not Implemented\r\n"));
    }
}
