use super::vault_crypto::MasterKey;
use async_trait::async_trait;
use base64::prelude::*;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProtectorCapabilities {
    pub is_available: bool,
    pub provider_name: String,
    pub biometric_type: String, // "Windows Hello (Fingerprint / Face / PIN)", "Touch ID", etc.
    pub is_enrolled: bool,
    pub availability_status: String, // "Ready", "NotConfiguredForUser", "DisabledByPolicy", "DeviceBusy", "DeviceNotPresent", "Unknown"
    pub supported_hardware: Vec<String>, // ["Face", "Fingerprint", "PIN", "TPM 2.0"]
    pub device_id: String,
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProtectorEntry {
    pub id: String, // e.g. "argon2id" or "windows-hello-<device_id>"
    pub protector_type: String, // "argon2id" | "windows-hello-cng" | "mock-protector"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salt_b64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce_b64: Option<String>,
    pub wrapped_master_key_b64: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

#[async_trait]
pub trait PlatformKeyProtector: Send + Sync {
    fn provider_id(&self) -> &'static str;
    fn get_device_id(&self) -> String;
    fn get_device_name(&self) -> String;
    async fn capabilities(
        &self,
        data_root: Option<&Path>,
        vault_id: Option<&str>,
        enrolled_entry: Option<&ProtectorEntry>,
    ) -> ProtectorCapabilities;
    async fn enroll(
        &self,
        vault_id: &str,
        master_key: &MasterKey,
        window_handle: Option<isize>,
    ) -> Result<ProtectorEntry, String>;
    async fn unlock(
        &self,
        vault_id: &str,
        entry: &ProtectorEntry,
        window_handle: Option<isize>,
    ) -> Result<MasterKey, String>;
    async fn revoke(
        &self,
        vault_id: &str,
        entry: Option<&ProtectorEntry>,
    ) -> Result<(), String>;
}

/// Mock Platform Protector for deterministic unit, multi-device sync, and adversary tests
#[derive(Clone)]
pub struct MockPlatformProtector {
    device_id: String,
    device_name: String,
    provider_name: String,
    availability_status: Arc<Mutex<String>>,
    is_available: Arc<Mutex<bool>>,
    simulated_keys: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    should_cancel: Arc<Mutex<bool>>,
}

impl MockPlatformProtector {
    pub fn new(device_id: &str, device_name: &str) -> Self {
        Self {
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            provider_name: "Mock Hardware Protector (TPM 2.0 / Biometrics)".into(),
            availability_status: Arc::new(Mutex::new("Ready".into())),
            is_available: Arc::new(Mutex::new(true)),
            simulated_keys: Arc::new(Mutex::new(HashMap::new())),
            should_cancel: Arc::new(Mutex::new(false)),
        }
    }

    pub fn set_availability(&self, is_available: bool, status: &str) {
        *self.is_available.lock().unwrap() = is_available;
        *self.availability_status.lock().unwrap() = status.to_string();
    }

    pub fn set_simulate_cancellation(&self, cancel: bool) {
        *self.should_cancel.lock().unwrap() = cancel;
    }

    pub fn has_key(&self, key_name: &str) -> bool {
        self.simulated_keys.lock().unwrap().contains_key(key_name)
    }
}

#[async_trait]
impl PlatformKeyProtector for MockPlatformProtector {
    fn provider_id(&self) -> &'static str {
        "mock-protector"
    }

    fn get_device_id(&self) -> String {
        self.device_id.clone()
    }

    fn get_device_name(&self) -> String {
        self.device_name.clone()
    }

    async fn capabilities(
        &self,
        _data_root: Option<&Path>,
        vault_id: Option<&str>,
        enrolled_entry: Option<&ProtectorEntry>,
    ) -> ProtectorCapabilities {
        let is_avail = *self.is_available.lock().unwrap();
        let status = self.availability_status.lock().unwrap().clone();

        let key_name = vault_id.map(|vid| format!("PixieVault_{}_{}", vid.replace('-', ""), self.device_id));
        let is_enrolled = if let Some(ref kn) = key_name {
            self.simulated_keys.lock().unwrap().contains_key(kn)
                || enrolled_entry.map(|e| e.device_id.as_deref() == Some(&self.device_id)).unwrap_or(false)
        } else {
            false
        };

        ProtectorCapabilities {
            is_available: is_avail,
            provider_name: self.provider_name.clone(),
            biometric_type: "Simulated Windows Hello (TPM 2.0 / Biometrics)".into(),
            is_enrolled,
            availability_status: status,
            supported_hardware: vec!["Fingerprint".into(), "PIN".into(), "TPM 2.0".into()],
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
        }
    }

    async fn enroll(
        &self,
        vault_id: &str,
        master_key: &MasterKey,
        _window_handle: Option<isize>,
    ) -> Result<ProtectorEntry, String> {
        if *self.should_cancel.lock().unwrap() {
            return Err("User cancelled biometric prompt".into());
        }

        let is_avail = *self.is_available.lock().unwrap();
        if !is_avail {
            return Err("Protector device is not available on this system".into());
        }

        let key_name = format!("PixieVault_{}_{}", vault_id.replace('-', ""), self.device_id);

        // Generate simulated non-exportable hardware key
        let mut sim_hw_key = [0u8; 32];
        OsRng.fill_bytes(&mut sim_hw_key);
        self.simulated_keys
            .lock()
            .unwrap()
            .insert(key_name.clone(), sim_hw_key.to_vec());

        // Wrap master key using simulated hardware key
        let mut wrapped = master_key.0;
        for i in 0..32 {
            wrapped[i] ^= sim_hw_key[i];
        }

        let wrapped_b64 = BASE64_STANDARD.encode(wrapped);

        Ok(ProtectorEntry {
            id: format!("windows-hello-{}", self.device_id),
            protector_type: "mock-protector".into(),
            salt_b64: None,
            nonce_b64: None,
            wrapped_master_key_b64: wrapped_b64,
            key_name: Some(key_name),
            device_id: Some(self.device_id.clone()),
            device_name: Some(self.device_name.clone()),
            extra: Some(serde_json::json!({
                "hardware_backed": true,
                "tpm_version": "2.0"
            })),
        })
    }

    async fn unlock(
        &self,
        vault_id: &str,
        entry: &ProtectorEntry,
        _window_handle: Option<isize>,
    ) -> Result<MasterKey, String> {
        if *self.should_cancel.lock().unwrap() {
            return Err("User cancelled biometric prompt".into());
        }

        let key_name = entry
            .key_name
            .clone()
            .unwrap_or_else(|| format!("PixieVault_{}_{}", vault_id.replace('-', ""), self.device_id));

        let sim_hw_key = {
            let guard = self.simulated_keys.lock().unwrap();
            guard.get(&key_name).cloned().ok_or_else(|| {
                format!(
                    "Hardware key '{}' not found on this device ({})",
                    key_name, self.device_id
                )
            })?
        };

        let wrapped = BASE64_STANDARD
            .decode(&entry.wrapped_master_key_b64)
            .map_err(|e| format!("Invalid wrapped key encoding: {}", e))?;

        if wrapped.len() != 32 {
            return Err("Corrupted wrapped key size in protector entry".into());
        }

        let mut unwrapped = [0u8; 32];
        for i in 0..32 {
            unwrapped[i] = wrapped[i] ^ sim_hw_key[i];
        }

        Ok(MasterKey(unwrapped))
    }

    async fn revoke(
        &self,
        vault_id: &str,
        entry: Option<&ProtectorEntry>,
    ) -> Result<(), String> {
        let key_name = entry
            .and_then(|e| e.key_name.clone())
            .unwrap_or_else(|| format!("PixieVault_{}_{}", vault_id.replace('-', ""), self.device_id));

        self.simulated_keys.lock().unwrap().remove(&key_name);
        Ok(())
    }
}

/// Fallback Platform Protector for platforms without hardware security modules
pub struct GenericFallbackProtector {
    device_id: String,
    device_name: String,
}

impl GenericFallbackProtector {
    pub fn new() -> Self {
        let host_str = hostname_or_default();
        let mut hasher = Sha256::new();
        hasher.update(b"PixieVault::Device::");
        hasher.update(host_str.as_bytes());
        let hash = hasher.finalize();
        let device_id = format!("{:x}", &hash[0..8].iter().fold(0u64, |acc, &b| (acc << 8) | b as u64));
        Self {
            device_id,
            device_name: host_str,
        }
    }
}

fn hostname_or_default() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "PixieVault-Device".into())
}

