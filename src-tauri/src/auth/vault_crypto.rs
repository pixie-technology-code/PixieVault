use super::protector::ProtectorEntry;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, Params};
use base64::prelude::*;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub const LEGACY_GLOBAL_SALT: &[u8] = b"PixieVaultSalt2026";
pub const DEFAULT_UNSECURED_PASSPHRASE: &str = "";

#[derive(Error, Debug, PartialEq)]
pub enum CryptoError {
    #[error("Key derivation failed: {0}")]
    DerivationError(String),
    #[error("Encryption failed: {0}")]
    EncryptionError(String),
    #[error("Decryption failed / invalid key or corrupted ciphertext")]
    DecryptionError,
    #[error("Invalid salt or nonce format")]
    InvalidFormat,
    #[error("Protector '{0}' not found in vault envelope")]
    ProtectorNotFound(String),
}

/// Secure memory wrapper for encryption keys that zeroizes memory on drop
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct MasterKey(pub [u8; 32]);

impl std::fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MasterKey([REDACTED])")
    }
}

/// Serialized encrypted container envelope (Version 3 with multi-protectors and backwards compat)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    pub version: u32,
    #[serde(default)]
    pub vault_id: String,
    #[serde(default)]
    pub protectors: Vec<ProtectorEntry>,
    #[serde(default)]
    pub salt_b64: String, // legacy v2 field
    pub nonce_b64: String,
    pub ciphertext_b64: String,
    #[serde(default)]
    pub is_initial_state: bool,
}

pub struct VaultCrypto;

impl VaultCrypto {
    /// Generate a cryptographically random 256-bit Vault Master Key (VMK)
    pub fn generate_master_key() -> MasterKey {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        MasterKey(key)
    }

    /// Derive a 256-bit Key from a user passphrase and salt using Argon2id
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

    /// Wrap a Vault Master Key using Argon2id passphrase derivation
    pub fn wrap_master_key_argon2id(
        master_key: &MasterKey,
        passphrase: &str,
        salt: &[u8],
    ) -> Result<ProtectorEntry, CryptoError> {
        let wrapping_key = Self::derive_key(passphrase, salt)?;
        let (wrapped_bytes, nonce) = Self::encrypt(&wrapping_key, &master_key.0)?;

        let salt_b64 = BASE64_STANDARD.encode(salt);
        let nonce_b64 = BASE64_STANDARD.encode(nonce);
        let wrapped_master_key_b64 = BASE64_STANDARD.encode(wrapped_bytes);

        Ok(ProtectorEntry {
            id: "argon2id".into(),
            protector_type: "argon2id".into(),
            salt_b64: Some(salt_b64),
            nonce_b64: Some(nonce_b64),
            wrapped_master_key_b64,
            key_name: None,
            device_id: None,
            device_name: None,
            extra: None,
        })
    }

    /// Unwrap a Vault Master Key from an Argon2id ProtectorEntry
    pub fn unwrap_master_key_argon2id(
        entry: &ProtectorEntry,
        passphrase: &str,
    ) -> Result<MasterKey, CryptoError> {
        let salt_str = entry.salt_b64.as_deref().unwrap_or_default();
        let salt = if salt_str.is_empty() {
            LEGACY_GLOBAL_SALT.to_vec()
        } else {
            BASE64_STANDARD
                .decode(salt_str)
                .map_err(|_| CryptoError::InvalidFormat)?
        };

        let nonce_str = entry.nonce_b64.as_deref().unwrap_or_default();
        let nonce_vec = BASE64_STANDARD
            .decode(nonce_str)
            .map_err(|_| CryptoError::InvalidFormat)?;
        if nonce_vec.len() != 12 {
            return Err(CryptoError::InvalidFormat);
        }
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&nonce_vec);

        let wrapped_bytes = BASE64_STANDARD
            .decode(&entry.wrapped_master_key_b64)
            .map_err(|_| CryptoError::InvalidFormat)?;

        let wrapping_key = Self::derive_key(passphrase, &salt)?;
        let decrypted_vmk = Self::decrypt(&wrapping_key, &wrapped_bytes, &nonce)?;
        if decrypted_vmk.len() != 32 {
            return Err(CryptoError::DecryptionError);
        }

