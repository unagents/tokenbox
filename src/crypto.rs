//! Derives the store file name and the encryption key from a key name, and
//! encrypts / decrypts values.
//!
//! One Argon2id run over the key name yields 64 bytes. The first half becomes
//! the public file name, the second half the secret encryption key. Guessing a
//! key name from a published file name therefore costs one Argon2id run per
//! guess.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use secrecy::{ExposeSecret, SecretString};

use crate::error::{Failure, Result};

/// Format version directory inside the store.
pub const FORMAT_DIR: &str = "v1";

const SALT: &[u8] = b"tokenbox/v1/key-name";
const MAGIC: &[u8] = b"TBX1";
const NONCE_LEN: usize = 24;
const ARGON2_MEMORY_KIB: u32 = 64 * 1024;
const ARGON2_PASSES: u32 = 3;
const ARGON2_LANES: u32 = 1;

pub struct Derived {
    /// File name inside `FORMAT_DIR`. Safe to publish.
    pub file_id: String,
    key: [u8; 32],
}

pub fn derive(key_name: &str) -> Result<Derived> {
    if key_name.is_empty() {
        return Err(Failure::Usage("key name must not be empty".into()));
    }
    let params = Params::new(ARGON2_MEMORY_KIB, ARGON2_PASSES, ARGON2_LANES, Some(64))
        .map_err(|e| Failure::Usage(format!("key derivation setup failed: {e}")))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = [0u8; 64];
    argon2
        .hash_password_into(key_name.as_bytes(), SALT, &mut out)
        .map_err(|e| Failure::Usage(format!("key derivation failed: {e}")))?;
    let mut key = [0u8; 32];
    key.copy_from_slice(&out[32..]);
    let file_id = out[..32].iter().map(|b| format!("{b:02x}")).collect();
    Ok(Derived { file_id, key })
}

impl Derived {
    pub fn encrypt(&self, value: &SecretString) -> Result<Vec<u8>> {
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let body = cipher
            .encrypt(&nonce, Payload { msg: value.expose_secret().as_bytes(), aad: MAGIC })
            .map_err(|_| Failure::Usage("encryption failed".into()))?;
        let mut out = Vec::with_capacity(MAGIC.len() + NONCE_LEN + body.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&body);
        Ok(out)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<SecretString> {
        let wrong = || Failure::Decrypt("cannot decrypt: the stored data does not match this key name".into());
        if data.len() < MAGIC.len() + NONCE_LEN || &data[..MAGIC.len()] != MAGIC {
            return Err(Failure::Decrypt("cannot decrypt: unknown store format".into()));
        }
        let (nonce, body) = data[MAGIC.len()..].split_at(NONCE_LEN);
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let plain = cipher
            .decrypt(XNonce::from_slice(nonce), Payload { msg: body, aad: MAGIC })
            .map_err(|_| wrong())?;
        let text = String::from_utf8(plain).map_err(|_| wrong())?;
        Ok(SecretString::from(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let d = derive("service-token-test-0001").unwrap();
        let data = d.encrypt(&SecretString::from("s3cret value".to_string())).unwrap();
        assert_eq!(d.decrypt(&data).unwrap().expose_secret(), "s3cret value");
    }

    #[test]
    fn derivation_is_deterministic_and_name_specific() {
        let a = derive("name-a").unwrap();
        let a2 = derive("name-a").unwrap();
        let b = derive("name-b").unwrap();
        assert_eq!(a.file_id, a2.file_id);
        assert_ne!(a.file_id, b.file_id);
        assert_eq!(a.file_id.len(), 64);
    }

    #[test]
    fn other_key_name_cannot_decrypt() {
        let a = derive("name-a").unwrap();
        let b = derive("name-b").unwrap();
        let data = a.encrypt(&SecretString::from("value".to_string())).unwrap();
        assert!(matches!(b.decrypt(&data), Err(Failure::Decrypt(_))));
    }

    #[test]
    fn file_does_not_contain_key_name_or_value() {
        let d = derive("visible-name").unwrap();
        let data = d.encrypt(&SecretString::from("visible-value".to_string())).unwrap();
        let hay = String::from_utf8_lossy(&data);
        assert!(!hay.contains("visible-name"));
        assert!(!hay.contains("visible-value"));
        assert!(!d.file_id.contains("visible-name"));
    }
}
