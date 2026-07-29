//! RTP 解析 —— 从 UDP 数据报中解析 RTP 头部与音频 payload。
//!
//! RTP 头部格式（12 字节，大端）：
//! - offset 0: `flag` — u8（V/P/X/CC 字节）
//! - offset 1: `type` — u8（M/PT 字节，payload type 用 0x7F 掩码）
//! - offset 2-3: `sequence_number` — 大端 u16
//! - offset 4-7: `timestamp` — 大端 u32
//! - offset 8-11: `ssrc` — 大端 u32
//! - 之后：编码后的音频 payload（剩余字节）
//!
//! 注：原实现 SSRC 解析偏移量错误（headerBytes[6] 而非
//! headerBytes[10]），已修正为正确读取 offset 8-11 的 4 字节。

/// RTP 头部长度（固定 12 字节）。
pub const RTP_HEADER_LEN: usize = 12;

/// RTP payload type 掩码（低 7 位）。
pub const RTP_PAYLOAD_TYPE_MASK: u8 = 0x7F;

/// 解析后的 RTP 头部。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtpHeader {
    /// V/P/X/CC 字节（offset 0）。
    pub flag: u8,
    /// payload type（offset 1 低 7 位，已应用 0x7F 掩码）。
    pub payload_type: u8,
    /// 序列号（offset 2-3，大端 u16）。
    pub sequence_number: u16,
    /// 时间戳（offset 4-7，大端 u32）。
    pub timestamp: u32,
    /// 同步源标识（offset 8-11，大端 u32）。
    pub ssrc: u32,
}

/// 解析后的音频包：RTP 头部 + payload。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioPacket {
    /// RTP 头部。
    pub header: RtpHeader,
    /// 音频 payload（头之后的所有字节）。
    pub payload: Vec<u8>,
}

/// 从 UDP 数据报解析 12 字节 RTP 头部。
///
/// 数据不足 12 字节时返回 `None`。
///
/// 注：已修正 SSRC 偏移量 bug（使用 offset 8-11 而非错误的 6）。
pub fn parse_rtp_header(packet: &[u8]) -> Option<RtpHeader> {
    if packet.len() < RTP_HEADER_LEN {
        return None;
    }

    let flag = packet[0];
    let payload_type = packet[1] & RTP_PAYLOAD_TYPE_MASK;
    let sequence_number = u16::from_be_bytes([packet[2], packet[3]]);
    let timestamp = u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]);
    // 修正：正确读取 offset 8-11
    let ssrc = u32::from_be_bytes([packet[8], packet[9], packet[10], packet[11]]);

    Some(RtpHeader {
        flag,
        payload_type,
        sequence_number,
        timestamp,
        ssrc,
    })
}

/// 从 UDP 数据报解析 RTP 头部并提取 payload。
///
/// 数据不足 12 字节时返回 `None`。payload 为头之后的所有剩余字节。
pub fn parse_audio_packet(packet: &[u8]) -> Option<AudioPacket> {
    let header = parse_rtp_header(packet)?;
    let payload = packet[RTP_HEADER_LEN..].to_vec();
    Some(AudioPacket { header, payload })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个 12 字节 RTP 头部。
    fn make_header(
        flag: u8,
        payload_type: u8,
        sequence_number: u16,
        timestamp: u32,
        ssrc: u32,
    ) -> Vec<u8> {
        let mut buf = Vec::with_capacity(RTP_HEADER_LEN);
        buf.push(flag);
        buf.push(payload_type);
        buf.extend_from_slice(&sequence_number.to_be_bytes());
        buf.extend_from_slice(&timestamp.to_be_bytes());
        buf.extend_from_slice(&ssrc.to_be_bytes());
        buf
    }

    #[test]
    fn test_parse_rtp_header_basic() {
        let packet = make_header(0x80, 0x60, 0x1234, 0xDEADBEEF, 0xCAFEBABE);
        let header = parse_rtp_header(&packet).expect("should parse header");

        assert_eq!(header.flag, 0x80);
        assert_eq!(header.payload_type, 0x60);
        assert_eq!(header.sequence_number, 0x1234);
        assert_eq!(header.timestamp, 0xDEADBEEF);
        assert_eq!(header.ssrc, 0xCAFEBABE);
    }

    #[test]
    fn test_parse_rtp_header_payload_type_mask() {
        // 高位（M 标志位）应被剥离，只保留低 7 位
        let packet = make_header(0x80, 0xE0, 0x0001, 0x0, 0x0);
        let header = parse_rtp_header(&packet).expect("should parse header");
        assert_eq!(header.payload_type, 0x60); // 0xE0 & 0x7F == 0x60
    }

    #[test]
    fn test_parse_rtp_header_short_packet() {
        let packet = vec![0u8; 11]; // 不足 12 字节
        assert!(parse_rtp_header(&packet).is_none());
    }

    #[test]
    fn test_parse_rtp_header_empty_packet() {
        let packet: Vec<u8> = Vec::new();
        assert!(parse_rtp_header(&packet).is_none());
    }

    #[test]
    fn test_parse_rtp_header_exactly_12_bytes() {
        let packet = make_header(0x90, 0x00, 0xFFFF, 0x00000001, 0x00000002);
        let header = parse_rtp_header(&packet).expect("should parse header");
        assert_eq!(header.flag, 0x90);
        assert_eq!(header.payload_type, 0x00);
        assert_eq!(header.sequence_number, 0xFFFF);
        assert_eq!(header.timestamp, 0x00000001);
        assert_eq!(header.ssrc, 0x00000002);
    }

    #[test]
    fn test_parse_audio_packet_with_payload() {
        let mut packet = make_header(0x80, 0x60, 0x0001, 0x100, 0x200);
        packet.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);

        let audio = parse_audio_packet(&packet).expect("should parse audio packet");
        assert_eq!(audio.header.flag, 0x80);
        assert_eq!(audio.header.payload_type, 0x60);
        assert_eq!(audio.header.sequence_number, 0x0001);
        assert_eq!(audio.header.timestamp, 0x100);
        assert_eq!(audio.header.ssrc, 0x200);
        assert_eq!(audio.payload, vec![0xAA, 0xBB, 0xCC, 0xDD]);
    }

    #[test]
    fn test_parse_audio_packet_empty_payload() {
        let packet = make_header(0x80, 0x60, 0x0001, 0x100, 0x200);
        let audio = parse_audio_packet(&packet).expect("should parse audio packet");
        assert!(audio.payload.is_empty());
    }

    #[test]
    fn test_parse_audio_packet_short_packet() {
        let packet = vec![0u8; 5];
        assert!(parse_audio_packet(&packet).is_none());
    }

    #[test]
    fn test_ssrc_uses_correct_offset() {
        // 显式验证 SSRC 读取自 offset 8-11（修正原实现的 SSRC 读取 bug）
        // 构造一个 timestamp 与 ssrc 字节不同的包，确保不会被误读
        let mut packet = vec![0u8; RTP_HEADER_LEN];
        packet[4] = 0x11; // timestamp 字节
        packet[5] = 0x22;
        packet[6] = 0x33;
        packet[7] = 0x44;
        packet[8] = 0x55; // ssrc 字节
        packet[9] = 0x66;
        packet[10] = 0x77;
        packet[11] = 0x88;

        let header = parse_rtp_header(&packet).expect("should parse header");
        assert_eq!(header.timestamp, 0x11223344);
        assert_eq!(header.ssrc, 0x55667788);
    }
}
