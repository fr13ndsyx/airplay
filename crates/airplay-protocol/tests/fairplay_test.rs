//! FairPlay 解密集成测试 —— 复现 Java AirPlayFairPlayTest。
//!
//! 用已知的 fp-setup / RTSP SETUP 输入驱动 AirPlay facade，
//! 然后用返回的 aes_key + 固定 sharedSecret + streamConnectionID 创建 VideoDecryptor，
//! 解密 encrypted_payload，验证 AVCC 长度前缀 == payload.len() - 4。

use std::io::Cursor;

use airplay_protocol::airplay::AirPlay;
use airplay_protocol::video_decryptor::VideoDecryptor;
use plist::Value;

/// Java 字节数组（有符号 byte）转 Rust u8 数组
fn jbytes(b: &[i8]) -> Vec<u8> {
    b.iter().map(|&x| x as u8).collect()
}

#[test]
fn fairplay_decrypt_test() {
    let mut airplay = AirPlay::new();

    // ---- /fp-setup 1 ----
    let fp1_req = jbytes(&[
        70, 80, 76, 89, 3, 1, 1, 0, 0, 0, 0, 4, 2, 0, 0, -69,
    ]);
    let fp1_resp = airplay.fairplay_setup(&fp1_req).expect("fp-setup 1");
    let fp1_expected = jbytes(&[
        70, 80, 76, 89, 3, 1, 2, 0, 0, 0, 0, -126, 2, 0, 15, -97, 63, -98, 10, 37, 33, -37, -33,
        49, 42, -78, -65, -78, -98, -115, 35, 43, 99, 118, -88, -56, 24, 112, 29, 34, -82, -109,
        -40, 39, 55, -2, -81, -99, -76, -3, -12, 28, 45, -70, -99, 31, 73, -54, -86, -65, 101,
        -111, -84, 31, 123, -58, -9, -32, 102, 61, 33, -81, -32, 21, 101, -107, 62, -85, -127,
        -12, 24, -50, -19, 9, 90, -37, 124, 61, 14, 37, 73, 9, -89, -104, 49, -44, -100, 57,
        -126, -105, 52, 52, -6, -53, 66, -58, 58, 28, -39, 17, -90, -2, -108, 26, -118, 109, 74,
        116, 59, 70, -61, -89, 100, -98, 68, -57, -119, 85, -28, -99, -127, 85, 0, -107, 73,
        -60, -30, -9, -93, -10, -43, -70,
    ]);
    assert_eq!(fp1_resp.len(), 142, "fp-setup 1 response length");
    assert_eq!(fp1_resp, fp1_expected, "fp-setup 1 response mismatch");

    // ---- /fp-setup 2 ----
    let fp2_req = jbytes(&[
        70, 80, 76, 89, 3, 1, 3, 0, 0, 0, 0, -104, 0, -113, 26, -100, -40, -92, -10, 52, 109, 20,
        120, 6, -62, -67, -118, 75, -47, -71, -109, -45, -61, 106, -95, 1, 36, -104, -7, 78, -1,
        -13, 70, 123, -49, 27, 49, -104, 98, 92, -94, 69, -114, 62, -48, 30, -35, 53, -25, 41,
        53, 125, -7, 75, -128, -51, 10, -50, 35, 84, -42, -116, -29, 127, 94, 24, -16, -49, -46,
        109, 65, 103, 21, 63, -64, -76, 54, 35, 22, 111, 8, -58, 111, -45, 1, 56, 14, -80, -98,
        -97, -115, -24, 59, -46, -82, -57, -92, 1, -15, -5, -67, -13, 46, 10, -43, 81, -24, 121,
        63, -25, -63, 25, 35, 51, -103, -91, 53, 76, -59, 67, 7, 30, -68, -50, -32, -84, -123,
        34, -82, 27, -85, 51, -44, 65, -60, 120, -11, 99, -50, -3, 66, 117, -5, 85, 90, 58, -29,
        58, -40, -71, -7, -108, -7, -75,
    ]);
    let fp2_resp = airplay.fairplay_setup(&fp2_req).expect("fp-setup 2");
    let fp2_expected = jbytes(&[
        70, 80, 76, 89, 3, 1, 4, 0, 0, 0, 0, 20, -60, 120, -11, 99, -50, -3, 66, 117, -5, 85, 90,
        58, -29, 58, -40, -71, -7, -108, -7, -75,
    ]);
    assert_eq!(fp2_resp.len(), 32, "fp-setup 2 response length");
    assert_eq!(fp2_resp, fp2_expected, "fp-setup 2 response mismatch");

    // ---- RTSP SETUP 1: ekey + eiv ----
    let encrypted_aes_key = jbytes(&[
        70, 80, 76, 89, 1, 2, 1, 0, 0, 0, 0, 60, 0, 0, 0, 0, 63, 121, 70, -69, 3, -8, 117, -13,
        83, 72, 105, -51, -11, -43, -1, 17, 0, 0, 0, 16, 24, -109, 13, 105, -32, -125, -73, -128,
        21, 29, -31, 72, -41, 112, -36, -75, 57, 110, 71, -72, -25, -59, 102, 22, 19, -43, 35,
        74, -20, 86, 15, 16, 126, 5, 15, -45,
    ]);
    let eiv = b"91IdM6RTh4keicMei2GfQA==".to_vec();

    // 构造 plist: { ekey: <data>, eiv: <data> }
    let mut dict = plist::Dictionary::new();
    dict.insert("ekey".into(), Value::Data(encrypted_aes_key.clone()));
    dict.insert("eiv".into(), Value::Data(eiv.clone()));
    let plist_val = Value::Dictionary(dict);
    let mut buf = Vec::new();
    plist_val.to_writer_binary(&mut buf).expect("write plist");
    airplay.rtsp_setup(&buf).expect("rtsp setup 1");

    // ---- RTSP SETUP 2: streamConnectionID ----
    let stream_connection_id: i64 = -3907568444900622110;
    let stream_connection_id_unsigned = stream_connection_id as u64; // 14939185628808929506

    let mut stream_dict = plist::Dictionary::new();
    stream_dict.insert("type".into(), Value::Integer(110.into()));
    stream_dict.insert(
        "streamConnectionID".into(),
        Value::Integer(stream_connection_id.into()),
    );
    let streams = vec![Value::Dictionary(stream_dict)];
    let mut setup2_dict = plist::Dictionary::new();
    setup2_dict.insert("streams".into(), Value::Array(streams));
    let setup2_val = Value::Dictionary(setup2_dict);
    let mut buf2 = Vec::new();
    setup2_val.to_writer_binary(&mut buf2).expect("write plist 2");
    airplay.rtsp_setup(&buf2).expect("rtsp setup 2");

    // ---- 获取 aes_key ----
    let aes_key = airplay
        .get_fairplay_aes_key()
        .expect("get aes key");
    println!("aes_key: {:02X?}", aes_key);

    // ---- sharedSecret ----
    let shared_secret: [u8; 32] = [
        251, 189, 152, 31, 49, 40, 180, 40, 140, 105, 45, 209, 125, 162, 117, 152, 202, 209, 206,
        6, 122, 1, 218, 142, 168, 171, 128, 2, 116, 137, 166, 123,
    ];

    let conn_id_str = stream_connection_id_unsigned.to_string();
    println!("stream_connection_id: {}", conn_id_str);

    // ---- 创建 VideoDecryptor ----
    let mut decryptor =
        VideoDecryptor::new(&aes_key, &shared_secret, &conn_id_str).expect("create decryptor");

    // ---- 读取 encrypted_payload ----
    let payload_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../encrypted_payload");
    let payload = std::fs::read(payload_path).expect("read encrypted_payload");
    println!("payload size: {} bytes", payload.len());

    // ---- 解密 ----
    let mut payload_mut = payload.clone();
    decryptor.decrypt(&mut payload_mut);

    // ---- 验证 AVCC 长度前缀 ----
    let nc_len = u32::from_be_bytes([
        payload_mut[0],
        payload_mut[1],
        payload_mut[2],
        payload_mut[3],
    ]) as usize;
    println!(
        "decrypted first 4 bytes: {:02X?} -> nc_len = {}",
        &payload_mut[0..4],
        nc_len
    );
    println!("expected nc_len = {}", payload_mut.len() - 4);

    assert_eq!(
        nc_len,
        payload_mut.len() - 4,
        "Decrypted payload is corrupted! nc_len != payload.len() - 4"
    );
}
