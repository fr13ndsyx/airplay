use aes::Aes128;
use cipher::{KeyIvInit, StreamCipher};
use ctr::Ctr128BE;
use sha2::{Digest, Sha512};
use thiserror::Error;

type AesCtr = Ctr128BE<Aes128>;

#[derive(Debug, Error)]
pub enum VideoDecryptorError {
    #[error("cipher error: {0}")]
    Cipher(String),
}

pub struct VideoDecryptor {
    cipher: AesCtr,
    og: [u8; 16],
    next_decrypt_count: usize,
}

impl VideoDecryptor {
    pub fn new(
        aes_key: &[u8],
        shared_secret: &[u8],
        stream_connection_id: &str,
    ) -> Result<Self, VideoDecryptorError> {
        // eaesKey = SHA-512(aesKey || sharedSecret)
        let mut hasher = Sha512::new();
        hasher.update(aes_key);
        hasher.update(shared_secret);
        let eaes_key = hasher.finalize();

        // decryptKey = SHA-512("AirPlayStreamKey" || streamConnectionID || eaesKey[0..16])[0..16]
        let mut hasher = Sha512::new();
        hasher.update(b"AirPlayStreamKey");
        hasher.update(stream_connection_id.as_bytes());
        hasher.update(&eaes_key[..16]);
        let hash1 = hasher.finalize();

        // decryptIV = SHA-512("AirPlayStreamIV" || streamConnectionID || eaesKey[0..16])[0..16]
        let mut hasher = Sha512::new();
        hasher.update(b"AirPlayStreamIV");
        hasher.update(stream_connection_id.as_bytes());
        hasher.update(&eaes_key[..16]);
        let hash2 = hasher.finalize();

        let mut decrypt_key = [0u8; 16];
        let mut decrypt_iv = [0u8; 16];
        decrypt_key.copy_from_slice(&hash1[..16]);
        decrypt_iv.copy_from_slice(&hash2[..16]);

        let cipher = AesCtr::new_from_slices(&decrypt_key, &decrypt_iv)
            .map_err(|e| VideoDecryptorError::Cipher(e.to_string()))?;

        // 诊断：打印密钥推导参数
        tracing::info!("VideoDecryptor 创建:");
        tracing::info!("  aes_key ({} 字节): {:02X?}", aes_key.len(), aes_key);
        tracing::info!("  shared_secret ({} 字节): {:02X?}", shared_secret.len(), shared_secret);
        tracing::info!("  stream_connection_id: {:?}", stream_connection_id);
        tracing::info!("  eaes_key[0..16]: {:02X?}", &eaes_key[..16]);
        tracing::info!("  decrypt_key: {:02X?}", decrypt_key);
        tracing::info!("  decrypt_iv: {:02X?}", decrypt_iv);

        Ok(Self {
            cipher,
            og: [0u8; 16],
            next_decrypt_count: 0,
        })
    }

    pub fn decrypt(&mut self, video: &mut [u8]) {
        let ndc = self.next_decrypt_count;

        // Step 1: XOR first ndc bytes with og tail
        if ndc > 0 {
            for i in 0..ndc {
                video[i] ^= self.og[(16 - ndc) + i];
            }
        }

        // Step 2: AES-CTR decrypt the middle portion
        let encryptlen = ((video.len() - ndc) / 16) * 16;
        if encryptlen > 0 {
            self.cipher
                .apply_keystream(&mut video[ndc..ndc + encryptlen]);
        }

        // Step 3: Handle tail
        let restlen = (video.len() - ndc) % 16;
        let reststart = video.len() - restlen;
        self.next_decrypt_count = 0;

        if restlen > 0 {
            self.og.fill(0);
            self.og[..restlen].copy_from_slice(&video[reststart..]);
            // AES-CTR decrypt full 16-byte og block (advances counter by 1)
            self.cipher.apply_keystream(&mut self.og);
            video[reststart..].copy_from_slice(&self.og[..restlen]);
            self.next_decrypt_count = 16 - restlen;
        }
    }
}
