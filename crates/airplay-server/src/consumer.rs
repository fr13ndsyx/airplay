//! 消费者接口。
//!
//! 业务方实现此 trait 来接收解密后的视频/音频数据。
//! 使用 `async_trait` 以便后续集成 GStreamer 等异步管线。

use async_trait::async_trait;

use airplay_protocol::stream_info::{AudioStreamInfo, VideoStreamInfo};

/// AirPlay 媒体消费者。
///
/// 6 个核心回调方法 + 5 个默认实现（HLS / YouTube 相关，MVP 阶段保留空默认）。
/// 实现者只需关心 `on_video*` / `on_audio*` 与 `playback_info`。
#[async_trait]
pub trait AirPlayConsumer: Send + Sync {
    /// 收到视频流格式信息（SETUP video 响应后触发）。
    async fn on_video_format(&self, video_stream_info: VideoStreamInfo);

    /// 收到一帧解密后的视频数据（已重写为 Annex-B 格式）。
    async fn on_video(&self, bytes: &[u8]);

    /// 视频源断开（TEARDOWN video）。
    async fn on_video_src_disconnect(&self);

    /// 收到音频流格式信息（SETUP audio 响应后触发）。
    async fn on_audio_format(&self, audio_stream_info: AudioStreamInfo);

    /// 收到一帧解密后的音频数据（已重排）。
    async fn on_audio(&self, bytes: &[u8]);

    /// 音频源断开（TEARDOWN audio）。
    async fn on_audio_src_disconnect(&self);

    // ---- 以下为 HLS / YouTube 相关，MVP 阶段提供空默认实现 ----

    async fn on_media_playlist(&self, _playlist_uri: &str) {}

    async fn on_media_playlist_remove(&self) {}

    async fn on_media_playlist_pause(&self) {}

    async fn on_media_playlist_resume(&self) {}

    /// 当前播放进度，用于响应 `/playback-info` 请求。
    async fn playback_info(&self) -> PlaybackInfo {
        PlaybackInfo::default()
    }
}

/// 播放进度信息。
///
/// 播放信息。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaybackInfo {
    /// 媒体总时长（秒）。
    pub duration: f64,
    /// 当前播放位置（秒）。
    pub position: f64,
}

impl PlaybackInfo {
    pub fn new(duration: f64, position: f64) -> Self {
        Self { duration, position }
    }
}

impl Default for PlaybackInfo {
    fn default() -> Self {
        // 默认实现: `new PlaybackInfo(0, 0)`
        Self {
            duration: 0.0,
            position: 0.0,
        }
    }
}
