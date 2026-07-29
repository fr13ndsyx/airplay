//! Session 管理 —— 每个 AirPlay 客户端连接对应一个 Session。
//!
//! Session 持有一个 `AirPlay` 协议 facade 实例，以及 VideoServer / AudioServer
//! 的实例。SessionManager 按 sessionId 懒创建并缓存。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use airplay_protocol::airplay::AirPlay;

use crate::audio::server::AudioServer;
use crate::audio::server_ctrl::AudioControlServer;
use crate::video::server::VideoServer;

/// 一个 AirPlay 客户端会话。
///
/// 每个 sessionId 对应一个 Session，在首次请求时懒创建。
/// `airplay` 字段持有协议层状态（Pairing / FairPlay / RTSP / 解密器）。
/// `video_server` / `audio_server` / `audio_control_server` 在 RTSP SETUP 时
/// 延迟创建并启动，在 TEARDOWN 时停止。
pub struct Session {
    /// 会话 ID（从 `Active-Remote` 或 `X-Apple-Session-ID` 头提取）。
    pub id: String,
    /// AirPlay 协议 facade（Pairing / FairPlay / RTSP / 解密器）。
    pub airplay: AirPlay,
    /// 视频服务器（视频流 SETUP 时创建并启动）。
    pub video_server: Option<VideoServer>,
    /// 音频服务器（音频流 SETUP 时创建并启动）。
    pub audio_server: Option<AudioServer>,
    /// 音频控制服务器（随音频服务器一起启动）。
    pub audio_control_server: Option<AudioControlServer>,
}

impl Session {
    /// 创建新会话。
    ///
    /// 延迟创建：SETUP 时才实例化 VideoServer / AudioServer，
    /// 避免循环依赖（VideoServer 需要 Arc<Mutex<Session>>）。
    pub fn new(id: String) -> Self {
        Self {
            id,
            airplay: AirPlay::new(),
            video_server: None,
            audio_server: None,
            audio_control_server: None,
        }
    }

    /// 停止所有子服务器（TEARDOWN 时调用）。
    ///
    /// 停止 sub-servers。
    pub fn stop_servers(&mut self) {
        if let Some(vs) = self.video_server.take() {
            vs.stop();
        }
        if let Some(as_) = self.audio_server.take() {
            as_.stop();
        }
        if let Some(acs) = self.audio_control_server.take() {
            acs.stop();
        }
    }

    /// 重置视频解密器（每个新视频 TCP 连接开始时调用）。
    ///
    /// VideoDecryptor 是有状态的 AES-CTR 流密码，iPhone 重连视频端口时
    /// 必须重置计数器，否则解密输出全是密文垃圾。
    pub fn reset_video_decryptor(&mut self) {
        self.airplay.reset_video_decryptor();
    }

    /// 重置音频解密器（每个新音频 TCP 连接开始时调用）。
    pub fn reset_audio_decryptor(&mut self) {
        self.airplay.reset_audio_decryptor();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop_servers();
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("id", &self.id)
            .field("has_video_server", &self.video_server.is_some())
            .field("has_audio_server", &self.audio_server.is_some())
            .field(
                "has_audio_control_server",
                &self.audio_control_server.is_some(),
            )
            .finish()
    }
}

/// 会话管理器。
///
/// 使用 `Arc<Mutex<HashMap>>` 保证线程安全的会话管理。
/// `get_or_create(session_id)` 懒创建 Session。
pub struct SessionManager {
    sessions: Mutex<HashMap<String, Arc<Mutex<Session>>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// 获取或创建会话（懒创建）。
    ///
    /// 返回 `Arc<Mutex<Session>>`，调用方可在异步任务中持有引用。
    pub fn get_or_create(&self, session_id: &str) -> Arc<Mutex<Session>> {
        let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
        sessions
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(Session::new(session_id.to_string()))))
            .clone()
    }

    /// 移除并返回指定会话（用于显式清理）。
    ///
    /// 原实现未实现 session 级清理（存在资源泄漏），
    /// 本实现提供此方法以便在 TEARDOWN 后主动释放。
    pub fn remove(&self, session_id: &str) -> Option<Arc<Mutex<Session>>> {
        let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
        sessions.remove(session_id)
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_or_create_creates_new_session() {
        let manager = SessionManager::new();
        let session = manager.get_or_create("test-session-1");
        let s = session.lock().unwrap();
        assert_eq!(s.id, "test-session-1");
        assert!(s.video_server.is_none());
        assert!(s.audio_server.is_none());
        assert!(s.audio_control_server.is_none());
    }

    #[test]
    fn test_get_or_create_returns_same_session_for_same_id() {
        let manager = SessionManager::new();
        let session1 = manager.get_or_create("same-id");
        let session2 = manager.get_or_create("same-id");
        // 同一个 sessionId 应返回同一个 Arc（指针相等）
        assert!(Arc::ptr_eq(&session1, &session2));
    }

    #[test]
    fn test_get_or_create_returns_different_sessions_for_different_ids() {
        let manager = SessionManager::new();
        let session1 = manager.get_or_create("id-1");
        let session2 = manager.get_or_create("id-2");
        assert!(!Arc::ptr_eq(&session1, &session2));
    }

    #[test]
    fn test_remove_session() {
        let manager = SessionManager::new();
        let session = manager.get_or_create("removable");
        assert!(Arc::strong_count(&session) >= 2); // manager + local

        let removed = manager.remove("removable");
        assert!(removed.is_some());

        // 再次获取应创建新 session
        let session2 = manager.get_or_create("removable");
        assert!(!Arc::ptr_eq(&session, &session2));
    }

    #[test]
    fn test_remove_nonexistent_returns_none() {
        let manager = SessionManager::new();
        assert!(manager.remove("nonexistent").is_none());
    }

    #[test]
    fn test_session_stop_servers_when_no_servers() {
        let mut session = Session::new("test".to_string());
        // 无服务器时调用 stop_servers 不应 panic
        session.stop_servers();
        assert!(session.video_server.is_none());
    }

    #[test]
    fn test_session_debug_format() {
        let session = Session::new("debug-test".to_string());
        let debug_str = format!("{:?}", session);
        assert!(debug_str.contains("debug-test"));
        assert!(debug_str.contains("has_video_server: false"));
    }
}
