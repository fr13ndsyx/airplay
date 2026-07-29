//! 音频重排缓冲 —— 512 槽环形缓冲区，按 RTP 序列号排序后输出连续包。
//!
//! UDP 不保证数据报按序到达，AudioHandler 维护一个 512 槽的环形缓冲区：
//! - `push`：将包存入 `buffer[seq % 512]`，然后从 `prev_seq_num + 1` 开始
//!   向后扫描，连续命中即取出并交给消费者。
//! - 重复或过期（落后于 `prev_seq_num`）的包直接丢弃。
//!
//! 原实现用 `int` 存储序列号，无法正确处理 u16 回绕；本实现使用 `u16`
//! + `wrapping_*` 算术，回绕自然生效。

use super::rtp::AudioPacket;

/// 重排缓冲区槽位数。
const REORDER_BUFFER_SIZE: usize = 512;

/// 音频重排缓冲区。
pub struct AudioReorderBuffer {
    /// 512 槽环形缓冲区，`buffer[seq % 512]` 存放对应序列号的包。
    buffer: Vec<Option<AudioPacket>>,
    /// 上一个成功排空（drain）的序列号。
    prev_seq_num: u16,
    /// `prev_seq_num` 是否已初始化（首个包到来前为 false）。
    initialized: bool,
}

impl AudioReorderBuffer {
    /// 创建新的重排缓冲区（512 槽全为 `None`，未初始化）。
    pub fn new() -> Self {
        let mut buffer = Vec::with_capacity(REORDER_BUFFER_SIZE);
        for _ in 0..REORDER_BUFFER_SIZE {
            buffer.push(None);
        }
        Self {
            buffer,
            prev_seq_num: 0,
            initialized: false,
        }
    }

    /// 压入一个音频包，返回本次排空得到的连续包（按序列号升序）。
    ///
    /// 逻辑：
    /// 1. 首个包：将 `prev_seq_num` 设为 `seq - 1`（使该包成为“下一个”），初始化。
    /// 2. 计算前向距离 `diff = seq.wrapping_sub(prev_seq_num)`：
    ///    - `diff == 0`：重复包，丢弃。
    ///    - `diff >= 0x8000`：过期包（回绕后落后于 `prev_seq_num`），丢弃。
    ///    - 否则：存入 `buffer[seq % 512]`，从 `prev_seq_num + 1` 开始排空。
    pub fn push(&mut self, packet: AudioPacket) -> Vec<AudioPacket> {
        let seq = packet.header.sequence_number;

        // 首个包：把 prev_seq_num 设为 seq-1，使该包成为“下一个期望的包”，随后即可排空。
        if !self.initialized {
            self.prev_seq_num = seq.wrapping_sub(1);
            self.initialized = true;
        }

        let diff = seq.wrapping_sub(self.prev_seq_num);

        // diff == 0 → 重复；diff >= 0x8000 → 回绕后落后于 prev（过期）。两者均丢弃。
        if diff == 0 || diff >= 0x8000 {
            return Vec::new();
        }

        // 存入环形槽（覆盖可能残留的旧包）。
        let slot = (seq as usize) % REORDER_BUFFER_SIZE;
        self.buffer[slot] = Some(packet);

        // 从 prev_seq_num + 1 开始连续排空。
        self.drain_consecutive()
    }

    /// 从 `prev_seq_num + 1` 开始，连续命中即取出，直到遇到空槽。
    fn drain_consecutive(&mut self) -> Vec<AudioPacket> {
        let mut drained = Vec::new();

        let mut next = self.prev_seq_num.wrapping_add(1);
        while let Some(packet) = self.buffer[(next as usize) % REORDER_BUFFER_SIZE].take() {
            self.prev_seq_num = next;
            drained.push(packet);
            next = next.wrapping_add(1);
        }

        drained
    }
}

