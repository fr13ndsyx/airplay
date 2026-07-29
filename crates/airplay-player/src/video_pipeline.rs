//! 视频管线构建器。
//!
//! 构建 `appsrc ! h264parse ! d3d11h264dec ! d3d11videosink` 管线，
//! 优先使用 Windows D3D11 硬件解码（零拷贝）。
//! 若 D3D11 元素不可用，自动回退到 `avdec_h264 ! videoconvert ! autovideosink` 软解。
//!
//! 纯镜像模式：不识别照片内容、不裁剪媒体、不强制输出画布尺寸；
//! 解码器协商出的每个 raw-video 尺寸直接交给 sink，由 sink 保持原始宽高比。

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;

/// 创建视频管线。
///
/// 返回 `(Pipeline, AppSrc)`，调用方通过 `AppSrc::push_buffer()` 推送 H264 Annex-B 数据。
/// 播放器不识别照片内容、不裁剪媒体、不强制输出画布尺寸；解码器协商出的每个
/// raw-video 尺寸直接交给 sink，并由 sink 保持原始宽高比。
///
/// # 管线
/// - 硬解（优先）：`appsrc ! h264parse(config-interval=-1) ! d3d11h264dec ! d3d11videosink`
/// - 软解（回退）：`appsrc ! h264parse ! avdec_h264 ! videoconvert ! autovideosink`
///
/// # AppSrc 配置
/// - `is_live=true`：实时流
/// - `do_timestamp=true`：自动打时间戳
/// - `max_bytes=2MB`：背压上限
/// - `leaky_type=Downstream`：队列满时丢旧帧
/// - caps=`video/x-h264, stream-format=byte-stream, alignment=au`
///
/// # 视频 Sink 配置
/// - `sync=false`：实时流不等待时钟同步，帧到达立即显示（关键！降低延迟 + 提高帧率）
/// - `force-aspect-ratio=true`：保持 iPhone 发送画面的原始宽高比
/// - `rotate-method=identity`：不二次旋转，方向只由 H.264 实际宽高表达
pub fn create_video_pipeline() -> Result<(gst::Pipeline, gst_app::AppSrc)> {
    let pipeline = gst::Pipeline::default();

    let caps = gst::Caps::builder("video/x-h264")
        .field("stream-format", "byte-stream")
        .field("alignment", "au")
        .build();

    let appsrc = gst_app::AppSrc::builder()
        .name("video-src")
        .caps(&caps)
        .is_live(true)
        .do_timestamp(true)
        .max_bytes(2_000_000)
        .leaky_type(gst_app::AppLeakyType::Downstream)
        .build();

    let h264parse = gst::ElementFactory::make("h264parse")
        .name("video-parser")
        .build()?;
    h264parse.set_property("config-interval", -1i32);

    match try_d3d11_pipeline() {
        Some((decoder, videosink)) => {
            tracing::info!("视频管线使用 D3D11 硬件解码（原始尺寸等比显示）");
            install_caps_probe(&decoder);

            videosink.set_property("sync", false);
            videosink.set_property("force-aspect-ratio", true);
            videosink.set_property(
                "rotate-method",
                gst_video::VideoOrientationMethod::Identity,
            );

            let elements: [&gst::Element; 4] =
                [appsrc.upcast_ref(), &h264parse, &decoder, &videosink];
            pipeline.add_many(&elements)?;
            gst::Element::link_many(&elements)?;
        }
        None => {
            tracing::warn!("D3D11 视频管线不可用，回退到软解 (avdec_h264 + videoconvert)");
            let decoder = gst::ElementFactory::make("avdec_h264")
                .name("video-decoder")
                .build()?;
            let videoconvert = gst::ElementFactory::make("videoconvert")
                .name("video-convert")
                .build()?;
            let videosink = gst::ElementFactory::make("autovideosink")
                .name("video-sink")
                .build()?;
            install_caps_probe(&decoder);

            videosink.set_property("sync", false);
            if videosink.find_property("force-aspect-ratio").is_some() {
                videosink.set_property("force-aspect-ratio", true);
            }

            let elements: [&gst::Element; 5] = [
                appsrc.upcast_ref(),
                &h264parse,
                &decoder,
                &videoconvert,
                &videosink,
            ];
            pipeline.add_many(&elements)?;
            gst::Element::link_many(&elements)?;
        }
    }

    Ok((pipeline, appsrc))
}

/// 只记录解码器动态协商出的真实画面尺寸，不修改事件、buffer 或 CAPS。
fn install_caps_probe(decoder: &gst::Element) {
    let Some(src_pad) = decoder.static_pad("src") else {
        tracing::warn!("视频解码器没有静态 src pad，无法记录协商尺寸");
        return;
    };

    src_pad.add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, move |_pad, info| {
        if let Some(gst::PadProbeData::Event(event)) = &info.data {
            if let gst::EventView::Caps(caps_event) = event.view() {
                tracing::info!("视频画面协商变化（原样通过）: {}", caps_event.caps());
            }
        }
        gst::PadProbeReturn::Ok
    });
}

/// 尝试创建 D3D11 硬解管线元素。
///
/// 返回 `None` 表示 D3D11 元素不可用（非 Windows 或插件缺失），
/// 调用方将自动回退到软件解码。
fn try_d3d11_pipeline() -> Option<(gst::Element, gst::Element)> {
    let decoder = gst::ElementFactory::make("d3d11h264dec")
        .name("video-decoder")
        .build()
        .map_err(|e| {
            tracing::debug!("d3d11h264dec 不可用: {}", e);
            e
        })
        .ok()?;
    let videosink = gst::ElementFactory::make("d3d11videosink")
        .name("video-sink")
        .build()
        .map_err(|e| {
            tracing::debug!("d3d11videosink 不可用: {}", e);
            e
        })
        .ok()?;
    Some((decoder, videosink))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_video_pipeline() {
        crate::init().expect("gst init");
        let (pipeline, _appsrc) = create_video_pipeline().expect("create video pipeline");
        pipeline
            .set_state(gst::State::Ready)
            .expect("set state to Ready");
        let _ = pipeline.set_state(gst::State::Null);
    }

    #[test]
    fn test_video_pipeline_has_no_content_transform() {
        crate::init().expect("gst init");
        let (pipeline, appsrc) = create_video_pipeline().expect("create video pipeline");

        assert_eq!(appsrc.name(), "video-src");
        assert!(pipeline.by_name("video-parser").is_some());
        assert!(pipeline.by_name("video-decoder").is_some());
        assert!(pipeline.by_name("video-sink").is_some());
        assert!(pipeline.by_name("photo-crop").is_none());

        let _ = pipeline.set_state(gst::State::Null);
    }

    #[test]
    fn test_video_sink_preserves_aspect_ratio() {
        crate::init().expect("gst init");
        let (pipeline, _) = create_video_pipeline().expect("create video pipeline");
        let sink = pipeline
            .by_name("video-sink")
            .expect("video-sink should exist");

        let sync: bool = sink.property("sync");
        assert!(!sync, "video sink sync 应为 false 以降低实时镜像延迟");
        if sink.find_property("force-aspect-ratio").is_some() {
            let force_aspect_ratio: bool = sink.property("force-aspect-ratio");
            assert!(force_aspect_ratio, "video sink 必须保持原始宽高比");
        }

        let _ = pipeline.set_state(gst::State::Null);
    }
}
