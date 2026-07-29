//! GstPlayer 核心 —— 专用线程 + mpsc 通道 + 生命周期管理。
//!
//! 架构：
//! ```text
//! [tokio 异步侧]                    [专用 GStreamer 线程]
//!  GstPlayerConsumer
//!    ├─ start_video() ──────────→  Command::StartVideo
//!    │                              → video_pipeline.set_state(Playing)
//!    ├─ push_video(bytes) ───────→  Command::VideoData(bytes)
//!    │                              → video_appsrc.push_buffer()
//!    ├─ start_audio(ct) ─────────→  Command::StartAudio(ct)
//!    │                              → alac/aac_eld_pipeline.set_state(Playing)
//!    └─ shutdown() ──────────────→  Command::Shutdown
//!                                   → set_state(Null) + exit thread
//! ```

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;

use airplay_protocol::stream_info::CompressionType;

use crate::audio_pipeline;
use crate::video_pipeline;

/// GStreamer 管线命令 —— tokio 侧通过 channel 发送给 GStreamer 线程。
pub(crate) enum Command {
    StartVideo,
    StopVideo,
    StartAudio(CompressionType),
    StopAudio,
    VideoData(Vec<u8>),
    AudioData(Vec<u8>),
    SetVolume(f32),
    Shutdown,
}

/// GStreamer 播放器核心。
///
/// 在专用 `std::thread` 中运行 GStreamer pipeline（同步 API）。
/// tokio 异步任务通过 `push_video` / `push_audio` 等方法发送命令，
/// **不会**阻塞 tokio runtime。
///
/// # 生命周期
/// 1. `GstPlayer::new()` → 创建 3 条管线 + 启动 GStreamer 线程
/// 2. `start_video()` / `start_audio()` → 切换管线到 Playing
/// 3. `push_video()` / `push_audio()` → 推送数据
/// 4. `stop_video()` / `stop_audio()` → 切换管线到 Null
/// 5. `Drop` → 发送 Shutdown + join 线程 + 清理管线
pub struct GstPlayer {
    tx: mpsc::Sender<Command>,
    thread: Option<thread::JoinHandle<()>>,
}

impl GstPlayer {
    /// 创建播放器并启动 GStreamer 线程。
    ///
    /// 内部创建 3 条管线（视频 / ALAC / AAC-ELD），但此时均为 Null 状态。
    /// 需调用 `start_video()` / `start_audio()` 切换到 Playing。
    pub fn new() -> Result<Self> {
        crate::init()?;

        // 创建管线（在主线程创建，然后移动到 GStreamer 线程）
        let (video_pipeline, video_appsrc) = video_pipeline::create_video_pipeline()?;
        let (alac_pipeline, alac_appsrc, alac_volume) = audio_pipeline::create_alac_pipeline()?;
        let (aac_eld_pipeline, aac_eld_appsrc, aac_eld_volume) = audio_pipeline::create_aac_eld_pipeline()?;

        let (tx, rx) = mpsc::channel();

        let thread = thread::Builder::new()
            .name("gst-player".into())
            .spawn(move || {
                run_gstreamer_loop(
                    rx,
                    video_pipeline,
                    video_appsrc,
                    alac_pipeline,
                    alac_appsrc,
                    aac_eld_pipeline,
                    aac_eld_appsrc,
                    alac_volume,
                    aac_eld_volume,
                );
            })?;

        Ok(Self {
            tx,
            thread: Some(thread),
        })
    }

    /// 创建关联的 `GstPlayerConsumer`，用于实现 `AirPlayConsumer` trait。
    ///
    /// Consumer 持有 sender 的克隆，可安全地在 tokio 异步任务中共享。
    pub fn consumer(&self) -> GstPlayerConsumer {
        GstPlayerConsumer::new(self.tx.clone())
    }

    /// 启动视频管线（切换到 Playing）。
    pub fn start_video(&self) -> Result<()> {
        self.tx
            .send(Command::StartVideo)
            .map_err(|e| anyhow::anyhow!("GStreamer 线程已退出: {}", e))
    }

    /// 停止视频管线（切换到 Null）。
    pub fn stop_video(&self) -> Result<()> {
        self.tx
            .send(Command::StopVideo)
            .map_err(|e| anyhow::anyhow!("GStreamer 线程已退出: {}", e))
    }