impl Default for AudioReorderBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::rtp::RtpHeader;

    /// 构造一个指定序列号的音频包（payload 标记序列号便于断言）。
    fn make_packet(seq: u16) -> AudioPacket {
        AudioPacket {
            header: RtpHeader {
                flag: 0x80,
                payload_type: 0x60,
                sequence_number: seq,
                timestamp: seq as u32,
                ssrc: 0xCAFEBABE,
            },
            payload: vec![seq as u8, (seq >> 8) as u8],
        }
    }

    #[test]
    fn test_push_first_packet() {
        let mut buf = AudioReorderBuffer::new();
        let drained = buf.push(make_packet(100));
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].header.sequence_number, 100);
    }

    #[test]
    fn test_push_in_order() {
        let mut buf = AudioReorderBuffer::new();
        let d1 = buf.push(make_packet(10));
        let d2 = buf.push(make_packet(11));
        let d3 = buf.push(make_packet(12));

        assert_eq!(d1.len(), 1);
        assert_eq!(d2.len(), 1);
        assert_eq!(d3.len(), 1);
        assert_eq!(d1[0].header.sequence_number, 10);
        assert_eq!(d2[0].header.sequence_number, 11);
        assert_eq!(d3[0].header.sequence_number, 12);
    }

    #[test]
    fn test_push_out_of_order() {
        let mut buf = AudioReorderBuffer::new();
        // 先推 seq=2（首个包，prev 变为 1，seq=2 立即排空）
        let d1 = buf.push(make_packet(2));
        assert_eq!(d1.len(), 1);
        assert_eq!(d1[0].header.sequence_number, 2);

        // 再推 seq=1：此时 prev=2，seq=1 为过期（diff = 1 - 2 = 0xFFFF >= 0x8000），丢弃
        let d2 = buf.push(make_packet(1));
        assert!(d2.is_empty());

        // 推 seq=3：连续，排空
        let d3 = buf.push(make_packet(3));
        assert_eq!(d3.len(), 1);
        assert_eq!(d3[0].header.sequence_number, 3);
    }

    #[test]
    fn test_push_out_of_order_fill_gap() {
        let mut buf = AudioReorderBuffer::new();
        // 首个包 seq=10 → prev=9，seq=10 排空，prev=10
        let d1 = buf.push(make_packet(10));
        assert_eq!(d1.len(), 1);

        // 推 seq=12：不连续，存入槽 12%512，无排空
        let d2 = buf.push(make_packet(12));
        assert!(d2.is_empty());

        // 推 seq=11：连续，排空 11，然后 12 也连续，一并排空
        let d3 = buf.push(make_packet(11));
        assert_eq!(d3.len(), 2);
        assert_eq!(d3[0].header.sequence_number, 11);
        assert_eq!(d3[1].header.sequence_number, 12);
    }

    #[test]
    fn test_push_duplicate() {
        let mut buf = AudioReorderBuffer::new();
        let _ = buf.push(make_packet(50));
        // 再次推 seq=50：prev=50，diff=0，重复丢弃
        let d = buf.push(make_packet(50));
        assert!(d.is_empty());
    }

    #[test]
    fn test_push_stale_packet_dropped() {
        let mut buf = AudioReorderBuffer::new();
        let _ = buf.push(make_packet(100)); // prev=100
        let _ = buf.push(make_packet(101)); // prev=101
        // 推 seq=99：过期，丢弃
        let d = buf.push(make_packet(99));
        assert!(d.is_empty());
    }

    #[test]
    fn test_push_wrap_around() {
        let mut buf = AudioReorderBuffer::new();
        // 首个包 seq=65535 → prev=65534，排空，prev=65535
        let d1 = buf.push(make_packet(65535));
        assert_eq!(d1.len(), 1);
        assert_eq!(d1[0].header.sequence_number, 65535);

        // 推 seq=0：diff = 0 - 65535 = 1（回绕），连续，排空
        let d2 = buf.push(make_packet(0));
        assert_eq!(d2.len(), 1);
        assert_eq!(d2[0].header.sequence_number, 0);

        // 推 seq=1：连续，排空
        let d3 = buf.push(make_packet(1));
        assert_eq!(d3.len(), 1);
        assert_eq!(d3[0].header.sequence_number, 1);
    }

    #[test]
    fn test_push_wrap_around_with_gap() {
        let mut buf = AudioReorderBuffer::new();
        // 首个包 seq=65535 → prev=65535
        let d1 = buf.push(make_packet(65535));
        assert_eq!(d1.len(), 1);

        // 推 seq=2（跳过 0、1）：不连续，存入，无排空
        let d2 = buf.push(make_packet(2));
        assert!(d2.is_empty());

        // 推 seq=0：连续（回绕），排空 0，但 1 缺失，停止
        let d3 = buf.push(make_packet(0));
        assert_eq!(d3.len(), 1);
        assert_eq!(d3[0].header.sequence_number, 0);

        // 推 seq=1：连续，排空 1，然后 2 也连续，一并排空
        let d4 = buf.push(make_packet(1));
        assert_eq!(d4.len(), 2);
        assert_eq!(d4[0].header.sequence_number, 1);
        assert_eq!(d4[1].header.sequence_number, 2);
    }

    #[test]
    fn test_default_creates_empty_buffer() {
        let buf = AudioReorderBuffer::default();
        assert_eq!(buf.buffer.len(), REORDER_BUFFER_SIZE);
        assert!(buf.buffer.iter().all(|slot| slot.is_none()));
        assert!(!buf.initialized);
    }
}
