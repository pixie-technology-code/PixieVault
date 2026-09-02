use crate::auth::{EncryptedPayload, MasterKey, VaultCrypto};
use base64::prelude::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Crypto error: {0}")]
    Crypto(#[from] crate::auth::CryptoError),
    #[error("Vault is locked")]
    VaultLocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSettings {
    pub auto_launch_last_app: bool,
    pub last_opened_app: Option<String>,
    pub theme: String,
    pub auto_lock_minutes: u32,
}

impl Default for VaultSettings {
    fn default() -> Self {
        Self {
            auto_launch_last_app: true,
            last_opened_app: None,
            theme: "Slate Dark".into(),
            auto_lock_minutes: 15,
        }
    }
}

/// Decrypted Vault Data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultData {
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub settings: VaultSettings,
    pub apps: HashMap<String, serde_json::Value>,
}

impl Default for VaultData {
    fn default() -> Self {
        Self {
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            settings: VaultSettings::default(),
            apps: HashMap::new(),
        }
    }
}

impl VaultData {
    pub fn get_app_state(&self, app_id: &str) -> Option<&serde_json::Value> {
        self.apps.get(app_id)
    }

    pub fn set_app_state(&mut self, app_id: &str, data: serde_json::Value) {
        self.apps.insert(app_id.to_string(), data);
        self.updated_at = Utc::now();
    }
}

pub struct VaultStorage {
    pub vault_path: PathBuf,
}

impl VaultStorage {
    pub fn new(custom_path: Option<PathBuf>) -> Self {
        let vault_path = custom_path.unwrap_or_else(|| {
            let mut p = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            p.push("vault_data.pvlt");
            p
        });
        Self { vault_path }
    }

    /// Read and decrypt vault data from disk using the MasterKey with automatic backup recovery
    pub fn load(&self, key: &MasterKey) -> Result<VaultData, StorageError> {
        if !self.vault_path.exists() {
            // Check if backup exists (e.g. after crash during save)
            let backup_path = self.vault_path.with_extension("pvlt.bak");
            if backup_path.exists() {
                if let Ok(data) = Self::load_file(&backup_path, key) {
                    eprintln!("[VaultStorage] Recovered vault data from backup copy");
                    return Ok(data);
                }
            }
            return Ok(VaultData::default());
        }

        match Self::load_file(&self.vault_path, key) {
            Ok(data) => Ok(data),
            Err(e) => {
                // Try recovery from backup file
                let backup_path = self.vault_path.with_extension("pvlt.bak");
                if backup_path.exists() {
                    if let Ok(backup_data) = Self::load_file(&backup_path, key) {
                        eprintln!("[VaultStorage] Primary vault file corrupted ({:?}). Successfully recovered from .bak!", e);
                        return Ok(backup_data);
                    }
                }
                Err(e)
            }
        }
    }

    fn load_file(path: &Path, key: &MasterKey) -> Result<VaultData, StorageError> {
        let raw_file = fs::read_to_string(path)?;
        let payload: EncryptedPayload = serde_json::from_str(&raw_file)?;

        let nonce_vec = BASE64_STANDARD
            .decode(&payload.nonce_b64)
            .map_err(|_| crate::auth::CryptoError::InvalidFormat)?;
        if nonce_vec.len() != 12 {
            return Err(StorageError::Crypto(
                crate::auth::CryptoError::InvalidFormat,
            ));
        }
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&nonce_vec);

        let ciphertext = BASE64_STANDARD
            .decode(&payload.ciphertext_b64)
            .map_err(|_| crate::auth::CryptoError::InvalidFormat)?;