    /// 启动音频管线（按压缩类型路由）。
    pub fn start_audio(&self, compression_type: CompressionType) -> Result<()> {
        self.tx
            .send(Command::StartAudio(compression_type))
            .map_err(|e| anyhow::anyhow!("GStreamer 线程已退出: {}", e))
    }

    /// 停止音频管线。
    pub fn stop_audio(&self) -> Result<()> {
        self.tx
            .send(Command::StopAudio)
            .map_err(|e| anyhow::anyhow!("GStreamer 线程已退出: {}", e))
    }

    /// 推送视频数据（H264 Annex-B 字节流）。
    ///
    /// 非阻塞：数据放入 channel 后立即返回，GStreamer 线程异步消费。
    pub fn push_video(&self, data: Vec<u8>) -> Result<()> {
        self.tx
            .send(Command::VideoData(data))
            .map_err(|e| anyhow::anyhow!("GStreamer 线程已退出: {}", e))
    }

    /// 推送音频数据。
    ///
    /// GStreamer 线程根据当前 `compression_type` 路由到 ALAC 或 AAC-ELD AppSrc。
    pub fn push_audio(&self, data: Vec<u8>) -> Result<()> {
        self.tx
            .send(Command::AudioData(data))
            .map_err(|e| anyhow::anyhow!("GStreamer 线程已退出: {}", e))
    }

    /// 设置音量 (0.0 = 静音, 1.0 = 原始音量)。
    pub fn set_volume(&self, volume: f32) -> Result<()> {
        self.tx
            .send(Command::SetVolume(volume))
            .map_err(|e| anyhow::anyhow!("GStreamer 线程已退出: {}", e))
    }

    /// 关闭播放器，停止所有管线并等待 GStreamer 线程退出。
    pub fn shutdown(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for GstPlayer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// GStreamer 线程主循环。
///
/// 接收命令、推送 buffer。
/// bus 错误消息只在 channel 超时（无数据）时轮询，避免每帧都检查 3 个 bus。
///
/// **延迟启动策略**：`StartVideo` 只标记 `video_pending = true`，
/// 收到第一帧 `VideoData` 时才真正切换到 Playing，避免 AppSrc 空数据导致 h264parse 崩溃。
fn run_gstreamer_loop(
    rx: mpsc::Receiver<Command>,
    video_pipeline: gst::Pipeline,
    video_appsrc: gst_app::AppSrc,
    alac_pipeline: gst::Pipeline,
    alac_appsrc: gst_app::AppSrc,
    aac_eld_pipeline: gst::Pipeline,
    aac_eld_appsrc: gst_app::AppSrc,
    alac_volume: gst::Element,
    aac_eld_volume: gst::Element,
) {
    let mut audio_compression: Option<CompressionType> = None;
    let mut video_pending = false; // 收到 StartVideo 但尚未收到第一帧数据

    tracing::info!("GStreamer 线程启动");

    loop {
        // recv_timeout 避免忙等，50ms 超时后轮询 bus
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Command::StartVideo) => {
                // 不立即启动管线，等第一帧数据到达再启动
                video_pending = true;
                tracing::info!("视频管线等待第一帧数据...");
            }
            Ok(Command::StopVideo) => {
                let _ = video_pipeline.set_state(gst::State::Null);
                video_pending = false;
                tracing::info!("视频管线 → Null");
            }
            Ok(Command::StartAudio(ct)) => {
                audio_compression = Some(ct);
                let pipeline = match ct {
                    CompressionType::Alac => &alac_pipeline,
                    CompressionType::AacEld => &aac_eld_pipeline,
                    other => {
                        tracing::warn!("不支持的音频压缩类型: {:?}", other);
                        continue;
                    }
                };
                if let Err(e) = pipeline.set_state(gst::State::Playing) {
                    tracing::error!("音频管线启动失败 ({:?}): {}", ct, e);
                } else {
                    tracing::info!("音频管线 → Playing ({:?})", ct);
                }
            }
            Ok(Command::StopAudio) => {
                let _ = alac_pipeline.set_state(gst::State::Null);
                let _ = aac_eld_pipeline.set_state(gst::State::Null);
                audio_compression = None;
                tracing::info!("音频管线 → Null");
            }
            Ok(Command::VideoData(data)) => {
                // 第一帧数据到达时才启动管线
                if video_pending {
                    video_pending = false;
                    if let Err(e) = video_pipeline.set_state(gst::State::Playing) {
                        tracing::error!("视频管线启动失败: {}", e);
                        // 启动失败则丢弃本帧
                        continue;
                    } else {
                        tracing::info!("视频管线 → Playing（收到第一帧数据）");
                    }
                }
                let buffer = gst::Buffer::from_slice(data);
                if let Err(e) = video_appsrc.push_buffer(buffer) {
                    tracing::debug!("push video buffer 失败: {}", e);
                }
            }
            Ok(Command::AudioData(data)) => {
                let buffer = gst::Buffer::from_slice(data);
                let appsrc = match audio_compression {
                    Some(CompressionType::Alac) => &alac_appsrc,
                    Some(CompressionType::AacEld) => &aac_eld_appsrc,
                    _ => {
                        tracing::debug!("音频数据到达但 compression_type 未设置，丢弃");
                        continue;
                    }
                };
                if let Err(e) = appsrc.push_buffer(buffer) {
                    tracing::debug!("push audio buffer 失败: {}", e);
                }
            }
            Ok(Command::SetVolume(vol)) => {
                // GStreamer volume 元素的 "volume" 属性期望 gdouble (f64)，
                // 传入 f32 会在 Windows MSVC + glib 0.18 下 panic
                let vol_f64 = vol as f64;
                alac_volume.set_property("volume", vol_f64);
                aac_eld_volume.set_property("volume", vol_f64);
                tracing::info!("音量设置为 {:.0}%", vol * 100.0);
            }
            Ok(Command::Shutdown) => {
                tracing::info!("收到 Shutdown 命令，退出 GStreamer 线程");
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                // 仅在无数据时轮询 bus 错误消息（避免每帧都检查 3 个 bus）
                poll_bus_errors(&video_pipeline, "视频");
                poll_bus_errors(&alac_pipeline, "ALAC");
                poll_bus_errors(&aac_eld_pipeline, "AAC-ELD");
            }
            Err(RecvTimeoutError::Disconnected) => {
                tracing::warn!("命令通道断开，退出 GStreamer 线程");
                break;
            }
        }
    }

