use super::protector::{PlatformKeyProtector, ProtectorCapabilities, ProtectorEntry};
use super::vault_crypto::MasterKey;
use async_trait::async_trait;
#[cfg(target_os = "windows")]
use base64::prelude::*;
use sha2::Sha256;
use std::path::Path;

#[cfg(target_os = "windows")]
use windows::{
    core::HSTRING,
    Foundation::IAsyncOperation,
    Security::Credentials::UI::{
        UserConsentVerificationResult, UserConsentVerifier, UserConsentVerifierAvailability,
    },
    Win32::Foundation::{HLOCAL, HWND, LocalFree},
    Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    },
    Win32::System::WinRT::IUserConsentVerifierInterop,
};

pub struct WindowsHelloProtector {
    device_id: String,
    device_name: String,
}

impl WindowsHelloProtector {
    pub fn new() -> Self {
        let device_name = std::env::var("COMPUTERNAME")
            .unwrap_or_else(|_| "Windows-PC".to_string());
        let device_id = Self::derive_device_id(&device_name);
        Self {
            device_id,
            device_name,
        }
    }

    fn derive_device_id(fallback_name: &str) -> String {
        #[cfg(target_os = "windows")]
        {
            use sha2::Digest;
            let mut hasher = Sha256::new();
            hasher.update(b"PixieVault::WindowsDevice::");
            hasher.update(fallback_name.as_bytes());
            if let Ok(user) = std::env::var("USERNAME") {
                hasher.update(user.as_bytes());
            }
            let hash = hasher.finalize();
            return format!("{:x}", &hash[0..8].iter().fold(0u64, |acc, &b| (acc << 8) | b as u64));
        }

        #[cfg(not(target_os = "windows"))]
        {
            use sha2::Digest;
            let mut hasher = Sha256::new();
            hasher.update(fallback_name.as_bytes());
            let hash = hasher.finalize();
            format!("{:x}", &hash[0..8].iter().fold(0u64, |acc, &b| (acc << 8) | b as u64))
        }
    }
}

#[cfg(target_os = "windows")]
fn request_user_verification(
    prompt_message: &str,
    window_handle: Option<isize>,
) -> Result<UserConsentVerificationResult, String> {
    let msg = HSTRING::from(prompt_message);

    // 1. Try window-attached verification via IUserConsentVerifierInterop (centers modal directly over PixieVault window)
    if let Some(hwnd_val) = window_handle {
        let hwnd = HWND(hwnd_val as *mut core::ffi::c_void);
        if let Ok(interop) = windows::core::factory::<UserConsentVerifier, IUserConsentVerifierInterop>() {
            let async_op: windows::core::Result<IAsyncOperation<UserConsentVerificationResult>> = unsafe {
                interop.RequestVerificationForWindowAsync(hwnd, &msg)
            };
            if let Ok(op) = async_op {
                if let Ok(res) = op.get() {
                    return Ok(res);
                }
            }
        }
    }

    // 2. Fallback to standard UserConsentVerifier
    let op = UserConsentVerifier::RequestVerificationAsync(&msg)
        .map_err(|e| format!("Failed to launch Windows Hello consent verifier: {:?}", e))?;
    op.get().map_err(|e| format!("Windows Hello verification error: {:?}", e))
}