        let plaintext = VaultCrypto::decrypt(key, &ciphertext, &nonce)?;
        let vault_data: VaultData = serde_json::from_slice(&plaintext)?;
        Ok(vault_data)
    }

    /// Encrypt and atomically save vault data to disk with durability sync, parent directory creation, and backup preservation
    pub fn save(&self, data: &VaultData, key: &MasterKey) -> Result<(), StorageError> {
        let plaintext = serde_json::to_vec(data)?;
        let (ciphertext, nonce) = VaultCrypto::encrypt(key, &plaintext)?;

        let nonce_b64 = BASE64_STANDARD.encode(nonce);
        let ciphertext_b64 = BASE64_STANDARD.encode(ciphertext);

        let payload = EncryptedPayload {
            version: 1,
            salt_b64: "".into(),
            nonce_b64,
            ciphertext_b64,
        };

        let serialized = serde_json::to_string_pretty(&payload)?;

        // 1. Ensure parent directories exist
        if let Some(parent) = self.vault_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // 2. Write to temp file with sync_all for durability
        let temp_path = self.vault_path.with_extension("pvlt.tmp");
        {
            let mut temp_file = fs::File::create(&temp_path)?;
            temp_file.write_all(serialized.as_bytes())?;
            temp_file.sync_all()?;
        }

        // 3. If existing file exists, create backup before atomic replacement
        let backup_path = self.vault_path.with_extension("pvlt.bak");
        if self.vault_path.exists() {
            let _ = fs::copy(&self.vault_path, &backup_path);
        }

        // 4. Atomic replacement (Windows-safe)
        #[cfg(target_os = "windows")]
        {
            if self.vault_path.exists() {
                let _ = fs::remove_file(&self.vault_path);
            }
            if let Err(_) = fs::rename(&temp_path, &self.vault_path) {
                fs::copy(&temp_path, &self.vault_path)?;
                let _ = fs::remove_file(&temp_path);
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            fs::rename(&temp_path, &self.vault_path)?;
        }

        Ok(())
    }

    /// Encrypt data directly to serialized bytes for portable package bundles
    pub fn encrypt_raw(
        &self,
        session: &crate::auth::VaultSession,
        data: &VaultData,
    ) -> Result<Vec<u8>, StorageError> {
        let key = session
            .active_key
            .as_ref()
            .ok_or(StorageError::VaultLocked)?;
        let plaintext = serde_json::to_vec(data)?;
        let (ciphertext, nonce) = VaultCrypto::encrypt(key, &plaintext)?;
        let payload = EncryptedPayload {
            version: 1,
            salt_b64: "".into(),
            nonce_b64: BASE64_STANDARD.encode(nonce),
            ciphertext_b64: BASE64_STANDARD.encode(ciphertext),
        };
        let serialized = serde_json::to_vec(&payload)?;
        Ok(serialized)
    }

    /// Decrypt raw bytes directly from portable package bundles
    pub fn decrypt_raw(
        &self,
        session: &crate::auth::VaultSession,
        raw_bytes: &[u8],
    ) -> Result<Vec<u8>, StorageError> {
        let key = session
            .active_key
            .as_ref()
            .ok_or(StorageError::VaultLocked)?;
        let payload: EncryptedPayload = serde_json::from_slice(raw_bytes)?;

        let nonce_vec = BASE64_STANDARD
            .decode(&payload.nonce_b64)
            .map_err(|_| crate::auth::CryptoError::InvalidFormat)?;
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&nonce_vec);

        let ciphertext = BASE64_STANDARD
            .decode(&payload.ciphertext_b64)
            .map_err(|_| crate::auth::CryptoError::InvalidFormat)?;

        let decrypted = VaultCrypto::decrypt(key, &ciphertext, &nonce)?;
        Ok(decrypted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_storage_save_and_load() {
        let temp_dir = std::env::temp_dir();
        let test_vault_path = temp_dir.join("test_vault_storage.pvlt");
        let storage = VaultStorage::new(Some(test_vault_path.clone()));

        let key = MasterKey([42u8; 32]);
        let mut data = VaultData::default();
        data.apps.insert(
            "cairn_dead_reckoning".into(),
            serde_json::json!({
                "netWorth": 1850000,
                "portfolioBalance": 1420000
            }),
        );

        storage
            .save(&data, &key)
            .expect("Failed to save vault data");
        let loaded = storage.load(&key).expect("Failed to load vault data");

        assert_eq!(
            loaded.apps.get("cairn_dead_reckoning"),
            data.apps.get("cairn_dead_reckoning")
        );

        // Test backup recovery on corrupted primary file
        fs::write(&test_vault_path, "corrupted-unparseable-data").unwrap();
        let recovered = storage.load(&key).expect("Should recover from .bak");
        assert_eq!(
            recovered.apps.get("cairn_dead_reckoning"),
            data.apps.get("cairn_dead_reckoning")
        );

        // Clean up
        let _ = fs::remove_file(&test_vault_path);
        let _ = fs::remove_file(test_vault_path.with_extension("pvlt.bak"));
    }
}
