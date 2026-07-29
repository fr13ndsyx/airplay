//! NAL 单元重写 —— AVCC 长度前缀格式 ↔ Annex-B 起始码格式。

/// Annex-B 起始码：`00 00 00 01`。
const ANNEXB_START_CODE: [u8; 4] = [0x00, 0x00, 0x00, 0x01];

/// 将 AVCC 格式的 NAL 单元（4 字节大端长度前缀）原地重写为 Annex-B 格式（4 字节起始码）。
///
/// AVCC → Annex-B 重写。
///
/// **修正原实现 bug**：长度判断应为 > 4，
/// 原实现误用 `naluSize`（当前 NAL 长度）而非 `idx`（当前偏移），导致多 NAL 包被错误截断。
/// 本实现使用 `while idx + 4 <= payload.len()` 循环条件正确处理。
///
/// # 算法
/// 1. 读取 4 字节大端 NAL 长度。
/// 2. 若长度 == 1（哨兵 `00 00 00 01`），返回（结束标记）。
/// 3. 若长度 == 0，跳出（避免死循环）。
/// 4. 将 4 字节长度前缀原地替换为 `00 00 00 01` 起始码。
/// 5. 前进 `nalu_size + 4` 字节，处理下一个 NAL。
pub fn rewrite_avcc_to_annexb(payload: &mut [u8]) {
    let mut idx = 0;
    while idx + 4 <= payload.len() {
        // 读取 4 字节大端 NAL 单元长度
        let nalu_size = u32::from_be_bytes([
            payload[idx],
            payload[idx + 1],
            payload[idx + 2],
            payload[idx + 3],
        ]);

        if nalu_size == 1 {
            // 哨兵 / 结束标记（00 00 00 01）
            return;
        }
        if nalu_size == 0 {
            // 避免死循环
            break;
        }

        // 将 4 字节长度前缀替换为 Annex-B 起始码（00 00 00 01）
        payload[idx] = 0;
        payload[idx + 1] = 0;
        payload[idx + 2] = 0;
        payload[idx + 3] = 1;

        idx += nalu_size as usize + 4;
    }
}

