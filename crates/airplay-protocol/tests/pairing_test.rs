// Pairing tests: based on step 02/03 captures

use airplay_protocol::pairing::Pairing;

fn extract_body(data: &[u8]) -> &[u8] {
    if data.starts_with(b"RTSP/") || data.starts_with(b"HTTP/")
        || data.starts_with(b"POST ") || data.starts_with(b"SETUP ")
        || data.starts_with(b"GET ") || data.starts_with(b"ANNOUNCE ") {
        for i in 0..data.len().saturating_sub(3) {
            if data[i] == b'\r' && data[i+1] == b'\n' && data[i+2] == b'\r' && data[i+3] == b'\n' {
                return &data[i+4..];
            }
        }
    }
    data
}

#[test]
fn test_step1_request_parsing() {
    let request = include_bytes!("fixtures/one_mirroring_app/02_RTSP_POST_pair_verify_request.bin");
    let body = extract_body(request);

    // pair-verify step1 request: 1 byte flag + 3 bytes skip + 32 bytes ecdh + 32 bytes ed = 68 bytes
    assert!(body.len() >= 68, "step1 body should be at least 68 bytes, got {}", body.len());

    let flag = body[0];
    assert!(flag > 0, "step1 flag should be > 0, got {}", flag);

    let ecdh_theirs = &body[4..36];
    let ed_theirs = &body[36..68];
    assert_eq!(ecdh_theirs.len(), 32);
    assert_eq!(ed_theirs.len(), 32);

    // Verify not all zeros
    assert!(ecdh_theirs.iter().any(|&b| b != 0), "ecdh_theirs should not be all zeros");
    assert!(ed_theirs.iter().any(|&b| b != 0), "ed_theirs should not be all zeros");
}

#[test]
fn test_step1_response_structure() {
    let request = include_bytes!("fixtures/one_mirroring_app/02_RTSP_POST_pair_verify_request.bin");
    let body = extract_body(request);

    let mut pairing = Pairing::new();
    let response = pairing.pair_verify_step1(body).expect("pair_verify_step1 should succeed");

    // Response: 32 bytes ecdhOurs + 64 bytes encryptedSignature = 96 bytes
    assert_eq!(response.len(), 96, "step1 response should be 96 bytes, got {}", response.len());

    // First 32 bytes should be the server's ecdh public key (not all zeros)
    let ecdh_ours = &response[..32];
    assert!(ecdh_ours.iter().any(|&b| b != 0), "ecdh_ours should not be all zeros");
}

#[test]
fn test_step2_request_parsing() {
    let request = include_bytes!("fixtures/one_mirroring_app/03_RTSP_POST_pair_verify_request.bin");
    let body = extract_body(request);

    // pair-verify step2 request: 1 byte flag + 3 bytes skip + 64 bytes signature = 68 bytes
    assert!(body.len() >= 68, "step2 body should be at least 68 bytes, got {}", body.len());

    let flag = body[0];
    assert_eq!(flag, 0, "step2 flag should be 0, got {}", flag);
}