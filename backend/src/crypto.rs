use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::app_error::{AppError, AppResult};

pub struct CryptoBox {
    cipher: Aes256Gcm,
}

impl CryptoBox {
    pub fn new(secret: &str) -> Self {
        let key = Sha256::digest(secret.as_bytes());
        let cipher = Aes256Gcm::new_from_slice(&key).expect("sha256 gives 32 bytes");
        Self { cipher }
    }

    pub fn encrypt(&self, plaintext: &str) -> AppResult<String> {
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|_| AppError::Internal("Failed to encrypt setting.".into()))?;

        Ok(format!(
            "v1:{}:{}",
            STANDARD.encode(nonce_bytes),
            STANDARD.encode(ciphertext)
        ))
    }

    pub fn decrypt(&self, value: &str) -> AppResult<String> {
        let mut parts = value.split(':');
        let version = parts.next();
        let nonce = parts.next();
        let ciphertext = parts.next();

        if version != Some("v1") {
            return Err(AppError::Internal("Unsupported encrypted setting version.".into()));
        }

        let nonce = STANDARD
            .decode(nonce.unwrap_or_default())
            .map_err(|_| AppError::Internal("Invalid encrypted setting nonce.".into()))?;
        let ciphertext = STANDARD
            .decode(ciphertext.unwrap_or_default())
            .map_err(|_| AppError::Internal("Invalid encrypted setting payload.".into()))?;

        let plaintext = self
            .cipher
            .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
            .map_err(|_| AppError::Internal("Failed to decrypt setting.".into()))?;

        String::from_utf8(plaintext).map_err(|_| AppError::Internal("Invalid UTF-8 setting.".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::CryptoBox;

    #[test]
    fn encrypts_and_decrypts_value() {
        let crypto = CryptoBox::new("test-secret");
        let encrypted = crypto.encrypt("secret-value").unwrap();
        assert_ne!(encrypted, "secret-value");
        assert_eq!(crypto.decrypt(&encrypted).unwrap(), "secret-value");
    }
}
