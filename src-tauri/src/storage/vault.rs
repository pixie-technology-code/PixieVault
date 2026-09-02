use crate::auth::{
    CryptoError, EncryptedPayload, MasterKey, ProtectorEntry, VaultCrypto, LEGACY_GLOBAL_SALT,
};
use base64::prelude::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

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
    #[error("Vault not initialized")]
    NotInitialized,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSettings {
    pub auto_launch_last_app: bool,
    pub last_opened_app: Option<String>,
    pub theme: String,
    pub auto_lock_minutes: u32,
    #[serde(default)]
    pub is_secured: bool, // true if user configured custom passphrase, false if initial unconfigured
}

impl Default for VaultSettings {
    fn default() -> Self {
        Self {
            auto_launch_last_app: true,
            last_opened_app: None,
            theme: "Slate Dark".into(),
            auto_lock_minutes: 15,
            is_secured: false,
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
    #[serde(default)]
    pub app_files: HashMap<String, HashMap<String, Vec<u8>>>,
}

impl Default for VaultData {
    fn default() -> Self {
        Self {
            version: 3,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            settings: VaultSettings::default(),
            apps: HashMap::new(),
            app_files: HashMap::new(),
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

    pub fn get_app_files(&self, app_id: &str) -> Option<&HashMap<String, Vec<u8>>> {
        self.app_files.get(app_id)
    }

    pub fn set_app_files(&mut self, app_id: &str, files: HashMap<String, Vec<u8>>) {
        self.app_files.insert(app_id.to_string(), files);
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

    pub fn is_initialized(&self) -> bool {
        self.vault_path.exists() || self.vault_path.with_extension("pvlt.bak").exists()
    }

    /// Read raw EncryptedPayload from disk (or backup file if primary missing/corrupted)
    pub fn load_payload(&self) -> Result<EncryptedPayload, StorageError> {
        let target = if self.vault_path.exists() {
            &self.vault_path
        } else {
            let bak = self.vault_path.with_extension("pvlt.bak");
            if bak.exists() {
                return Ok(Self::read_payload_from_file(&bak)?);
            }
            return Err(StorageError::NotInitialized);
        };

        match Self::read_payload_from_file(target) {
            Ok(p) => Ok(p),
            Err(e) => {
                let bak = self.vault_path.with_extension("pvlt.bak");
                if bak.exists() {
                    if let Ok(backup_payload) = Self::read_payload_from_file(&bak) {
                        eprintln!("[VaultStorage] Primary payload corrupted ({:?}). Recovered from .bak!", e);
                        return Ok(backup_payload);
                    }
                }
                Err(e)
            }
        }
    }

    fn read_payload_from_file(path: &Path) -> Result<EncryptedPayload, StorageError> {
        let raw = fs::read_to_string(path)?;
        let payload: EncryptedPayload = serde_json::from_str(&raw)?;
        Ok(payload)
    }

    /// Read Vault ID if vault exists
    pub fn get_vault_id(&self) -> Result<Option<String>, StorageError> {
        if !self.is_initialized() {
            return Ok(None);
        }
        let payload = self.load_payload()?;
        if payload.vault_id.is_empty() {
            Ok(None)
        } else {
            Ok(Some(payload.vault_id))
        }
    }

    /// Read all enrolled Protector entries
    pub fn get_protector_entries(&self) -> Result<Vec<ProtectorEntry>, StorageError> {
        if !self.is_initialized() {
            return Ok(vec![]);
        }
        let payload = self.load_payload()?;
        Ok(payload.protectors)
    }

    /// Read salt from container header or argon2id protector entry if vault exists
    pub fn get_salt(&self) -> Result<Option<Vec<u8>>, StorageError> {
        if !self.is_initialized() {
            return Ok(None);
        }
        let payload = self.load_payload()?;

        // Check argon2id protector first (v3 envelope)
        if let Some(argon_entry) = payload.protectors.iter().find(|p| p.protector_type == "argon2id") {
            if let Some(ref salt_str) = argon_entry.salt_b64 {
                let salt = BASE64_STANDARD
                    .decode(salt_str)
                    .map_err(|_| CryptoError::InvalidFormat)?;
                return Ok(Some(salt));
            }
        }

        // Fallback to legacy v2 header salt
        if !payload.salt_b64.is_empty() {
            let salt = BASE64_STANDARD
                .decode(&payload.salt_b64)
                .map_err(|_| CryptoError::InvalidFormat)?;
            return Ok(Some(salt));
        }

        Ok(Some(LEGACY_GLOBAL_SALT.to_vec()))
    }

    /// Read and decrypt vault data from disk using the MasterKey with strict authentication check
    pub fn load(&self, key: &MasterKey) -> Result<VaultData, StorageError> {
        if !self.vault_path.exists() {
            let backup_path = self.vault_path.with_extension("pvlt.bak");
            if backup_path.exists() {
                if let Ok(data) = Self::load_file(&backup_path, key) {
                    eprintln!("[VaultStorage] Recovered vault data from backup copy");
                    return Ok(data);
                }
            }
            return Err(StorageError::NotInitialized);
        }

        match Self::load_file(&self.vault_path, key) {
            Ok(data) => Ok(data),
            Err(e) => {
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

    /// Save an existing EncryptedPayload directly with cloud-safe durability and atomic replace
    pub fn save_payload(&self, payload: &EncryptedPayload) -> Result<(), StorageError> {
        let serialized = serde_json::to_string_pretty(payload)?;

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

        // 3. Backup preservation
        let backup_path = self.vault_path.with_extension("pvlt.bak");
        if self.vault_path.exists() {
            let _ = fs::copy(&self.vault_path, &backup_path);
        }

        // 4. Atomic replacement (with retry loop for Windows / cloud sync locks like OneDrive)
        #[cfg(target_os = "windows")]
        {
            let mut replaced = false;
            for _ in 0..10 {
                if self.vault_path.exists() {
                    let _ = fs::remove_file(&self.vault_path);
                }
                if fs::rename(&temp_path, &self.vault_path).is_ok() {
                    replaced = true;
                    break;
                }
                if fs::copy(&temp_path, &self.vault_path).is_ok() {
                    let _ = fs::remove_file(&temp_path);
                    replaced = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            if !replaced {
                return Err(StorageError::Io(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "Failed to replace vault file after sync retries",
                )));
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            fs::rename(&temp_path, &self.vault_path)?;
        }

        Ok(())
    }

    /// Encrypt and atomically save vault data to disk with envelope preservation (v3)
    pub fn save(
        &self,
        data: &VaultData,
        key: &MasterKey,
        salt: &[u8],
        is_initial_state: bool,
    ) -> Result<(), StorageError> {
        let plaintext = serde_json::to_vec(data)?;
        let (ciphertext, nonce) = VaultCrypto::encrypt(key, &plaintext)?;

        // Load existing payload to preserve vault_id and device-bound protectors (multi-device OneDrive sync)
        let mut existing_payload = self.load_payload().unwrap_or_else(|_| {
            let vault_id = Uuid::new_v4().to_string();
            EncryptedPayload {
                version: 3,
                vault_id,
                protectors: vec![],
                salt_b64: "".into(),
                nonce_b64: "".into(),
                ciphertext_b64: "".into(),
                is_initial_state,
            }
        });

        if existing_payload.vault_id.is_empty() {
            existing_payload.vault_id = Uuid::new_v4().to_string();
        }

        // Update Argon2id protector if passphrase is provided or if creating new envelope
        let passphrase_for_wrap = if is_initial_state {
            crate::auth::DEFAULT_UNSECURED_PASSPHRASE
        } else {
            // Re-wrap or retain existing protector
            ""
        };

        // If argon2id protector does not exist or we need to initialize it:
        if !existing_payload.protectors.iter().any(|p| p.protector_type == "argon2id") {
            let argon_entry = VaultCrypto::wrap_master_key_argon2id(key, passphrase_for_wrap, salt)?;
            existing_payload.protectors.push(argon_entry);
        }

        existing_payload.version = 3;
        existing_payload.nonce_b64 = BASE64_STANDARD.encode(nonce);
        existing_payload.ciphertext_b64 = BASE64_STANDARD.encode(ciphertext);
        existing_payload.is_initial_state = is_initial_state;

        self.save_payload(&existing_payload)
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
            version: 3,
            vault_id: Uuid::new_v4().to_string(),
            protectors: vec![],
            salt_b64: "".into(),
            nonce_b64: BASE64_STANDARD.encode(nonce),
            ciphertext_b64: BASE64_STANDARD.encode(ciphertext),
            is_initial_state: false,
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
    fn test_vault_storage_save_and_load_v3() {
        let temp_dir = std::env::temp_dir();
        let test_vault_path = temp_dir.join(format!("test_vault_storage_{}.pvlt", Uuid::new_v4()));
        let storage = VaultStorage::new(Some(test_vault_path.clone()));

        let salt = VaultCrypto::generate_salt();
        let key = MasterKey([42u8; 32]);
        let mut data = VaultData::default();
        data.apps.insert(
            "cairn_dead_reckoning".into(),
            serde_json::json!({
                "netWorth": 1850000,
                "portfolioBalance": 1420000
            }),
        );
        let mut app_files = HashMap::new();
        app_files.insert("mac_finder.db".to_string(), b"BINARY_SQLITE_DATA".to_vec());
        data.set_app_files("mikrotik_fleet", app_files);

        storage
            .save(&data, &key, &salt, false)
            .expect("Failed to save vault data");

        let loaded = storage.load(&key).expect("Failed to load vault data");

        assert_eq!(
            loaded.apps.get("cairn_dead_reckoning"),
            data.apps.get("cairn_dead_reckoning")
        );
        assert_eq!(
            loaded.get_app_files("mikrotik_fleet"),
            data.get_app_files("mikrotik_fleet")
        );

        // Verify envelope headers
        let vault_id = storage.get_vault_id().expect("get_vault_id");
        assert!(vault_id.is_some());

        let protectors = storage.get_protector_entries().expect("get_protector_entries");
        assert_eq!(protectors.len(), 1);
        assert_eq!(protectors[0].protector_type, "argon2id");

        // Test wrong key fails strictly
        let wrong_key = MasterKey([99u8; 32]);
        let load_result = storage.load(&wrong_key);
        assert!(load_result.is_err());

        // Clean up
        let _ = fs::remove_file(&test_vault_path);
        let _ = fs::remove_file(test_vault_path.with_extension("pvlt.bak"));
    }
}
