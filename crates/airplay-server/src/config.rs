//! 服务器配置。

use serde::{Deserialize, Serialize};

/// AirPlay 服务器配置。
///
/// 使用 builder 模式 + `Default` 实现，便于从 TOML 等配置文件加载。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AirPlayConfig {
    /// 服务器名称，将出现在 mDNS TXT 记录与 `/info` 响应中。
    pub server_name: String,
    /// 视频宽度（像素）。
    pub width: u32,
    /// 视频高度（像素）。
    pub height: u32,
    /// 视频帧率。
    pub fps: u32,
    /// 设备标识（MAC 地址格式 `XX:XX:XX:XX:XX:XX`），用于 mDNS TXT `deviceid` 字段
    /// 与 RAOP 服务实例名 `MAC@serverName`。
    ///
    /// 原实现从网络接口自动获取；默认使用固定值，可由调用方覆盖。
    pub device_id: String,
}

impl Default for AirPlayConfig {
    fn default() -> Self {
        Self {
            // 新设备名 + 新设备 ID，避免 iOS 缓存旧版曾声明 Photo/Video 能力时的设备身份
            server_name: "airplay-rs-mirror".to_string(),
            width: 1920,
            height: 1080,
            fps: 60,
            device_id: "AA:BB:CC:DD:EE:F1".to_string(),
        }
    }
}

impl AirPlayConfig {
    /// 创建一个新的 builder。
    pub fn builder() -> AirPlayConfigBuilder {
        AirPlayConfigBuilder::default()
    }
}

/// [`AirPlayConfig`] 的 builder。
#[derive(Debug, Clone, Default)]
pub struct AirPlayConfigBuilder {
    server_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    fps: Option<u32>,
    device_id: Option<String>,
}

impl AirPlayConfigBuilder {
    pub fn server_name(mut self, name: impl Into<String>) -> Self {
        self.server_name = Some(name.into());
        self
    }
    pub fn width(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }
    pub fn height(mut self, height: u32) -> Self {
        self.height = Some(height);
        self
    }
    pub fn fps(mut self, fps: u32) -> Self {
        self.fps = Some(fps);
        self
    }
    pub fn device_id(mut self, id: impl Into<String>) -> Self {
        self.device_id = Some(id.into());
        self
    }
    pub fn build(self) -> AirPlayConfig {
        let default = AirPlayConfig::default();
        AirPlayConfig {
            server_name: self.server_name.unwrap_or(default.server_name),
            width: self.width.unwrap_or(default.width),
            height: self.height.unwrap_or(default.height),
            fps: self.fps.unwrap_or(default.fps),
            device_id: self.device_id.unwrap_or(default.device_id),
        }
    }
}