    // 清理：所有管线切到 Null
    let _ = video_pipeline.set_state(gst::State::Null);
    let _ = alac_pipeline.set_state(gst::State::Null);
    let _ = aac_eld_pipeline.set_state(gst::State::Null);
    tracing::info!("GStreamer 线程退出，所有管线已清理");
}


/// 轮询管线 bus 的错误/EOS 消息。
fn poll_bus_errors(pipeline: &gst::Pipeline, label: &str) {
    let bus = match pipeline.bus() {
        Some(b) => b,
        None => return,
    };
    while let Some(msg) = bus.pop_filtered(&[gst::MessageType::Error, gst::MessageType::Eos]) {
        match msg.view() {
            gst::MessageView::Error(err) => {
                tracing::error!("{}管线错误: {} ({:?})", label, err.error(), err.debug());
                let _ = pipeline.set_state(gst::State::Null);
            }
            gst::MessageView::Eos(_) => {
                tracing::info!("{}管线 EOS", label);
                let _ = pipeline.set_state(gst::State::Null);
            }
            _ => {}
        }
    }
}

// 引入 GstPlayerConsumer 用于 player.consumer() 方法
use crate::consumer::GstPlayerConsumer;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gst_player_lifecycle() {
        let mut player = GstPlayer::new().expect("create player");

        // 启动视频
        player.start_video().expect("start video");
        std::thread::sleep(Duration::from_millis(100));

        // 停止视频
        player.stop_video().expect("stop video");
        std::thread::sleep(Duration::from_millis(50));

        // 启动 ALAC 音频
        player
            .start_audio(CompressionType::Alac)
            .expect("start alac");
        std::thread::sleep(Duration::from_millis(100));
        player.stop_audio().expect("stop audio");

        // 启动 AAC-ELD 音频
        player
            .start_audio(CompressionType::AacEld)
            .expect("start aac-eld");
        std::thread::sleep(Duration::from_millis(100));
        player.stop_audio().expect("stop audio");

        // shutdown 在 Drop 中自动调用
        player.shutdown();
    }

    #[test]
    fn test_gst_player_push_video() {
        let player = GstPlayer::new().expect("create player");
        player.start_video().expect("start video");

        // 推送假数据（SPS NAL unit: 00 00 00 01 67 ...）
        let fake_h264 = vec![0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x0a, 0xf8, 0x41, 0xa2];
        player.push_video(fake_h264).expect("push video");

        std::thread::sleep(Duration::from_millis(100));
        // Drop 时自动 shutdown
    }
}
