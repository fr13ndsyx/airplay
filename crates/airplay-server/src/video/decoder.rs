//! 视频解码器 —— 从 TCP 流中按 128 字节头 + payload 分帧。
//!
//! 帧格式：
//! - 128 字节固定头：offset 0 为 LE u32 payloadSize，offset 4 为 u8 payloadType。
//! - payloadSize 字节 payload。
//!
//! payloadType 语义：
//! - 0：加密 NAL（需 FairPlay 解密 + AVCC→Annex-B 重写）
//! - 1：AVCDecoderConfigurationRecord（含 SPS/PPS）
//! - 5：忽略（解码层直接跳过）

use bytes::BytesMut;
use tokio_util::codec::Decoder;

/// 视频包头长度（固定 128 字节）。
pub const VIDEO_HEADER_LEN: usize = 128;

/// 解析后的视频包。
#[derive(Debug, Clone)]
pub struct VideoPacket {
    /// payload 类型（0 = 加密 NAL，1 = AVCDecoderConfigurationRecord）。
    pub payload_type: u8,
    /// payload 数据（已从流中提取，长度即 payloadSize）。
    pub payload: Vec<u8>,
}

/// 视频帧解码器，实现 `tokio_util::codec::Decoder`。
///
/// 无内部状态：每次 `decode` 调用从缓冲区头部读取一个完整帧。
/// 若 payloadType 不为 0/1（如类型 5），跳过该帧并继续尝试下一帧。
#[derive(Debug, Clone, Default)]
pub struct VideoCodec;

impl Decoder for VideoCodec {
    type Item = VideoPacket;
    type Error = std::io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        loop {
            // 1. 头部不足 128 字节 → 等待更多数据
            if buf.len() < VIDEO_HEADER_LEN {
                return Ok(None);
            }

            // 2. 读取 payloadSize（LE u32）和 payloadType（u8）
            let payload_size =
                u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
            let payload_type = buf[4];

            // 3. 检查 payload 是否完整
            let total_len = VIDEO_HEADER_LEN + payload_size;
            if buf.len() < total_len {
                return Ok(None);
            }

            // 4. 消费头部
            let _ = buf.split_to(VIDEO_HEADER_LEN);

            // 5. 按 payloadType 分发
            if payload_type == 0 || payload_type == 1 {
                // 提取 payload
                let payload = buf.split_to(payload_size).to_vec();
                return Ok(Some(VideoPacket {
                    payload_type,
                    payload,
                }));
            } else {
                // 跳过 payload（如类型 5），继续尝试下一帧
                let _ = buf.split_to(payload_size);
                tracing::debug!(
                    payload_type,
                    payload_size,
                    "video packet skipped"
                );
                // 继续循环以解码下一帧（如果缓冲区中已有数据）
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个 128 字节视频包头。
    fn make_header(payload_size: u32, payload_type: u8) -> Vec<u8> {
        let mut header = vec![0u8; VIDEO_HEADER_LEN];
        header[0..4].copy_from_slice(&payload_size.to_le_bytes());
        header[4] = payload_type;
        header
    }

    #[test]
    fn test_decode_returns_none_when_header_incomplete() {
        let mut buf = BytesMut::from(&vec![0u8; 100][..]); // 不足 128 字节
        let mut codec = VideoCodec;
        assert!(codec.decode(&mut buf).unwrap().is_none());
    }

    #[test]
    fn test_decode_returns_none_when_payload_incomplete() {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&make_header(100, 0));
        buf.extend_from_slice(&vec![0u8; 50]); // 只有 50 字节，需要 100
        let mut codec = VideoCodec;
        assert!(codec.decode(&mut buf).unwrap().is_none());
    }

    #[test]
    fn test_decode_type0_packet() {
        let payload = vec![0x67, 0x42, 0x00, 0x1e, 0x8d];
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&make_header(payload.len() as u32, 0));
        buf.extend_from_slice(&payload);

        let mut codec = VideoCodec;
        let packet = codec.decode(&mut buf).unwrap().expect("should decode");
        assert_eq!(packet.payload_type, 0);
        assert_eq!(packet.payload, payload);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_decode_type1_packet() {
        let payload = vec![0x01, 0x64, 0x00, 0x1e, 0xff, 0xe1];
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&make_header(payload.len() as u32, 1));
        buf.extend_from_slice(&payload);

        let mut codec = VideoCodec;
        let packet = codec.decode(&mut buf).unwrap().expect("should decode");
        assert_eq!(packet.payload_type, 1);
        assert_eq!(packet.payload, payload);
    }

    #[test]
    fn test_decode_skips_type5_packet_and_returns_next() {
        // 类型 5 包（跳过）+ 类型 0 包
        let skip_payload = vec![0xAA, 0xBB, 0xCC];
        let nal_payload = vec![0x67, 0x42];

        let mut buf = BytesMut::new();
        buf.extend_from_slice(&make_header(skip_payload.len() as u32, 5));
        buf.extend_from_slice(&skip_payload);
        buf.extend_from_slice(&make_header(nal_payload.len() as u32, 0));
        buf.extend_from_slice(&nal_payload);

        let mut codec = VideoCodec;
        // 第一次 decode 应跳过类型 5，返回类型 0
        let packet = codec.decode(&mut buf).unwrap().expect("should decode type 0");
        assert_eq!(packet.payload_type, 0);
        assert_eq!(packet.payload, nal_payload);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_decode_multiple_packets_in_one_buffer() {
        let p1 = vec![0x01, 0x02, 0x03];
        let p2 = vec![0x04, 0x05];

        let mut buf = BytesMut::new();
        buf.extend_from_slice(&make_header(p1.len() as u32, 0));
        buf.extend_from_slice(&p1);
        buf.extend_from_slice(&make_header(p2.len() as u32, 1));
        buf.extend_from_slice(&p2);

        let mut codec = VideoCodec;
        let pkt1 = codec.decode(&mut buf).unwrap().expect("first packet");
        let pkt2 = codec.decode(&mut buf).unwrap().expect("second packet");
        assert_eq!(pkt1.payload_type, 0);
        assert_eq!(pkt1.payload, p1);
        assert_eq!(pkt2.payload_type, 1);
        assert_eq!(pkt2.payload, p2);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_decode_split_across_calls() {
        let payload = vec![0x67, 0x42, 0x00, 0x1e, 0x8d];
        let mut header = make_header(payload.len() as u32, 0);
        header.extend_from_slice(&payload);
        let mid = 64; // 在头中间断开

        let mut buf = BytesMut::new();
        let mut codec = VideoCodec;

        // 前半部分 → None
        buf.extend_from_slice(&header[..mid]);
        assert!(codec.decode(&mut buf).unwrap().is_none());

        // 补全 → 解析成功
        buf.extend_from_slice(&header[mid..]);
        let packet = codec.decode(&mut buf).unwrap().expect("should decode");
        assert_eq!(packet.payload_type, 0);
        assert_eq!(packet.payload, payload);
    }
}
