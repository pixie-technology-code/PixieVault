use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, Params};
use base64::prelude::*;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Key derivation failed: {0}")]
    DerivationError(String),
    #[error("Encryption failed: {0}")]
    EncryptionError(String),
    #[error("Decryption failed / invalid key or corrupted ciphertext")]
    DecryptionError,
    #[error("Invalid salt or nonce format")]
    InvalidFormat,
}

/// Secure memory wrapper for encryption keys that zeroizes memory on drop
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct MasterKey(pub [u8; 32]);

/// Serialized encrypted container envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    pub version: u32,
    pub salt_b64: String,
    pub nonce_b64: String,
    pub ciphertext_b64: String,
}

pub struct VaultCrypto;

impl VaultCrypto {
    /// Derive a 256-bit Master Key from a user passphrase and salt using Argon2id
    pub fn derive_key(passphrase: &str, salt: &[u8]) -> Result<MasterKey, CryptoError> {
        let params = Params::new(64 * 1024, 3, 4, Some(32))
            .map_err(|e| CryptoError::DerivationError(e.to_string()))?;
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);

        let mut output_key = [0u8; 32];
        argon2
            .hash_password_into(passphrase.as_bytes(), salt, &mut output_key)
            .map_err(|e| CryptoError::DerivationError(e.to_string()))?;

        Ok(MasterKey(output_key))
    }

    /// Generate a cryptographically secure 16-byte random salt
    pub fn generate_salt() -> [u8; 16] {
        let mut salt = [0u8; 16];
        OsRng.fill_bytes(&mut salt);
        salt
    }

    /// Encrypt arbitrary plaintext bytes using AES-256-GCM
    pub fn encrypt(key: &MasterKey, plaintext: &[u8]) -> Result<(Vec<u8>, [u8; 12]), CryptoError> {
        let cipher = Aes256Gcm::new_from_slice(&key.0)
            .map_err(|e| CryptoError::EncryptionError(e.to_string()))?;

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| CryptoError::EncryptionError(e.to_string()))?;

        Ok((ciphertext, nonce_bytes))
    }

    /// Decrypt AES-256-GCM ciphertext using the Master Key and nonce
    pub fn decrypt(
        key: &MasterKey,
        ciphertext: &[u8],
        nonce_bytes: &[u8; 12],
    ) -> Result<Vec<u8>, CryptoError> {
        let cipher = Aes256Gcm::new_from_slice(&key.0).map_err(|_| CryptoError::DecryptionError)?;

        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| CryptoError::DecryptionError)?;

        Ok(plaintext)
    }

    /// Helper to package plaintext into a full EncryptedPayload struct with Argon2id derivation
    pub fn encrypt_with_passphrase(
        passphrase: &str,
        plaintext: &[u8],
    ) -> Result<EncryptedPayload, CryptoError> {
        let salt = Self::generate_salt();
        let key = Self::derive_key(passphrase, &salt)?;
        let (ciphertext, nonce) = Self::encrypt(&key, plaintext)?;

        let salt_b64 = BASE64_STANDARD.encode(salt);
        let nonce_b64 = BASE64_STANDARD.encode(nonce);
        let ciphertext_b64 = BASE64_STANDARD.encode(ciphertext);

        Ok(EncryptedPayload {
            version: 1,
            salt_b64,
            nonce_b64,
            ciphertext_b64,
        })
    }

    /// Helper to decrypt an EncryptedPayload struct using the passphrase
    pub fn decrypt_with_passphrase(
        passphrase: &str,
        payload: &EncryptedPayload,
    ) -> Result<Vec<u8>, CryptoError> {
        let salt = BASE64_STANDARD
            .decode(&payload.salt_b64)
            .map_err(|_| CryptoError::InvalidFormat)?;

        let nonce_vec = BASE64_STANDARD
            .decode(&payload.nonce_b64)
            .map_err(|_| CryptoError::InvalidFormat)?;
        if nonce_vec.len() != 12 {
            return Err(CryptoError::InvalidFormat);
        }
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&nonce_vec);

        let ciphertext = BASE64_STANDARD
            .decode(&payload.ciphertext_b64)
            .map_err(|_| CryptoError::InvalidFormat)?;

        let key = Self::derive_key(passphrase, &salt)?;
        Self::decrypt(&key, &ciphertext, &nonce)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argon2_and_aes_roundtrip() {
        let password = "SuperSecretPixieVaultMasterKey!123";
        let secret_data = b"{\"bhp\":505,\"torque\":470,\"curbWeight\":3180}";

        let encrypted =
            VaultCrypto::encrypt_with_passphrase(password, secret_data).expect("Encryption failed");
        assert_ne!(encrypted.ciphertext_b64.as_bytes(), secret_data);

        let decrypted =
            VaultCrypto::decrypt_with_passphrase(password, &encrypted).expect("Decryption failed");
        assert_eq!(decrypted, secret_data);
    }

    #[test]
    fn test_invalid_password_fails() {
        let password = "CorrectPassword123";
        let secret_data = b"Confidential data payload";

        let encrypted =
            VaultCrypto::encrypt_with_passphrase(password, secret_data).expect("Encryption failed");
        let result = VaultCrypto::decrypt_with_passphrase("WrongPassword456", &encrypted);
        assert!(result.is_err());
    }
}