        let mut vmk_bytes = [0u8; 32];
        vmk_bytes.copy_from_slice(&decrypted_vmk);
        Ok(MasterKey(vmk_bytes))
    }

    /// Create a new Version 3 Envelope with a random VMK and Argon2id protector
    pub fn create_envelope_v3(
        passphrase: &str,
        plaintext: &[u8],
    ) -> Result<(EncryptedPayload, MasterKey), CryptoError> {
        let master_key = Self::generate_master_key();
        let vault_id = Uuid::new_v4().to_string();
        let salt = Self::generate_salt();

        let argon_protector = Self::wrap_master_key_argon2id(&master_key, passphrase, &salt)?;
        let (ciphertext, nonce) = Self::encrypt(&master_key, plaintext)?;

        let payload = EncryptedPayload {
            version: 3,
            vault_id,
            protectors: vec![argon_protector],
            salt_b64: "".into(),
            nonce_b64: BASE64_STANDARD.encode(nonce),
            ciphertext_b64: BASE64_STANDARD.encode(ciphertext),
            is_initial_state: passphrase.is_empty(),
        };

        Ok((payload, master_key))
    }

    /// Unlock an EncryptedPayload (v3 or legacy v2) using the master passphrase
    pub fn unlock_envelope_with_passphrase(
        payload: &EncryptedPayload,
        passphrase: &str,
    ) -> Result<(MasterKey, Vec<u8>), CryptoError> {
        if payload.version >= 3 {
            let argon_entry = payload
                .protectors
                .iter()
                .find(|p| p.protector_type == "argon2id")
                .ok_or_else(|| CryptoError::ProtectorNotFound("argon2id".into()))?;

            let master_key = Self::unwrap_master_key_argon2id(argon_entry, passphrase)?;

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

            let plaintext = Self::decrypt(&master_key, &ciphertext, &nonce)?;
            Ok((master_key, plaintext))
        } else {
            // Legacy Version 2 Direct Passphrase Derivation
            let salt = if payload.salt_b64.is_empty() {
                LEGACY_GLOBAL_SALT.to_vec()
            } else {
                BASE64_STANDARD
                    .decode(&payload.salt_b64)
                    .map_err(|_| CryptoError::InvalidFormat)?
            };

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
            let plaintext = Self::decrypt(&key, &ciphertext, &nonce)?;
            Ok((key, plaintext))
        }
    }

    /// Update the master passphrase in an envelope without changing the VMK or invalidating hardware protectors
    pub fn update_passphrase_in_envelope(
        payload: &mut EncryptedPayload,
        master_key: &MasterKey,
        new_passphrase: &str,
    ) -> Result<(), CryptoError> {
        let salt = Self::generate_salt();
        let new_argon_entry = Self::wrap_master_key_argon2id(master_key, new_passphrase, &salt)?;

        if let Some(pos) = payload.protectors.iter().position(|p| p.protector_type == "argon2id") {
            payload.protectors[pos] = new_argon_entry;
        } else {
            payload.protectors.push(new_argon_entry);
        }

        payload.version = 3;
        payload.is_initial_state = new_passphrase.is_empty();
        Ok(())
    }

    /// Add or update a device hardware protector entry in the envelope
    pub fn add_or_update_device_protector(payload: &mut EncryptedPayload, entry: ProtectorEntry) {
        if let Some(pos) = payload.protectors.iter().position(|p| {
            p.id == entry.id
                || (entry.device_id.is_some() && p.device_id == entry.device_id && p.protector_type == entry.protector_type)
        }) {
            payload.protectors[pos] = entry;
        } else {
            payload.protectors.push(entry);
        }
        payload.version = 3;
    }

    /// Remove a device protector from the envelope
    pub fn remove_device_protector(payload: &mut EncryptedPayload, protector_id_or_device_id: &str) -> bool {
        let initial_len = payload.protectors.len();
        payload.protectors.retain(|p| {
            p.id != protector_id_or_device_id && p.device_id.as_deref() != Some(protector_id_or_device_id)
        });
        payload.protectors.len() < initial_len
    }

    /// Derive a deterministic application-specific subkey using HKDF-like domain separation
    pub fn derive_sub_key(master_key: &MasterKey, domain: &str) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"PixieVault::SubKey::v2::");
        hasher.update(master_key.0);
        hasher.update(domain.as_bytes());
        let result = hasher.finalize();
        let mut subkey = [0u8; 32];
        subkey.copy_from_slice(&result);
        subkey
    }

    /// Derive a Fernet-compatible base64 encryption key for microservices
    pub fn derive_fernet_key(master_key: &MasterKey, app_id: &str) -> String {
        let raw = Self::derive_sub_key(master_key, app_id);
        BASE64_URL_SAFE.encode(raw)
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
        let (payload, _) = Self::create_envelope_v3(passphrase, plaintext)?;
        Ok(payload)
    }

    /// Helper to decrypt an EncryptedPayload struct using the passphrase
    pub fn decrypt_with_passphrase(
        passphrase: &str,
        payload: &EncryptedPayload,
    ) -> Result<Vec<u8>, CryptoError> {
        let (_, plaintext) = Self::unlock_envelope_with_passphrase(payload, passphrase)?;
        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derived_microservice_key_is_fernet_compatible() {
        let master_key = MasterKey([0x5Au8; 32]);
        let encoded = VaultCrypto::derive_fernet_key(&master_key, "mikrotik_fleet_mgr");

        assert_eq!(encoded.len(), 44);
        assert!(encoded.ends_with('='));
        let decoded = BASE64_URL_SAFE
            .decode(encoded.as_bytes())
            .expect("Fernet key must be valid padded URL-safe Base64");
        assert_eq!(decoded.len(), 32);
    }

    #[test]
    fn test_envelope_v3_roundtrip_and_passphrase_rotation() {
        let password = "InitialMasterPassword123";
        let secret_data = b"{\"bhp\":505,\"torque\":470,\"curbWeight\":3180}";

        let (mut payload, master_key) =
            VaultCrypto::create_envelope_v3(password, secret_data).expect("Envelope creation failed");
        assert_eq!(payload.version, 3);
        assert!(!payload.vault_id.is_empty());
        assert_eq!(payload.protectors.len(), 1);
        assert_eq!(payload.protectors[0].protector_type, "argon2id");

        // Unlock with correct password
        let (unlocked_vmk, decrypted) =
            VaultCrypto::unlock_envelope_with_passphrase(&payload, password).expect("Unlock failed");
        assert_eq!(decrypted, secret_data);
        assert_eq!(unlocked_vmk.0, master_key.0);

        // Wrong password fails
        let wrong_res = VaultCrypto::unlock_envelope_with_passphrase(&payload, "WrongPassword");
        assert!(wrong_res.is_err());

        // Rotate password without touching ciphertext or changing VMK
        let new_password = "RotatedMasterPassword456";
        VaultCrypto::update_passphrase_in_envelope(&mut payload, &master_key, new_password)
            .expect("Update passphrase failed");

        // Old password no longer works
        assert!(VaultCrypto::unlock_envelope_with_passphrase(&payload, password).is_err());

        // New password unlocks the exact same VMK and data
        let (rotated_vmk, rotated_decrypted) =
            VaultCrypto::unlock_envelope_with_passphrase(&payload, new_password)
                .expect("Unlock with new password failed");
        assert_eq!(rotated_decrypted, secret_data);
        assert_eq!(rotated_vmk.0, master_key.0);
    }

    #[test]
    fn test_legacy_v2_payload_backward_compatibility() {
        let password = "LegacyPassword123";
        let secret_data = b"Legacy vault data payload";

        // Simulate a legacy v2 payload
        let salt = VaultCrypto::generate_salt();
        let key = VaultCrypto::derive_key(password, &salt).unwrap();
        let (ciphertext, nonce) = VaultCrypto::encrypt(&key, secret_data).unwrap();

        let v2_payload = EncryptedPayload {
            version: 2,
            vault_id: "".into(),
            protectors: vec![],
            salt_b64: BASE64_STANDARD.encode(salt),
            nonce_b64: BASE64_STANDARD.encode(nonce),
            ciphertext_b64: BASE64_STANDARD.encode(ciphertext),
            is_initial_state: false,
        };

        // unlock_envelope_with_passphrase transparently reads v2
        let (unlocked_key, decrypted) =
            VaultCrypto::unlock_envelope_with_passphrase(&v2_payload, password).expect("v2 unlock");
        assert_eq!(decrypted, secret_data);
        assert_eq!(unlocked_key.0, key.0);
    }
}
