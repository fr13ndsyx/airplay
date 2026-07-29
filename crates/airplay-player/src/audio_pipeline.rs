//! 音频管线构建器。
//!
//! 构建 ALAC 和 AAC-ELD 两条音频管线，根据 `CompressionType` 选择使用。
//!
//! # 管线
//! - ALAC：`appsrc ! avdec_alac ! audioconvert ! audioresample ! autoaudiosink(sync=false)`
//! - AAC-ELD：`appsrc ! avdec_aac ! audioconvert ! audioresample ! autoaudiosink(sync=false)`
//!
//! # 关键设计
//! - `sync=false`：实时流不等待时钟同步，避免音频滞后
//! - `codec_data`：硬编码 magic cookie，解码器初始化必需
//! - 两条管线预创建，按 `compression_type` 路由数据

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;

/// ALAC codec_data（magic cookie）—— 硬编码值。
///
/// 36 字节，对应 44100Hz / 16-bit / 2 声道 ALAC 配置。
const ALAC_CODEC_DATA: &[u8] = &[
    0x00, 0x00, 0x00, 0x24, 0x61, 0x6c, 0x61, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x60,
    0x00, 0x10, 0x28, 0x0a, 0x0e, 0x02, 0x00, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0xac, 0x44,
];

/// AAC-ELD codec_data（AudioSpecificConfig）—— 硬编码值。
///
/// 4 字节，对应 44100Hz / 2 声道 AAC-ELD 配置。
const AAC_ELD_CODEC_DATA: &[u8] = &[0xf8, 0xe8, 0x50, 0x00];

/// 创建 ALAC 音频管线。
///
/// 管线：`appsrc ! avdec_alac ! audioconvert ! audioresample ! autoaudiosink(sync=false)`
///
/// caps：`audio/x-alac, mpegversion=4, channels=2, rate=44100, stream-format=raw, codec_data=<ALAC_CODEC_DATA>`
pub fn create_alac_pipeline() -> Result<(gst::Pipeline, gst_app::AppSrc, gst::Element)> {
    let pipeline = gst::Pipeline::default();

    let codec_data = gst::Buffer::from_slice(ALAC_CODEC_DATA.to_vec());
    let caps = gst::Caps::builder("audio/x-alac")
        .field("mpegversion", 4)
        .field("channels", 2)
        .field("rate", 44100)
        .field("stream-format", "raw")
        .field("codec_data", &codec_data)
        .build();

    let appsrc = gst_app::AppSrc::builder()
        .name("alac-src")
        .caps(&caps)
        .is_live(true)
        .do_timestamp(true)
        .format(gst::Format::Time)
        .max_bytes(2_000_000)
        .leaky_type(gst_app::AppLeakyType::Downstream)
        .build();

    let decoder = gst::ElementFactory::make("avdec_alac")
        .name("alac-decoder")
        .build()?;
    let audioconvert = gst::ElementFactory::make("audioconvert")
        .name("alac-convert")
        .build()?;
    let audioresample = gst::ElementFactory::make("audioresample")
        .name("alac-resample")
        .build()?;
    let volume = gst::ElementFactory::make("volume")
        .name("alac-volume")
        .build()?;
    let audiosink = gst::ElementFactory::make("autoaudiosink")
        .name("alac-sink")
        .build()?;
    // sync=false: 实时流不等待时钟同步，避免音频滞后
    audiosink.set_property("sync", false);

    let elements: [&gst::Element; 6] = [
        appsrc.upcast_ref(),
        &decoder,
        &audioconvert,
        &audioresample,
        &volume,
        &audiosink,
    ];
    pipeline.add_many(&elements)?;
    gst::Element::link_many(&elements)?;

    Ok((pipeline, appsrc, volume))
}

/// 创建 AAC-ELD 音频管线。
///
/// 管线：`appsrc ! avdec_aac ! audioconvert ! audioresample ! autoaudiosink(sync=false)`
///
/// caps：`audio/mpeg, mpegversion=4, channels=2, rate=44100, stream-format=raw, codec_data=<AAC_ELD_CODEC_DATA>`
pub fn create_aac_eld_pipeline() -> Result<(gst::Pipeline, gst_app::AppSrc, gst::Element)> {
    let pipeline = gst::Pipeline::default();

    let codec_data = gst::Buffer::from_slice(AAC_ELD_CODEC_DATA.to_vec());
    let caps = gst::Caps::builder("audio/mpeg")
        .field("mpegversion", 4)
        .field("channels", 2)
        .field("rate", 44100)
        .field("stream-format", "raw")
        .field("codec_data", &codec_data)
        .build();

    let appsrc = gst_app::AppSrc::builder()
        .name("aac-eld-src")
        .caps(&caps)
        .is_live(true)
        .do_timestamp(true)
        .format(gst::Format::Time)
        .max_bytes(2_000_000)
        .leaky_type(gst_app::AppLeakyType::Downstream)
        .build();

    let decoder = gst::ElementFactory::make("avdec_aac")
        .name("aac-decoder")
        .build()?;
    let audioconvert = gst::ElementFactory::make("audioconvert")
        .name("aac-convert")
        .build()?;
    let audioresample = gst::ElementFactory::make("audioresample")
        .name("aac-resample")
        .build()?;
    let volume = gst::ElementFactory::make("volume")
        .name("aac-volume")
        .build()?;
    let audiosink = gst::ElementFactory::make("autoaudiosink")
        .name("aac-sink")
        .build()?;
    audiosink.set_property("sync", false);

    let elements: [&gst::Element; 6] = [
        appsrc.upcast_ref(),
        &decoder,
        &audioconvert,
        &audioresample,
        &volume,
        &audiosink,
    ];
    pipeline.add_many(&elements)?;
    gst::Element::link_many(&elements)?;

    Ok((pipeline, appsrc, volume))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_alac_pipeline() {
        crate::init().expect("gst init");
        let (pipeline, appsrc) = create_alac_pipeline().expect("create alac pipeline");
        assert_eq!(appsrc.name(), "alac-src");
        assert!(pipeline.by_name("alac-decoder").is_some());
        assert!(pipeline.by_name("alac-sink").is_some());
        pipeline.set_state(gst::State::Ready).expect("set Ready");
        let _ = pipeline.set_state(gst::State::Null);
    }

    #[test]
    fn test_create_aac_eld_pipeline() {
        crate::init().expect("gst init");
        let (pipeline, appsrc) = create_aac_eld_pipeline().expect("create aac-eld pipeline");
        assert_eq!(appsrc.name(), "aac-eld-src");
        assert!(pipeline.by_name("aac-decoder").is_some());
        assert!(pipeline.by_name("aac-sink").is_some());
        pipeline.set_state(gst::State::Ready).expect("set Ready");
        let _ = pipeline.set_state(gst::State::Null);
    }
}
