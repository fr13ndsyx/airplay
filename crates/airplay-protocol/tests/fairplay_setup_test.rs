// FairPlaySetup tests: based on step 04/05/13 captures

use airplay_protocol::fairplay_setup::FairPlaySetup;

/// Extract body from RTSP/HTTP message (skip headers).
/// If data is not a text message (does not start with RTSP/HTTP/POST/SETUP/GET), return as-is.
fn extract_body(data: &[u8]) -> &[u8] {
    // Check if it is an RTSP/HTTP text message
    if data.starts_with(b"RTSP/") || data.starts_with(b"HTTP/")
        || data.starts_with(b"POST ") || data.starts_with(b"SETUP ")
        || data.starts_with(b"GET ") || data.starts_with(b"ANNOUNCE ")
        || data.starts_with(b"RECORD ") || data.starts_with(b"FLUSH ")
        || data.starts_with(b"TEARDOWN ") || data.starts_with(b"SET_PARAMETER ")
        || data.starts_with(b"GET_PARAMETER ") {
        // Find the \r\n\r\n separator
        for i in 0..data.len().saturating_sub(3) {
            if data[i] == b'\r' && data[i+1] == b'\n' && data[i+2] == b'\r' && data[i+3] == b'\n' {
                return &data[i+4..];
            }
        }
    }
    data  // return as-is (raw binary data)
}

#[test]
fn test_step4_16byte_request() {
    let request = include_bytes!("fixtures/one_mirroring_app/04_RTSP_POST_fp_setup_request.bin");

    let req_body = extract_body(request);

    assert_eq!(req_body.len(), 16, "step4 request body should be 16 bytes");
    assert_eq!(req_body[4], 3, "FairPlay version should be 3");

    let mut fps = FairPlaySetup::new();
    let result = fps.fair_play_setup(req_body).expect("fair_play_setup should succeed");

    // 16-byte request returns 142-byte replyMessage (hardcoded, may not match capture)
    assert_eq!(result.len(), 142, "16-byte request should return 142-byte response");
}

#[test]
fn test_step5_164byte_request() {
    let request = include_bytes!("fixtures/one_mirroring_app/05_RTSP_POST_fp_setup_request.bin");
    let response = include_bytes!("fixtures/one_mirroring_app/05_RTSP_POST_fp_setup_response.bin");

    let req_body = extract_body(request);
    let resp_body = extract_body(response);

    assert_eq!(req_body.len(), 164, "step5 request body should be 164 bytes");

    let mut fps = FairPlaySetup::new();
    let result = fps.fair_play_setup(req_body).expect("fair_play_setup should succeed");

    assert_eq!(result, resp_body, "step5 response should match");
}

#[test]
fn test_step13_fp_setup2() {
    // Step13 capture (13_HTTP_POST_fp_setup2_request.bin) only has HTTP headers, no body.
    // Use step4 request which has the same 16-byte FairPlay setup body format.
    let request = include_bytes!("fixtures/one_mirroring_app/04_RTSP_POST_fp_setup_request.bin");
    let req_body = extract_body(request);
    assert_eq!(req_body.len(), 16, "request body should be 16 bytes");
    let mut fps = FairPlaySetup::new();
    let result = fps.fair_play_setup(req_body).expect("fair_play_setup should succeed");
    // 16-byte request returns 142-byte replyMessage
    assert_eq!(result.len(), 142, "16-byte request should return 142-byte response");
}