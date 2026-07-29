// RTSP tests: based on step 06/10/17 captures

use airplay_protocol::rtsp::RTSP;
use airplay_protocol::stream_info::MediaStreamInfo;

fn extract_body(data: &[u8]) -> &[u8] {
    if data.starts_with(b"RTSP/") || data.starts_with(b"HTTP/")
        || data.starts_with(b"POST ") || data.starts_with(b"SETUP ")
        || data.starts_with(b"GET ") || data.starts_with(b"ANNOUNCE ")
        || data.starts_with(b"RECORD ") || data.starts_with(b"TEARDOWN ")
        || data.starts_with(b"SET_PARAMETER ") || data.starts_with(b"GET_PARAMETER ")
        || data.starts_with(b"FLUSH ") {
        for i in 0..data.len().saturating_sub(3) {
            if data[i] == b'\r' && data[i+1] == b'\n' && data[i+2] == b'\r' && data[i+3] == b'\n' {
                return &data[i+4..];
            }
        }
    }
    data
}

#[test]
fn test_step6_setup_audio() {
    let request = include_bytes!("fixtures/one_mirroring_app/06_RTSP_SETUP_request.bin");
    let body = extract_body(request);

    let mut rtsp = RTSP::new();
    let result = rtsp.setup(body).expect("setup should succeed");

    // Step 6 is audio SETUP - should contain ekey/eiv or streams
    // If it has ekey/eiv, result is None and rtsp.ekey()/eiv() are set
    // If it has streams, result is Some(MediaStreamInfo::Audio(...))
    match &result {
        None => {
            // ekey/eiv path
            let ekey = rtsp.ekey().expect("ekey should be set");
            let eiv = rtsp.eiv().expect("eiv should be set");
            assert!(!ekey.is_empty(), "ekey should not be empty");
            assert!(!eiv.is_empty(), "eiv should not be empty");
        }
        Some(MediaStreamInfo::Audio(audio)) => {
            // streams path - just verify we got an audio stream
            println!("Audio stream: {:?}", audio);
        }
        Some(MediaStreamInfo::Video(_)) => {
            panic!("step6 should not return a video stream");
        }
    }
}

#[test]
fn test_step10_setup() {
    let request = include_bytes!("fixtures/one_mirroring_app/10_RTSP_SETUP_request.bin");
    let body = extract_body(request);

    let mut rtsp = RTSP::new();
    let result = rtsp.setup(body).expect("setup should succeed");

    // Step 10 SETUP - capture may contain audio or video stream
    match result {
        Some(MediaStreamInfo::Video(video)) => {
            assert!(!video.stream_connection_id.is_empty(), "stream_connection_id should not be empty");
        }
        Some(MediaStreamInfo::Audio(audio)) => {
            println!("step10 returned audio stream: {:?}", audio);
        }
        None => {
            panic!("step10 should return a stream info");
        }
    }
}

#[test]
fn test_step17_teardown() {
    let request = include_bytes!("fixtures/one_mirroring_app/17_RTSP_TEARDOWN_request.bin");
    let body = extract_body(request);

    let mut rtsp = RTSP::new();
    let result = rtsp.teardown(body).expect("teardown should succeed");

    // TEARDOWN may or may not return a stream info
    // Just verify it doesn't crash
    if let Some(info) = result {
        match info {
            MediaStreamInfo::Audio(_) | MediaStreamInfo::Video(_) => {
                // OK
            }
        }
    }
}