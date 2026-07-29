// plist_util tests: based on step 01/12 captures

use airplay_protocol::plist_util::{AirPlayConfig, prepare_info_response, prepare_server_info_response};
use plist::Value;
use std::io::Cursor;

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
fn test_parse_info_response() {
    let response = include_bytes!("fixtures/one_mirroring_app/01_RTSP_GET_info_response.bin");
    let body = extract_body(response);

    // Parse the binary plist
    let value = Value::from_reader(Cursor::new(body)).expect("should parse info response plist");
    let dict = value.as_dictionary().expect("info response should be a dictionary");

    // Verify key fields (note: capture uses "audioFormat" singular, not "audioFormats" plural)
    assert!(dict.contains_key("audioFormat"), "should contain audioFormat");
    assert!(dict.contains_key("displays"), "should contain displays");
    assert!(dict.contains_key("features"), "should contain features");
    assert!(dict.contains_key("model"), "should contain model");
    assert!(dict.contains_key("name"), "should contain name");
    assert!(dict.contains_key("sourceVersion"), "should contain sourceVersion");

    // Verify model is "AppleTV3,2"
    let model = dict.get("model").unwrap().as_string().unwrap();
    assert_eq!(model, "AppleTV3,2");

    // Verify name (capture is from macOS AirPlay Receiver, not actual Apple TV)
    let name = dict.get("name").unwrap().as_string().unwrap();
    assert_eq!(name, "Airplay Receiver");
}

#[test]
fn test_prepare_info_response_structure() {
    let config = AirPlayConfig {
        width: 1920,
        height: 1080,
        fps: 60.0,
    };

    let bytes = prepare_info_response(&config).expect("prepare_info_response should succeed");

    // Parse the generated binary plist
    let value = Value::from_reader(Cursor::new(&bytes[..])).expect("should parse generated plist");
    let dict = value.as_dictionary().expect("generated plist should be a dictionary");

    // Verify key fields exist
    assert!(dict.contains_key("audioFormats"));
    assert!(dict.contains_key("displays"));
    assert!(dict.contains_key("features"));
    assert!(dict.contains_key("model"));
    assert_eq!(dict.get("model").unwrap().as_string().unwrap(), "AppleTV3,2");
    assert_eq!(dict.get("name").unwrap().as_string().unwrap(), "Apple TV");

    // Verify displays contains width/height
    let displays = dict.get("displays").unwrap().as_array().unwrap();
    assert_eq!(displays.len(), 1);
    let display = displays[0].as_dictionary().unwrap();
    assert_eq!(display.get("width").unwrap().as_signed_integer().unwrap(), 1920);
    assert_eq!(display.get("height").unwrap().as_signed_integer().unwrap(), 1080);
}

#[test]
fn test_prepare_server_info_response() {
    let bytes = prepare_server_info_response();

    // Should be valid XML plist
    let value = Value::from_reader(Cursor::new(&bytes[..])).expect("should parse server info plist");
    let dict = value.as_dictionary().expect("server info should be a dictionary");

    assert!(dict.contains_key("features"));
    assert!(dict.contains_key("srcvers"));
    assert!(dict.contains_key("protovers"));
}