//! airplay-protocol crate
//!
//! Phase 1 协议层实现：Pairing / FairPlay / RTSP / 解密器 / plist 工具。

#![allow(clippy::all)]
#![allow(clippy::pedantic)]

pub mod airplay;
pub mod audio_decryptor;
pub mod fairplay_setup;
pub mod pairing;
pub mod plist_util;
pub mod rtsp;
pub mod stream_info;
pub mod video_decryptor;
