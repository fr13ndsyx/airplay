//! airplay-server crate
//!
//! Phase 2 网络层实现：tokio 重写的 AirPlay 服务器。

#![allow(clippy::all)]
#![allow(clippy::pedantic)]

pub mod audio;
pub mod bonjour;
pub mod config;
pub mod consumer;
pub mod h264_dump;
pub mod control;
pub mod rtsp_codec;
pub mod server;
pub mod session;
pub mod video;
