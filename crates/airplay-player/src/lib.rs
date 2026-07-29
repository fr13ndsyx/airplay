#![allow(clippy::all)]
#![allow(clippy::pedantic)]

//! AirPlay 播放器 —— GStreamer-rs 实时音视频播放。
//!
//! 架构：
//! - `GstPlayer` 在专用线程中运行 GStreamer pipeline
//! - tokio 异步任务通过 mpsc channel 推送数据
//! - `GstPlayerConsumer` 实现 `AirPlayConsumer` trait

pub mod audio_pipeline;
pub mod consumer;
pub mod player;
pub mod video_pipeline;

pub use consumer::GstPlayerConsumer;
pub use player::GstPlayer;

/// GStreamer 初始化。
///
/// 在调用任何 GStreamer API 前必须调用一次。
/// 内部使用 `std::sync::Once` 保证只初始化一次。
pub fn init() -> anyhow::Result<()> {
    use std::sync::Once;
    static INIT: Once = Once::new();
    let mut result = Ok(());
    INIT.call_once(|| {
        if let Err(e) = gstreamer::init() {
            result = Err(anyhow::anyhow!("gstreamer init failed: {}", e));
        }
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gst_init() {
        // 冒烟测试：gst::init() 不 panic
        init().expect("gst init should succeed");
    }
}