/// 从 AVCDecoderConfigurationRecord 中提取 SPS 和 PPS，返回 Annex-B 格式。
///
/// 提取 SPS/PPS。
///
/// # 格式
/// ```text
/// Offset 0-5:   配置头（6 字节，跳过）
/// Offset 6-7:   SPS 长度（大端 u16）
/// Offset 8..8+sps_len: SPS NAL 字节
/// Offset 8+sps_len: PPS 计数字节（跳过）
/// Offset 9+sps_len..10+sps_len: PPS 长度（大端 u16）
/// Offset 11+sps_len..11+sps_len+pps_len: PPS NAL 字节
/// ```
///
/// # 返回
/// `[00 00 00 01][SPS][00 00 00 01][PPS]`，若数据不足返回 `None`。
pub fn extract_sps_pps(payload: &[u8]) -> Option<Vec<u8>> {
    // 至少需要 6 字节配置头 + 2 字节 SPS 长度
    if payload.len() < 8 {
        return None;
    }

    let sps_len = u16::from_be_bytes([payload[6], payload[7]]) as usize;

    // 检查 SPS 数据是否完整
    let sps_start = 8;
    let sps_end = sps_start + sps_len;
    if payload.len() < sps_end + 1 {
        // sps_end + 1 为 PPS 计数字节
        return None;
    }

    // PPS 计数字节在 sps_end，跳过；之后 2 字节为 PPS 长度
    let pps_len_offset = sps_end + 1;
    if payload.len() < pps_len_offset + 2 {
        return None;
    }
    let pps_len = u16::from_be_bytes([payload[pps_len_offset], payload[pps_len_offset + 1]]) as usize;

    let pps_start = pps_len_offset + 2;
    let pps_end = pps_start + pps_len;
    if payload.len() < pps_end {
        return None;
    }

    // 构造输出：[start_code][SPS][start_code][PPS]
    let mut out = Vec::with_capacity(4 + sps_len + 4 + pps_len);
    out.extend_from_slice(&ANNEXB_START_CODE);
    out.extend_from_slice(&payload[sps_start..sps_end]);
    out.extend_from_slice(&ANNEXB_START_CODE);
    out.extend_from_slice(&payload[pps_start..pps_end]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rewrite_avcc_to_annexb_single_nal() {
        // 输入：[长度前缀 00 00 00 05][5 字节 NAL]
        let mut payload = vec![0x00, 0x00, 0x00, 0x05, 0x67, 0x42, 0x00, 0x1e, 0x8d];
        rewrite_avcc_to_annexb(&mut payload);

        // 前 4 字节应为 Annex-B 起始码
        assert_eq!(&payload[0..4], &[0x00, 0x00, 0x00, 0x01]);
        // NAL 数据不变
        assert_eq!(&payload[4..9], &[0x67, 0x42, 0x00, 0x1e, 0x8d]);
    }

    #[test]
    fn test_rewrite_avcc_to_annexb_multiple_nals() {
        // 输入：两个 NAL，各带 4 字节长度前缀
        // NAL1: 长度 5，数据 [0x67, 0x42, 0x00, 0x1e, 0x8d]
        // NAL2: 长度 3，数据 [0x68, 0xce, 0x38]
        let mut payload = vec![
            0x00, 0x00, 0x00, 0x05, 0x67, 0x42, 0x00, 0x1e, 0x8d, // NAL1
            0x00, 0x00, 0x00, 0x03, 0x68, 0xce, 0x38, // NAL2
        ];
        rewrite_avcc_to_annexb(&mut payload);

        // 第一个起始码
        assert_eq!(&payload[0..4], &[0x00, 0x00, 0x00, 0x01]);
        // NAL1 数据
        assert_eq!(&payload[4..9], &[0x67, 0x42, 0x00, 0x1e, 0x8d]);
        // 第二个起始码（偏移 9）
        assert_eq!(&payload[9..13], &[0x00, 0x00, 0x00, 0x01]);
        // NAL2 数据
        assert_eq!(&payload[13..16], &[0x68, 0xce, 0x38]);
    }

    #[test]
    fn test_rewrite_avcc_to_annexb_sentinel() {
        // 输入：NAL + 哨兵 00 00 00 01
        let mut payload = vec![
            0x00, 0x00, 0x00, 0x05, 0x67, 0x42, 0x00, 0x1e, 0x8d, // NAL
            0x00, 0x00, 0x00, 0x01, // 哨兵
        ];
        rewrite_avcc_to_annexb(&mut payload);

        // 第一个前缀被重写为起始码
        assert_eq!(&payload[0..4], &[0x00, 0x00, 0x00, 0x01]);
        // 哨兵保持不变（已经是 00 00 00 01）
        assert_eq!(&payload[9..13], &[0x00, 0x00, 0x00, 0x01]);
        // 不应死循环（测试能完成即通过）
    }

    #[test]
    fn test_rewrite_avcc_to_annexb_empty() {
        let mut payload: Vec<u8> = vec![];
        rewrite_avcc_to_annexb(&mut payload);
        assert!(payload.is_empty());
    }

    #[test]
    fn test_rewrite_avcc_to_annexb_zero_size_breaks() {
        // 长度为 0 → 跳出，避免死循环
        let mut payload = vec![0x00, 0x00, 0x00, 0x00, 0x01, 0x02];
        rewrite_avcc_to_annexb(&mut payload);
        // 前 4 字节不变（0 长度，不重写）
        assert_eq!(&payload[0..4], &[0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_extract_sps_pps() {
        // 构造最小 AVCDecoderConfigurationRecord：
        // [6 字节配置头][2 字节 SPS 长度][SPS][1 字节 PPS 计数][2 字节 PPS 长度][PPS]
        let sps = vec![0x67, 0x42, 0x00, 0x1e]; // 4 字节 SPS
        let pps = vec![0x68, 0xce, 0x38]; // 3 字节 PPS

        let mut config = vec![0x01, 0x64, 0x00, 0x1e, 0xff, 0xe1]; // 6 字节头
        config.extend_from_slice(&(sps.len() as u16).to_be_bytes()); // SPS 长度
        config.extend_from_slice(&sps);
        config.push(0x01); // PPS 计数
        config.extend_from_slice(&(pps.len() as u16).to_be_bytes()); // PPS 长度
        config.extend_from_slice(&pps);

        let result = extract_sps_pps(&config).expect("should extract");

        // 期望输出：[00 00 00 01][SPS][00 00 00 01][PPS]
        let mut expected = vec![];
        expected.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        expected.extend_from_slice(&sps);
        expected.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        expected.extend_from_slice(&pps);

        assert_eq!(result, expected);
    }

    #[test]
    fn test_extract_sps_pps_too_short() {
        // 不足 8 字节
        assert!(extract_sps_pps(&[0x01, 0x02, 0x03]).is_none());
    }

    #[test]
    fn test_extract_sps_pps_truncated_sps() {
        // SPS 长度声明 10 但实际只有 2 字节
        let mut config = vec![0x01, 0x64, 0x00, 0x1e, 0xff, 0xe1];
        config.extend_from_slice(&10u16.to_be_bytes()); // SPS 长度 = 10
        config.extend_from_slice(&[0x67, 0x42]); // 只有 2 字节
        assert!(extract_sps_pps(&config).is_none());
    }

    #[test]
    fn test_extract_sps_pps_truncated_pps() {
        let sps = vec![0x67, 0x42];
        let mut config = vec![0x01, 0x64, 0x00, 0x1e, 0xff, 0xe1];
        config.extend_from_slice(&(sps.len() as u16).to_be_bytes());
        config.extend_from_slice(&sps);
        config.push(0x01);
        config.extend_from_slice(&10u16.to_be_bytes()); // PPS 长度 = 10
        config.extend_from_slice(&[0x68]); // 只有 1 字节
        assert!(extract_sps_pps(&config).is_none());
    }
}
