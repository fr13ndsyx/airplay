#![allow(clippy::pedantic)]

use aes::Aes128;
use ctr::cipher::{KeyIvInit, StreamCipher};
use ctr::Ctr128BE;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use sha2::{Digest, Sha512};
use thiserror::Error;
use x25519_dalek::{EphemeralSecret, PublicKey};

#[derive(Debug, Error)]
pub enum PairingError {
    #[error("invalid request length: expected {expected}, got {actual}")]
    InvalidLength { expected: usize, actual: usize },
    #[error("ed25519 error: {0}")]
    Ed25519(#[from] ed25519_dalek::SignatureError),
    #[error("cipher error: {0}")]
    Cipher(String),
}

pub struct Pairing {
    server_signing_key: SigningKey,
    ed_theirs: Option<[u8; 32]>,
    ecdh_ours: Option<[u8; 32]>,
    ecdh_theirs: Option<[u8; 32]>,
    ecdh_secret: Option<[u8; 32]>,
    pair_verified: bool,
}

impl Pairing {
    pub fn new() -> Self {
        let mut csprng = OsRng;
        Self {
            server_signing_key: SigningKey::generate(&mut csprng),
            ed_theirs: None,
            ecdh_ours: None,
            ecdh_theirs: None,
            ecdh_secret: None,
            pair_verified: false,
        }
    }

    pub fn public_key(&self) -> [u8; 32] {
        self.server_signing_key.verifying_key().to_bytes()
    }

    pub fn pair_verify_step1(&mut self, request: &[u8]) -> Result<Vec<u8>, PairingError> {
        if request.len() < 68 {
            return Err(PairingError::InvalidLength {
                expected: 68,
                actual: request.len(),
            });
        }

        let ecdh_theirs: [u8; 32] = request[4..36].try_into().expect("32 bytes");
        let ed_theirs: [u8; 32] = request[36..68].try_into().expect("32 bytes");
        self.ecdh_theirs = Some(ecdh_theirs);
        self.ed_theirs = Some(ed_theirs);

        let mut csprng = OsRng;
        let ephemeral_secret = EphemeralSecret::random_from_rng(&mut csprng);
        let ecdh_ours = PublicKey::from(&ephemeral_secret);
        let their_public = PublicKey::from(ecdh_theirs);
        let shared_secret = ephemeral_secret.diffie_hellman(&their_public);

        let ecdh_ours_bytes = ecdh_ours.to_bytes();
        let ecdh_secret_bytes = shared_secret.to_bytes();
        self.ecdh_ours = Some(ecdh_ours_bytes);
        self.ecdh_secret = Some(ecdh_secret_bytes);

        let mut data_to_sign = [0u8; 64];
        data_to_sign[..32].copy_from_slice(&ecdh_ours_bytes);
        data_to_sign[32..].copy_from_slice(&ecdh_theirs);

        let signature: Signature = self.server_signing_key.sign(&data_to_sign);
        let mut encrypted_signature = signature.to_bytes().to_vec();

        let mut cipher = self.init_cipher()?;
        cipher.apply_keystream(&mut encrypted_signature);

        let mut response = Vec::with_capacity(96);
        response.extend_from_slice(&ecdh_ours_bytes);
        response.extend_from_slice(&encrypted_signature);
        Ok(response)
    }
    pub fn pair_verify_step2(&mut self, request: &[u8]) -> Result<(), PairingError> {
        if request.len() < 68 {
            return Err(PairingError::InvalidLength {
                expected: 68,
                actual: request.len(),
            });
        }

        let encrypted_signature: [u8; 64] = request[4..68].try_into().expect("64 bytes");

        let mut cipher = self.init_cipher()?;
        let mut keystream_skip = [0u8; 64];
        cipher.apply_keystream(&mut keystream_skip);
        let mut decrypted_signature = encrypted_signature.to_vec();
        cipher.apply_keystream(&mut decrypted_signature);

        let ecdh_theirs = self
            .ecdh_theirs
            .ok_or_else(|| PairingError::Cipher("pair-verify step 1 not run".into()))?;
        let ecdh_ours = self
            .ecdh_ours
            .ok_or_else(|| PairingError::Cipher("pair-verify step 1 not run".into()))?;
        let ed_theirs = self
            .ed_theirs
            .ok_or_else(|| PairingError::Cipher("pair-verify step 1 not run".into()))?;

        let mut signed_message = [0u8; 64];
        signed_message[..32].copy_from_slice(&ecdh_theirs);
        signed_message[32..].copy_from_slice(&ecdh_ours);

        let verifying_key = VerifyingKey::from_bytes(&ed_theirs)?;
        let sig_array: [u8; 64] = decrypted_signature.as_slice().try_into().expect("64 bytes");
        let signature = Signature::from_bytes(&sig_array);
        verifying_key.verify_strict(&signed_message, &signature)?;
        self.pair_verified = true;
        Ok(())
    }

    pub fn is_pair_verified(&self) -> bool {
        self.pair_verified
    }

    pub fn shared_secret(&self) -> Option<&[u8; 32]> {
        self.ecdh_secret.as_ref()
    }

    fn init_cipher(&self) -> Result<Ctr128BE<Aes128>, PairingError> {
        let ecdh_secret = self
            .ecdh_secret
            .ok_or_else(|| PairingError::Cipher("pair-verify step 1 not run".into()))?;

        let mut key_hash = Sha512::new();
        key_hash.update(b"Pair-Verify-AES-Key");
        key_hash.update(ecdh_secret);
        let key_digest = key_hash.finalize();
        let aes_key: [u8; 16] = key_digest[..16].try_into().expect("16 bytes");

        let mut iv_hash = Sha512::new();
        iv_hash.update(b"Pair-Verify-AES-IV");
        iv_hash.update(ecdh_secret);
        let iv_digest = iv_hash.finalize();
        let aes_iv: [u8; 16] = iv_digest[..16].try_into().expect("16 bytes");

        Ctr128BE::<Aes128>::new_from_slices(&aes_key, &aes_iv)
            .map_err(|e| PairingError::Cipher(e.to_string()))
    }
}

impl Default for Pairing {
    fn default() -> Self {
        Self::new()
    }
}