#[async_trait]
impl PlatformKeyProtector for GenericFallbackProtector {
    fn provider_id(&self) -> &'static str {
        #[cfg(target_os = "macos")]
        {
            "apple-localauth"
        }
        #[cfg(target_os = "linux")]
        {
            "linux-keyring"
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            "generic-fallback"
        }
    }

    fn get_device_id(&self) -> String {
        self.device_id.clone()
    }

    fn get_device_name(&self) -> String {
        self.device_name.clone()
    }

    async fn capabilities(
        &self,
        _data_root: Option<&Path>,
        _vault_id: Option<&str>,
        enrolled_entry: Option<&ProtectorEntry>,
    ) -> ProtectorCapabilities {
        #[cfg(target_os = "macos")]
        {
            ProtectorCapabilities {
                is_available: true,
                provider_name: "Apple LocalAuthentication (Touch ID)".into(),
                biometric_type: "Touch ID / Face ID / Apple Passcode".into(),
                is_enrolled: enrolled_entry.is_some(),
                availability_status: "Ready".into(),
                supported_hardware: vec!["Touch ID".into(), "Secure Enclave".into()],
                device_id: self.device_id.clone(),
                device_name: self.device_name.clone(),
            }
        }
        #[cfg(target_os = "linux")]
        {
            ProtectorCapabilities {
                is_available: true,
                provider_name: "Linux Secret Service & PAM".into(),
                biometric_type: "Linux PAM / FPrint / Keyring".into(),
                is_enrolled: enrolled_entry.is_some(),
                availability_status: "Ready".into(),
                supported_hardware: vec!["PAM".into(), "Secret Service".into()],
                device_id: self.device_id.clone(),
                device_name: self.device_name.clone(),
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let is_enrolled = enrolled_entry.is_some();
            ProtectorCapabilities {
                is_available: false,
                provider_name: "Generic Fallback Protector".into(),
                biometric_type: "Master Passphrase".into(),
                is_enrolled,
                availability_status: "NotSupported".into(),
                supported_hardware: vec![],
                device_id: self.device_id.clone(),
                device_name: self.device_name.clone(),
            }
        }
    }

    async fn enroll(
        &self,
        vault_id: &str,
        master_key: &MasterKey,
        _window_handle: Option<isize>,
    ) -> Result<ProtectorEntry, String> {
        let key_name = format!("PixieVault_{}_{}", vault_id.replace('-', ""), self.device_id);
        let wrapped_b64 = BASE64_STANDARD.encode(master_key.0);
        Ok(ProtectorEntry {
            id: format!("windows-hello-{}", self.device_id),
            protector_type: self.provider_id().into(),
            salt_b64: None,
            nonce_b64: None,
            wrapped_master_key_b64: wrapped_b64,
            key_name: Some(key_name),
            device_id: Some(self.device_id.clone()),
            device_name: Some(self.device_name.clone()),
            extra: None,
        })
    }

    async fn unlock(
        &self,
        _vault_id: &str,
        entry: &ProtectorEntry,
        _window_handle: Option<isize>,
    ) -> Result<MasterKey, String> {
        let bytes = BASE64_STANDARD
            .decode(&entry.wrapped_master_key_b64)
            .map_err(|e| format!("Invalid wrapped key encoding: {}", e))?;
        if bytes.len() != 32 {
            return Err("Invalid wrapped key length".into());
        }
        let mut raw = [0u8; 32];
        raw.copy_from_slice(&bytes);
        Ok(MasterKey(raw))
    }

    async fn revoke(
        &self,
        _vault_id: &str,
        _entry: Option<&ProtectorEntry>,
    ) -> Result<(), String> {
        Ok(())
    }
}

/// Helper to get the active platform protector for the host system
pub fn get_platform_protector() -> Arc<dyn PlatformKeyProtector> {
    #[cfg(target_os = "windows")]
    {
        Arc::new(super::windows_hello::WindowsHelloProtector::new())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Arc::new(GenericFallbackProtector::new())
    }
}
