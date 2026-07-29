//! 集成测试 —— 验证 airplay-player 公共 API 端到端流程。
//!
//! 测试内容：
//! 1. GstPlayer 创建 + consumer 创建
//! 2. AirPlayConsumer trait 方法调用（模拟数据流）
//! 3. GstPlayerConsumer 与 AirPlayServer 集成

use std::sync::Arc;
use std::time::Duration;

use airplay_player::GstPlayer;
use airplay_protocol::stream_info::{
    AudioStreamInfo, CompressionType, VideoStreamInfo,
};
use airplay_server::consumer::AirPlayConsumer;

/// 测试 GstPlayer 创建和 consumer 获取。
#[test]
fn test_player_and_consumer_creation() {
    let player = GstPlayer::new().expect("create GstPlayer");
    let _consumer = player.consumer();
    // player drop 时自动 shutdown
}

/// 测试 GstPlayerConsumer 实现 AirPlayConsumer trait。
#[test]
fn test_consumer_implements_trait() {
    let player = GstPlayer::new().expect("create GstPlayer");
    let consumer: Arc<dyn AirPlayConsumer> = Arc::new(player.consumer());
    // 可以作为 Arc<dyn AirPlayConsumer> 使用
    drop(consumer);
}

/// 测试完整的视频数据流：on_video_format → on_video → on_video_src_disconnect。
#[tokio::test]
async fn test_video_data_flow() {
    let player = GstPlayer::new().expect("create GstPlayer");
    let consumer = player.consumer();

    // 1. 模拟 SETUP video 响应
    let video_info = VideoStreamInfo::new("integration-test".into());
    consumer.on_video_format(video_info).await;

    // 2. 模拟视频数据推送（SPS NAL）
    let fake_h264 = vec![
        0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x0a, 0xf8, 0x41, 0xa2,
    ];
    consumer.on_video(&fake_h264).await;

    // 3. 等待 GStreamer 线程处理
    tokio::time::sleep(Duration::from_millis(150)).await;

    // 4. 模拟 TEARDOWN
    consumer.on_video_src_disconnect().await;
    tokio::time::sleep(Duration::from_millis(50)).await;
}

/// 测试完整的音频数据流：on_audio_format → on_audio → on_audio_src_disconnect。
#[tokio::test]
async fn test_audio_data_flow_alac() {
    let player = GstPlayer::new().expect("create GstPlayer");
    let consumer = player.consumer();

    // 1. 模拟 SETUP audio 响应（ALAC）
    let audio_info = AudioStreamInfo::builder()
        .compression_type(CompressionType::Alac)
        .build();
    consumer.on_audio_format(audio_info).await;

    // 2. 模拟音频数据推送
    let fake_audio = vec![0x00u8; 32];
    consumer.on_audio(&fake_audio).await;

    tokio::time::sleep(Duration::from_millis(150)).await;

    // 3. 模拟 TEARDOWN
    consumer.on_audio_src_disconnect().await;
    tokio::time::sleep(Duration::from_millis(50)).await;
}

/// 测试 AAC-ELD 音频数据流。
#[tokio::test]
async fn test_audio_data_flow_aac_eld() {
    let player = GstPlayer::new().expect("create GstPlayer");
    let consumer = player.consumer();

    let audio_info = AudioStreamInfo::builder()
        .compression_type(CompressionType::AacEld)
        .build();
    consumer.on_audio_format(audio_info).await;

    let fake_audio = vec![0xFFu8; 16];
    consumer.on_audio(&fake_audio).await;

    tokio::time::sleep(Duration::from_millis(150)).await;
    consumer.on_audio_src_disconnect().await;
    tokio::time::sleep(Duration::from_millis(50)).await;
}

/// 测试 playback_info 返回默认值。
#[tokio::test]
async fn test_playback_info_default() {
    let player = GstPlayer::new().expect("create GstPlayer");
    let consumer = player.consumer();

    let info = consumer.playback_info().await;
    assert_eq!(info.duration, 0.0);
    assert_eq!(info.position, 0.0);
}

/// 测试 GstPlayerConsumer 可以克隆 sender（多个 consumer 共享一个 player）。
#[tokio::test]
async fn test_multiple_consumers() {
    let player = GstPlayer::new().expect("create GstPlayer");
    let consumer1 = player.consumer();
    let consumer2 = player.consumer();

    // 两个 consumer 都能推送数据
    consumer1.on_video(&[0x00, 0x01, 0x02]).await;
    consumer2.on_video(&[0x03, 0x04, 0x05]).await;

    tokio::time::sleep(Duration::from_millis(100)).await;
}

/// 测试与 AirPlayServer 的集成（创建服务器但不启动）。
#[tokio::test]
async fn test_server_integration() {
    use airplay_server::config::AirPlayConfig;
    use airplay_server::server::AirPlayServer;

    let player = GstPlayer::new().expect("create GstPlayer");
    let consumer: Arc<dyn AirPlayConsumer> = Arc::new(player.consumer());

    let config = AirPlayConfig::default();
    let _server = AirPlayServer::new(config, consumer);
    // 不调用 start()，仅验证类型兼容性
}
