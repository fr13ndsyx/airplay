//! mDNS 服务注册（Bonjour）。
//!
//! 注册两个 mDNS 服务：
//! - `{server_name}._airplay._tcp.local.` — AirPlay 控制/镜像通道
//! - `{mac}@{server_name}._raop._tcp.local.` — AirTunes 音频通道
//!
//! 使用 `mdns-sd` crate 的 `enable_addr_auto()` 自动在所有可用网络接口上广播。

use anyhow::Result;
use mdns_sd::ServiceDaemon;
use tracing::{info, warn};

use airplay_protocol::plist_util::MIRRORING_FEATURES_TXT;

/// AirPlay 固定 Ed25519 公钥（与 Pairing 动态密钥无关）。
const AIRPLAY_PK: &str = "f3769a660475d27b4f6040381d784645e13e21c53e6d2da6a8c3d757086fc336";

/// AirPlay / AirTunes mDNS 服务注册器。
pub struct Bonjour {
    server_name: String,
    /// MAC 地址格式：`XX:XX:XX:XX:XX:XX`
    device_id: String,
    daemon: Option<ServiceDaemon>,
    /// 已注册的 airplay 服务全名（用于注销）。
    airplay_fullname: String,
    /// 已注册的 raop 服务全名（用于注销）。
    raop_fullname: String,
}

impl Bonjour {
    /// 创建一个新的 Bonjour 注册器。
    ///
    /// - `server_name`: 服务器名称（如 `"airplay-rs"`）
    /// - `device_id`: MAC 地址格式（如 `"AA:BB:CC:DD:EE:FF"`）
    pub fn new(server_name: String, device_id: String) -> Self {
        let airplay_fullname = format!("{}._airplay._tcp.local.", server_name);
        let mac_no_colon = device_id.replace(':', "");
        let raop_instance = format!("{}@{}", mac_no_colon, server_name);
        let raop_fullname = format!("{}._raop._tcp.local.", raop_instance);
        Self {
            server_name,
            device_id,
            daemon: None,
            airplay_fullname,
            raop_fullname,
        }
    }

    /// 启动 mDNS 服务注册。
    ///
    /// 在所有可用 IPv4 接口上广播 `_airplay._tcp` 与 `_raop._tcp` 服务。
    pub fn start(&mut self, port: u16) -> Result<()> {
        let daemon = ServiceDaemon::new()?;

        // ---- 注册 _airplay._tcp ----
        let airplay_props = airplay_txt_properties(&self.device_id);
        let airplay_info = mdns_sd::ServiceInfo::new(
            "_airplay._tcp.local.",
            &self.server_name,
            &format!("{}.local.", self.server_name),
            "", // 空 IP + enable_addr_auto → 自动检测所有接口
            port,
            &airplay_props[..],
        )?
        .enable_addr_auto();

        daemon.register(airplay_info)?;
        info!(
            "mDNS service {} registered on port {}",
            self.airplay_fullname, port
        );

        // ---- 注册 _raop._tcp ----
        let mac_no_colon = self.device_id.replace(':', "");
        let raop_instance = format!("{}@{}", mac_no_colon, self.server_name);
        let raop_props = raop_txt_properties();
        let raop_info = mdns_sd::ServiceInfo::new(
            "_raop._tcp.local.",
            &raop_instance,
            &format!("{}.local.", self.server_name),
            "",
            port,
            &raop_props[..],
        )?
        .enable_addr_auto();

        daemon.register(raop_info)?;
        info!(
            "mDNS service {} registered on port {}",
            self.raop_fullname, port
        );

        self.daemon = Some(daemon);
        Ok(())
    }

    /// 停止 mDNS 服务并注销。
    pub fn stop(&mut self) {
        if let Some(daemon) = self.daemon.take() {
            if let Err(e) = daemon.unregister(&self.airplay_fullname) {
                warn!("Failed to unregister {}: {}", self.airplay_fullname, e);
            }
            if let Err(e) = daemon.unregister(&self.raop_fullname) {
                warn!("Failed to unregister {}: {}", self.raop_fullname, e);
            }
            if let Err(e) = daemon.shutdown() {
                warn!("Failed to shutdown mDNS daemon: {}", e);
            }
            info!("mDNS services unregistered");
        }
    }
}

impl Drop for Bonjour {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 构建 `_airplay._tcp` TXT 记录。
///
/// AirPlay mDNS 属性。
fn airplay_txt_properties(device_id: &str) -> Vec<(&'static str, String)> {
    vec![
        ("deviceid", device_id.to_string()),
        ("features", MIRRORING_FEATURES_TXT.to_string()),
        ("srcvers", "220.68".to_string()),
        ("flags", "0x44".to_string()),
        ("vv", "2".to_string()),
        ("model", "AppleTV3,2C".to_string()),
        ("rhd", "5.6.0.0".to_string()),
        ("pw", "false".to_string()),
        ("pk", AIRPLAY_PK.to_string()),
        ("rmodel", "PC1.0".to_string()),
        ("rrv", "1.01".to_string()),
        ("rsv", "1.00".to_string()),
        ("pcversion", "1715".to_string()),
    ]
}

/// 构建 `_raop._tcp` TXT 记录。
///
/// AirTunes mDNS 属性。
fn raop_txt_properties() -> Vec<(&'static str, String)> {
    vec![
        ("ch", "2".to_string()),
        ("cn", "1,3".to_string()),
        ("da", "true".to_string()),
        ("et", "0,3,5".to_string()),
        ("ek", "1".to_string()),
        ("ft", MIRRORING_FEATURES_TXT.to_string()),
        ("am", "AppleTV3,2C".to_string()),
        ("md", "0,1,2".to_string()),
        ("sr", "44100".to_string()),
        ("ss", "16".to_string()),
        ("sv", "false".to_string()),
        ("sm", "false".to_string()),
        ("tp", "UDP".to_string()),
        ("txtvers", "1".to_string()),
        ("sf", "0x44".to_string()),
        ("vs", "220.68".to_string()),
        ("vn", "65537".to_string()),
        ("pk", AIRPLAY_PK.to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_airplay_txt_properties_contains_required_keys() {
        let props = airplay_txt_properties("AA:BB:CC:DD:EE:FF");
        let keys: Vec<_> = props.iter().map(|(k, _)| *k).collect();
        assert!(keys.contains(&"deviceid"));
        assert!(keys.contains(&"features"));
        assert!(keys.contains(&"pk"));
        assert!(keys.contains(&"model"));
    }

    #[test]
    fn test_raop_txt_properties_contains_required_keys() {
        let props = raop_txt_properties();
        let keys: Vec<_> = props.iter().map(|(k, _)| *k).collect();
        assert!(keys.contains(&"ch"));
        assert!(keys.contains(&"sr"));
        assert!(keys.contains(&"tp"));
        assert!(keys.contains(&"pk"));
    }

    #[test]
    fn test_bonjour_fullname_construction() {
        let b = Bonjour::new("airplay-rs".to_string(), "AA:BB:CC:DD:EE:FF".to_string());
        assert_eq!(b.airplay_fullname, "airplay-rs._airplay._tcp.local.");
        assert_eq!(b.raop_fullname, "AABBCCDDEEFF@airplay-rs._raop._tcp.local.");
    }
}
