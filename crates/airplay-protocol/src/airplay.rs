// AirPlay Facade —— 聚合 Pairing / FairPlaySetup / RTSP / 解密器，
// 提供 pair-setup / pair-verify / fp-setup /
// rtsp-setup / rtsp-teardown / decryptVideo / decryptAudio 的统一入口。

#![allow(clippy::pedantic)]

use crate::audio_decryptor::AudioDecryptor;
use crate::fairplay_setup::FairPlaySetup;
use crate::pairing::Pairing;
use crate::rtsp::RTSP;
use crate::stream_info::MediaStreamInfo;
use crate::video_decryptor::VideoDecryptor;
use thiserror::Error;

/// AirPlay Facade 错误类型。
#[derive(Debug, Error)]
pub enum AirPlayError {
    #[error("pairing error: {0}")]
    Pairing(#[from] crate::pairing::PairingError),
    #[error("fairplay error: {0}")]
    FairPlay(#[from] crate::fairplay_setup::FairPlayError),
    #[error("rtsp error: {0}")]
    Rtsp(#[from] crate::rtsp::RtspError),
    #[error("video decryptor error: {0}")]
    VideoDecryptor(#[from] crate::video_decryptor::VideoDecryptorError),
    #[error("audio decryptor error: {0}")]
    AudioDecryptor(#[from] crate::audio_decryptor::AudioDecryptorError),
    #[error("shared secret not available (pair-verify step 1 not completed)")]
    NoSharedSecret,
    #[error("ekey not available (rtsp setup with ekey not completed)")]
    NoEkey,
    #[error("eiv not available (rtsp setup with eiv not completed)")]
    NoEiv,
    #[error("stream connection ID not available (rtsp video setup not completed)")]
    NoStreamConnectionId,
}

/// AirPlay Facade。
///
/// 聚合协议层的全部组件，提供单一入口。解密器懒加载，
/// 首次调用 `decrypt_video` / `decrypt_audio` 时创建。
pub struct AirPlay {
    pairing: Pairing,
    fairplay_setup: FairPlaySetup,
    rtsp: RTSP,
    video_decryptor: Option<VideoDecryptor>,
    audio_decryptor: Option<AudioDecryptor>,
    /// 缓存的 FairPlay AES 密钥（首次计算后缓存）。
    aes_key_cache: Option<[u8; 16]>,
}

impl AirPlay {
    pub fn new() -> Self {
        Self {
            pairing: Pairing::new(),
            fairplay_setup: FairPlaySetup::new(),
            rtsp: RTSP::new(),
            video_decryptor: None,
            audio_decryptor: None,
            aes_key_cache: None,
        }
    }

    /// /pair-setup：返回服务器 Ed25519 公钥（32 字节）。
    pub fn pair_setup(&self) -> Vec<u8> {
        self.pairing.public_key().to_vec()
    }

    /// /pair-verify：根据请求首字节分发 step 1 / step 2。
    ///
    /// 读取首字节作为 flag：
    /// - flag > 0 → step 1（ECDH 握手），返回 96 字节（32 ecdh_ours + 64 encSig）
    /// - flag == 0 → step 2（验证签名），返回空 Vec
    pub fn pair_verify(&mut self, request: &[u8]) -> Result<Vec<u8>, AirPlayError> {
        let flag = request.first().copied().unwrap_or(0);
        if flag > 0 {
            // Step 1
            let response = self.pairing.pair_verify_step1(request)?;
            Ok(response)
        } else {
            // Step 2
            self.pairing.pair_verify_step2(request)?;
            Ok(Vec::new())
        }
    }

    /// 配对是否已验证。
    pub fn is_pair_verified(&self) -> bool {
        self.pairing.is_pair_verified()
    }

    /// /fp-setup：FairPlay 握手。
    pub fn fairplay_setup(&mut self, request: &[u8]) -> Result<Vec<u8>, AirPlayError> {
        // 每次调用可能更新 key_msg，使 AES key 缓存失效
        let result = self.fairplay_setup.fair_play_setup(request)?;
        if request.len() == 164 {
            // key_msg 已更新，缓存失效
            self.aes_key_cache = None;
        }
        Ok(result)
    }

    /// RTSP SETUP：解析 plist，保存 ekey/eiv 或返回流信息。
    pub fn rtsp_setup(&mut self, payload: &[u8]) -> Result<Option<MediaStreamInfo>, AirPlayError> {
        let result = self.rtsp.setup(payload)?;
        // 如果 ekey/eiv 发生变化，AES key 缓存可能需要失效
        // （但实际上 ekey 只在第一次 SETUP 时设置，之后不变）
        Ok(result)
    }

    /// RTSP TEARDOWN：解析 plist，返回流信息。
    pub fn rtsp_teardown(&mut self, payload: &[u8]) -> Result<Option<MediaStreamInfo>, AirPlayError> {
        self.rtsp.teardown(payload).map_err(Into::into)
    }

    /// 获取 FairPlay AES 密钥（带缓存）。
    ///
    /// 原实现每次调用都重新解密 ekey，Rust 优化为首次解密后缓存。
    pub fn get_fairplay_aes_key(&mut self) -> Result<[u8; 16], AirPlayError> {
        if let Some(cached) = self.aes_key_cache {
            return Ok(cached);
        }
        let ekey = self.rtsp.ekey().ok_or(AirPlayError::NoEkey)?;
        let aes_key = self.fairplay_setup.decrypt_aes_key(ekey);
        self.aes_key_cache = Some(aes_key);
        Ok(aes_key)
    }

    /// 视频解密器是否就绪（sharedSecret + ekey + streamConnectionID 均可用）。
    pub fn is_video_decryptor_ready(&self) -> bool {
        self.pairing.shared_secret().is_some()
            && self.rtsp.ekey().is_some()
            && self.rtsp.stream_connection_id().is_some()
    }

    /// 重置视频解密器（每个新视频 TCP 连接开始时调用）。
    ///
    /// `VideoDecryptor` 是有状态的 AES-CTR 流密码，计数器跨包累积。
    /// 当 iPhone 重连视频端口时，新连接期望计数器从 0 开始，
    /// 若复用旧解密器会导致计数器错位 → 输出全是密文垃圾。
    pub fn reset_video_decryptor(&mut self) {
        if self.video_decryptor.is_some() {
            tracing::info!("重置视频解密器（新 TCP 连接）");
        }
        self.video_decryptor = None;
    }

    /// 重置音频解密器（每个新音频 TCP 连接开始时调用）。
    pub fn reset_audio_decryptor(&mut self) {
        if self.audio_decryptor.is_some() {
            tracing::info!("重置音频解密器（新 TCP 连接）");
        }
        self.audio_decryptor = None;
    }

    /// 音频解密器是否就绪（sharedSecret + ekey + eiv 均可用）。
    pub fn is_audio_decryptor_ready(&self) -> bool {
        self.pairing.shared_secret().is_some()
            && self.rtsp.ekey().is_some()
            && self.rtsp.eiv().is_some()
    }

    /// 解密视频帧（原地修改）。
    ///
    /// 首次调用时懒加载 `VideoDecryptor`，需要 sharedSecret + ekey + streamConnectionID。
    pub fn decrypt_video(&mut self, video: &mut [u8]) -> Result<(), AirPlayError> {
        if self.video_decryptor.is_none() {
            let aes_key = self.get_fairplay_aes_key()?;
            let shared_secret = self
                .pairing
                .shared_secret()
                .ok_or(AirPlayError::NoSharedSecret)?;
            let conn_id = self
                .rtsp
                .stream_connection_id()
                .ok_or(AirPlayError::NoStreamConnectionId)?;
            self.video_decryptor = Some(VideoDecryptor::new(
                &aes_key,
                shared_secret,
                conn_id,
            )?);
        }
        self.video_decryptor.as_mut().unwrap().decrypt(video);
        Ok(())
    }

    /// 解密音频帧（原地修改）。
    ///
    /// 首次调用时懒加载 `AudioDecryptor`，需要 sharedSecret + ekey + eiv。
    pub fn decrypt_audio(
        &mut self,
        audio: &mut [u8],
        audio_length: usize,
    ) -> Result<(), AirPlayError> {
        if self.audio_decryptor.is_none() {
            let aes_key = self.get_fairplay_aes_key()?;
            let shared_secret = self
                .pairing
                .shared_secret()
                .ok_or(AirPlayError::NoSharedSecret)?;
            let eiv = self.rtsp.eiv().ok_or(AirPlayError::NoEiv)?;
            self.audio_decryptor = Some(AudioDecryptor::new(
                &aes_key,
                eiv,
                shared_secret,
            )?);
        }
        self.audio_decryptor
            .as_ref()
            .unwrap()
            .decrypt(audio, audio_length)?;
        Ok(())
    }

    /// 共享密钥（ECDH secret），pair-verify step 1 后可用。
    pub fn shared_secret(&self) -> Option<&[u8; 32]> {
        self.pairing.shared_secret()
    }

    /// 流连接 ID，RTSP SETUP (video) 后可用。
    pub fn stream_connection_id(&self) -> Option<&str> {
        self.rtsp.stream_connection_id()
    }
}

impl Default for AirPlay {
    fn default() -> Self {
        Self::new()
    }
}
