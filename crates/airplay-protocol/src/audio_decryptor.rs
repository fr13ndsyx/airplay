use aes::Aes128;
use cbc::Decryptor as CbcDecryptor;
use cipher::{block_padding::NoPadding, BlockDecryptMut, KeyIvInit};
use sha2::{Digest, Sha512};
use thiserror::Error;

type Aes128CbcDec = CbcDecryptor<Aes128>;

#[derive(Debug, Error)]
pub enum AudioDecryptorError {
    #[error("invalid AES key/IV length")]
    InvalidKeyIv,
    #[error("AES-CBC decryption failed")]
    Decrypt,
}

pub struct AudioDecryptor {
    aes_iv: Vec<u8>,
    eaes_key: [u8; 16],
}

impl AudioDecryptor {
    pub fn new(aes_key: &[u8], aes_iv: &[u8], shared_secret: &[u8]) -> Result<Self, AudioDecryptorError> {
        // eaesKey = SHA-512(aesKey || sharedSecret)[0..16]
        let mut hasher = Sha512::new();
        hasher.update(aes_key);
        hasher.update(shared_secret);
        let digest = hasher.finalize();
        let mut eaes_key = [0u8; 16];
        eaes_key.copy_from_slice(&digest[..16]);

        Ok(Self {
            aes_iv: aes_iv.to_vec(),
            eaes_key,
        })
    }

    /// 每次 decrypt 调用都重新初始化 IV（初始化 AES-CBC 密码器）。
    /// AES-CBC / NoPadding 解密；只解密 audio_length / 16 * 16 字节，末尾不足 16 字节保留原样。
    pub fn decrypt(&self, audio: &mut [u8], audio_length: usize) -> Result<(), AudioDecryptorError> {
        let n = audio_length / 16 * 16;
        if n == 0 {
            return Ok(());
        }
        // 每次调用重新初始化 IV（初始化 AES-CBC 密码器）
        let decryptor = Aes128CbcDec::new_from_slices(&self.eaes_key, &self.aes_iv)
            .map_err(|_| AudioDecryptorError::InvalidKeyIv)?;
        decryptor
            .decrypt_padded_mut::<NoPadding>(&mut audio[..n])
            .map_err(|_| AudioDecryptorError::Decrypt)?;
        Ok(())
    }
}