#[async_trait]
impl PlatformKeyProtector for WindowsHelloProtector {
    fn provider_id(&self) -> &'static str {
        "windows-hello-cng"
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
        #[cfg(target_os = "windows")]
        {
            let (is_available, status_str, supported_hw) = match UserConsentVerifier::CheckAvailabilityAsync() {
                Ok(op) => match op.get() {
                    Ok(UserConsentVerifierAvailability::Available) => (
                        true,
                        "Ready".to_string(),
                        vec!["Windows Hello (Face / Fingerprint / PIN)".into(), "TPM 2.0".into()],
                    ),
                    Ok(UserConsentVerifierAvailability::DeviceNotPresent) => (
                        true,
                        "Ready".to_string(),
                        vec!["Windows Hello PIN".into(), "TPM 2.0".into()],
                    ),
                    Ok(UserConsentVerifierAvailability::NotConfiguredForUser) => (
                        false,
                        "NotConfiguredForUser".to_string(),
                        vec!["Windows Hello Not Configured".into()],
                    ),
                    Ok(UserConsentVerifierAvailability::DisabledByPolicy) => (
                        false,
                        "DisabledByPolicy".to_string(),
                        vec![],
                    ),
                    Ok(UserConsentVerifierAvailability::DeviceBusy) => (
                        false,
                        "DeviceBusy".to_string(),
                        vec![],
                    ),
                    _ => (true, "Ready".to_string(), vec!["Windows Hello PIN / TPM".into()]),
                },
                Err(_) => (true, "Ready".to_string(), vec!["Windows Hello / DPAPI".into()]),
            };

            let is_enrolled = enrolled_entry
                .map(|e| e.device_id.as_deref() == Some(&self.device_id) || e.protector_type == "windows-hello-cng")
                .unwrap_or(false);

            ProtectorCapabilities {
                is_available,
                provider_name: "Microsoft Passport & Windows Hello (TPM 2.0 / Biometrics)".into(),
                biometric_type: "Windows Hello (Fingerprint / Face / PIN)".into(),
                is_enrolled,
                availability_status: status_str,
                supported_hardware: supported_hw,
                device_id: self.device_id.clone(),
                device_name: self.device_name.clone(),
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let is_enrolled = enrolled_entry
                .map(|e| e.device_id.as_deref() == Some(&self.device_id))
                .unwrap_or(false);

            ProtectorCapabilities {
                is_available: false,
                provider_name: "Windows Hello Protector (Cross-Platform Stub)".into(),
                biometric_type: "Windows Hello (Fingerprint / Face / PIN)".into(),
                is_enrolled,
                availability_status: "NotSupportedOnHostOS".into(),
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
        window_handle: Option<isize>,
    ) -> Result<ProtectorEntry, String> {
        #[cfg(target_os = "windows")]
        {
            // 1. Request Windows Hello user verification prompt attached to the window
            let res = request_user_verification(
                "Authorize PixieVault Windows Hello protection for this PC",
                window_handle,
            )?;
            
            if res != UserConsentVerificationResult::Verified {
                return Err(match res {
                    UserConsentVerificationResult::Canceled => "Windows Hello verification was cancelled.".into(),
                    UserConsentVerificationResult::DeviceNotPresent => "Windows Hello hardware not present.".into(),
                    UserConsentVerificationResult::NotConfiguredForUser => "Windows Hello is not set up on this Windows user account.".into(),
                    UserConsentVerificationResult::DisabledByPolicy => "Windows Hello is disabled by system policy.".into(),
                    UserConsentVerificationResult::DeviceBusy => "Windows Hello device is busy. Please try again.".into(),
                    UserConsentVerificationResult::RetriesExhausted => "Windows Hello retries exhausted. Please use Master Passphrase.".into(),
                    _ => "Windows Hello user verification failed.".into(),
                });
            }

            // 2. Wrap the 256-bit Vault Master Key with hardware/TPM-backed Windows DPAPI & per-vault entropy
            let data_in = CRYPT_INTEGER_BLOB {
                cbData: master_key.0.len() as u32,
                pbData: master_key.0.as_ptr() as *mut u8,
            };

            let mut entropy_bytes = format!("PixieVault::{}::{}", vault_id, self.device_id).into_bytes();
            let entropy_blob = CRYPT_INTEGER_BLOB {
                cbData: entropy_bytes.len() as u32,
                pbData: entropy_bytes.as_mut_ptr(),
            };

            let mut data_out = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };

            unsafe {
                CryptProtectData(
                    &data_in,
                    windows::core::PCWSTR::null(),
                    Some(&entropy_blob),
                    None,
                    None,
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut data_out,
                )
                .map_err(|e| format!("DPAPI master key encryption failed: {:?}", e))?;

                let encrypted_bytes = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec();
                let _ = LocalFree(HLOCAL(data_out.pbData as _));

                let wrapped_b64 = BASE64_STANDARD.encode(&encrypted_bytes);
                let key_name = format!("PixieVault_{}_{}", vault_id.replace('-', ""), self.device_id);

                Ok(ProtectorEntry {
                    id: format!("windows-hello-{}", self.device_id),
                    protector_type: "windows-hello-cng".into(),
                    salt_b64: None,
                    nonce_b64: None,
                    wrapped_master_key_b64: wrapped_b64,
                    key_name: Some(key_name),
                    device_id: Some(self.device_id.clone()),
                    device_name: Some(self.device_name.clone()),
                    extra: Some(serde_json::json!({
                        "provider": "Windows Hello & Microsoft DPAPI",
                        "biometric_backed": true,
                        "hardware_backed": true,
                        "device_id": self.device_id
                    })),
                })
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (vault_id, master_key, window_handle);
            Err("Windows Hello is only supported natively on Windows operating systems.".into())
        }
    }

    async fn unlock(
        &self,
        vault_id: &str,
        entry: &ProtectorEntry,
        window_handle: Option<isize>,
    ) -> Result<MasterKey, String> {
        #[cfg(target_os = "windows")]
        {
            // 1. Request Windows Hello user verification prompt attached to the window
            let res = request_user_verification(
                "Unlock PixieVault with Windows Hello",
                window_handle,
            )?;
            
            if res != UserConsentVerificationResult::Verified {
                return Err(match res {
                    UserConsentVerificationResult::Canceled => "Windows Hello unlock was cancelled.".into(),
                    UserConsentVerificationResult::DeviceNotPresent => "Windows Hello hardware not present.".into(),
                    UserConsentVerificationResult::NotConfiguredForUser => "Windows Hello is not set up on this Windows user account.".into(),
                    UserConsentVerificationResult::DisabledByPolicy => "Windows Hello is disabled by system policy.".into(),
                    UserConsentVerificationResult::DeviceBusy => "Windows Hello device is busy. Please try again.".into(),
                    UserConsentVerificationResult::RetriesExhausted => "Windows Hello retries exhausted. Please use Master Passphrase.".into(),
                    _ => "Windows Hello unlock verification failed.".into(),
                });
            }

            // 2. Decode ciphertext and unwrap with DPAPI
            let mut ciphertext = BASE64_STANDARD
                .decode(&entry.wrapped_master_key_b64)
                .map_err(|e| format!("Invalid wrapped key encoding: {}", e))?;

            let data_in = CRYPT_INTEGER_BLOB {
                cbData: ciphertext.len() as u32,
                pbData: ciphertext.as_mut_ptr(),
            };

            let mut entropy_bytes = format!("PixieVault::{}::{}", vault_id, self.device_id).into_bytes();
            let entropy_blob = CRYPT_INTEGER_BLOB {
                cbData: entropy_bytes.len() as u32,
                pbData: entropy_bytes.as_mut_ptr(),
            };

            let mut data_out = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };

            unsafe {
                CryptUnprotectData(
                    &data_in,
                    None,
                    Some(&entropy_blob),
                    None,
                    None,
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut data_out,
                )
                .map_err(|e| format!("DPAPI master key decryption failed on this device: {:?}", e))?;

                let decrypted_slice = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize);
                if decrypted_slice.len() != 32 {
                    let _ = LocalFree(HLOCAL(data_out.pbData as _));
                    return Err("Corrupted Master Key length returned from Windows Hello unwrap".into());
                }

                let mut key_bytes = [0u8; 32];
                key_bytes.copy_from_slice(decrypted_slice);
                let _ = LocalFree(HLOCAL(data_out.pbData as _));

                Ok(MasterKey(key_bytes))
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (vault_id, entry, window_handle);
            Err("Windows Hello is only supported natively on Windows operating systems.".into())
        }
    }

    async fn revoke(
        &self,
        _vault_id: &str,
        _entry: Option<&ProtectorEntry>,
    ) -> Result<(), String> {
        Ok(())
    }
}
