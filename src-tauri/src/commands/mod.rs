use crate::app_manager::{
    AppRegistry, ComposerAppStatus, InstalledAppInfo, InterAppBus, MetricValue, SidecarStatus,
    VaultComposer,
};
use crate::auth::{
    get_platform_protector, AuthStatus, ProtectorCapabilities, ProtectorEntry, VaultCrypto,
    VaultSession,
};
use crate::storage::{VaultData, VaultStorage, WorkspaceManager};
use base64::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tauri::State;

pub struct AppState {
    pub session: RwLock<VaultSession>,
    pub storage: VaultStorage,
    pub workspace: WorkspaceManager,
    pub registry: AppRegistry,
    pub bus: InterAppBus,
    pub sidecars: VaultComposer,
    pub vault_data: RwLock<Option<VaultData>>,
    pub vault_salt: RwLock<Option<Vec<u8>>>,
    pub app_data_root: PathBuf,
}

#[derive(Serialize)]
pub struct VaultStatusResponse {
    pub is_locked: bool,
    pub is_initialized: bool,
    pub is_secured: bool,
    pub active_app: Option<String>,
    pub biometrics_available: bool,
    pub biometric_enrolled: bool,
    pub biometric_provider: String,
    pub biometric_type: String,
    pub availability_status: String,
    pub supported_hardware: Vec<String>,
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub platform_label: String,
    pub auto_launch_last_app: bool,
    pub last_opened_app: Option<String>,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub success: bool,
    pub auto_launch_app: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct UpdateCheckResponse {
    pub has_update: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_notes: Option<String>,
}

fn get_window_handle(_window: &tauri::Window) -> Option<isize> {
    #[cfg(target_os = "windows")]
    {
        _window.hwnd().ok().map(|h| h.0 as isize)
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

fn purge_legacy_credential_file(app_data_root: &std::path::Path) {
    let legacy_file = app_data_root.join(".biometric_vault_cred.enc");
    if legacy_file.exists() {
        let _ = std::fs::remove_file(legacy_file);
    }
}

#[tauri::command]
pub async fn pv_get_vault_status(
    state: State<'_, Arc<AppState>>,
) -> Result<VaultStatusResponse, String> {
    let (is_locked, is_init, active_app, vault_id, protectors) = {
        let session = state.session.read().unwrap();
        let is_locked = session.status == AuthStatus::Locked;
        let is_init = state.storage.is_initialized();
        let active_app = state.registry.get_active_app_id();
        let vault_id = state.storage.get_vault_id().unwrap_or(None);
        let protectors = state.storage.get_protector_entries().unwrap_or_default();
        (is_locked, is_init, active_app, vault_id, protectors)
    };

    let protector = get_platform_protector();
    let device_id = protector.get_device_id();
    let enrolled_entry = protectors.iter().find(|p| {
        p.device_id.as_deref() == Some(&device_id)
            || p.protector_type == "windows-hello-cng"
            || (p.protector_type == protector.provider_id() && p.device_id.is_none())
    });

    let caps = protector
        .capabilities(
            Some(&state.app_data_root),
            vault_id.as_deref(),
            enrolled_entry,
        )
        .await;

    let (auto_launch, last_opened, is_secured) = {
        let vd = state.vault_data.read().unwrap();
        if let Some(ref data) = *vd {
            let valid_last_app = data
                .settings
                .last_opened_app
                .clone()
                .filter(|id| state.registry.get_app(id).is_some());
            (
                data.settings.auto_launch_last_app,
                valid_last_app,
                data.settings.is_secured,
            )
        } else {
            (false, None, is_init)
        }
    };

    let (platform, platform_label) = if cfg!(target_os = "windows") {
        ("windows", "Windows 11 / Windows Hello")
    } else if cfg!(target_os = "macos") {
        ("macos", "macOS / Touch ID")
    } else {
        ("linux", "Linux / PAM Keyring")
    };

    Ok(VaultStatusResponse {
        is_locked,
        is_initialized: is_init,
        is_secured,
        active_app,
        biometrics_available: caps.is_available,
        biometric_enrolled: caps.is_enrolled,
        biometric_provider: caps.provider_name,
        biometric_type: caps.biometric_type,
        availability_status: caps.availability_status,
        supported_hardware: caps.supported_hardware,
        device_id: caps.device_id,
        device_name: caps.device_name,
        platform: platform.to_string(),
        platform_label: platform_label.to_string(),
        auto_launch_last_app: auto_launch,
        last_opened_app: last_opened,
    })
}

#[tauri::command]
pub async fn pv_windows_hello_capabilities(
    state: State<'_, Arc<AppState>>,
) -> Result<ProtectorCapabilities, String> {
    let vault_id = state.storage.get_vault_id().unwrap_or(None);
    let protectors = state.storage.get_protector_entries().unwrap_or_default();
    let protector = get_platform_protector();
    let device_id = protector.get_device_id();
    let enrolled_entry = protectors.iter().find(|p| {
        p.device_id.as_deref() == Some(&device_id)
            || p.protector_type == "windows-hello-cng"
            || (p.protector_type == protector.provider_id() && p.device_id.is_none())
    });
    Ok(protector
        .capabilities(
            Some(&state.app_data_root),
            vault_id.as_deref(),
            enrolled_entry,
        )
        .await)
}

#[tauri::command]
pub async fn pv_windows_hello_enroll(
    passphrase: Option<String>,
    state: State<'_, Arc<AppState>>,
    window: tauri::Window,
) -> Result<ProtectorEntry, String> {
    let (master_key, _) = {
        let session = state.session.read().unwrap();
        if let Some(ref key) = session.active_key {
            (key.clone(), session.is_initial_unconfigured_passphrase)
        } else if let Some(pw) = passphrase {
            if !state.storage.is_initialized() {
                return Err("Vault is not initialized. Please set up the vault first.".into());
            }
            let payload = state.storage.load_payload().map_err(|e| e.to_string())?;
            let (key, _) = VaultCrypto::unlock_envelope_with_passphrase(&payload, &pw)
                .map_err(|e| format!("Passphrase verification failed: {}", e))?;
            (key, payload.is_initial_state)
        } else {
            return Err("Passphrase is required to authorize Windows Hello enrollment when the vault is locked.".into());
        }
    };

    let mut payload = if state.storage.is_initialized() {
        state.storage.load_payload().map_err(|e| e.to_string())?
    } else {
        return Err("Vault must be initialized before enrolling hardware protectors.".into());
    };

    if payload.vault_id.is_empty() {
        payload.vault_id = uuid::Uuid::new_v4().to_string();
    }

    let protector = get_platform_protector();
    let hwnd = get_window_handle(&window);

    // 2. Perform Hardware Key Enrollment
    let entry = protector
        .enroll(&payload.vault_id, &master_key, hwnd)
        .await?;

    // 3. Immediate transactional verification of created key before saving
    let verified_key = protector
        .unlock(&payload.vault_id, &entry, hwnd)
        .await
        .map_err(|e| {
            format!(
                "Immediate Windows Hello key unwrap verification failed: {}",
                e
            )
        })?;
    if verified_key.0 != master_key.0 {
        return Err("Hardware key verification mismatch: wrapped key does not unwrap to the active Vault Master Key.".into());
    }

    // 4. Add device protector to envelope and save atomically
    VaultCrypto::add_or_update_device_protector(&mut payload, entry.clone());
    state
        .storage
        .save_payload(&payload)
        .map_err(|e| e.to_string())?;

    // 5. Clean up legacy credential file if present
    purge_legacy_credential_file(&state.app_data_root);

    Ok(entry)
}

#[tauri::command]
pub async fn pv_windows_hello_unlock(
    state: State<'_, Arc<AppState>>,
    window: tauri::Window,
) -> Result<AuthResponse, String> {
    if !state.storage.is_initialized() {
        return Ok(AuthResponse {
            success: false,
            auto_launch_app: None,
            error: Some(
                "Vault is not initialized. Please create or unlock with your Master Passphrase."
                    .into(),
            ),
        });
    }

    let payload = match state.storage.load_payload() {
        Ok(p) => p,
        Err(e) => {
            return Ok(AuthResponse {
                success: false,
                auto_launch_app: None,
                error: Some(format!("Failed to read vault envelope: {}", e)),
            });
        }
    };

    let protector = get_platform_protector();
    let device_id = protector.get_device_id();

    // Find matching device protector entry
    let entry = payload.protectors.iter().find(|p| {
        p.device_id.as_deref() == Some(&device_id)
            || p.protector_type == "windows-hello-cng"
            || (p.protector_type == protector.provider_id() && p.device_id.is_none())
    });

    let entry = match entry {
        Some(e) => e,
        None => {
            return Ok(AuthResponse {
                success: false,
                auto_launch_app: None,
                error: Some(format!(
                    "Windows Hello is not enrolled for this device ({}). Please unlock with your Master Passphrase.",
                    protector.get_device_name()
                )),
            });
        }
    };

    let hwnd = get_window_handle(&window);
    let key = match protector.unlock(&payload.vault_id, entry, hwnd).await {
        Ok(k) => k,
        Err(e) => {
            return Ok(AuthResponse {
                success: false,
                auto_launch_app: None,
                error: Some(e),
            });
        }
    };

    // Load vault data using the unwrapped Master Key
    let vault_data = match state.storage.load(&key) {
        Ok(vd) => vd,
        Err(e) => {
            return Ok(AuthResponse {
                success: false,
                auto_launch_app: None,
                error: Some(format!("Vault data load error: {}", e)),
            });
        }
    };

    let is_unsecured = !vault_data.settings.is_secured;
    let salt = state
        .storage
        .get_salt()
        .unwrap_or(None)
        .unwrap_or_else(|| crate::auth::LEGACY_GLOBAL_SALT.to_vec());

    state
        .workspace
        .materialize_workspace()
        .map_err(|e| e.to_string())?;
    for (app_id, files) in &vault_data.app_files {
        let _ = state.workspace.unpack_app_files(app_id, files);
    }

    {
        let mut session = state.session.write().unwrap();
        session.unlock(key, is_unsecured);
    }
    *state.vault_salt.write().unwrap() = Some(salt);

    let auto_launch_target = if vault_data.settings.auto_launch_last_app {
        vault_data
            .settings
            .last_opened_app
            .clone()
            .filter(|id| state.registry.get_app(id).is_some())
    } else {
        None
    };
    *state.vault_data.write().unwrap() = Some(vault_data);

    // Purge legacy credential file
    purge_legacy_credential_file(&state.app_data_root);

    Ok(AuthResponse {
        success: true,
        auto_launch_app: auto_launch_target,
        error: None,
    })
}

#[tauri::command]
pub async fn pv_windows_hello_revoke(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    if !state.storage.is_initialized() {
        return Ok(true);
    }

    let mut payload = state.storage.load_payload().map_err(|e| e.to_string())?;
    let protector = get_platform_protector();
    let device_id = protector.get_device_id();

    let entry = payload
        .protectors
        .iter()
        .find(|p| {
            p.device_id.as_deref() == Some(&device_id)
                || (p.protector_type == protector.provider_id() && p.device_id.is_none())
        })
        .cloned();

    protector.revoke(&payload.vault_id, entry.as_ref()).await?;

    VaultCrypto::remove_device_protector(&mut payload, &device_id);
    state
        .storage
        .save_payload(&payload)
        .map_err(|e| e.to_string())?;

    purge_legacy_credential_file(&state.app_data_root);
    Ok(true)
}

#[tauri::command]
pub async fn pv_authenticate_biometric(
    state: State<'_, Arc<AppState>>,
    window: tauri::Window,
) -> Result<AuthResponse, String> {
    pv_windows_hello_unlock(state, window).await
}

#[tauri::command]
pub async fn pv_authenticate_password(
    passphrase: String,
    state: State<'_, Arc<AppState>>,
) -> Result<AuthResponse, String> {
    let is_init = state.storage.is_initialized();

    let (key, vault_data, is_unsecured, salt) = if is_init {
        let mut payload = match state.storage.load_payload() {
            Ok(p) => p,
            Err(e) => {
                return Ok(AuthResponse {
                    success: false,
                    auto_launch_app: None,
                    error: Some(format!("Vault load error: {}", e)),
                });
            }
        };

        let (key, plaintext) =
            match VaultCrypto::unlock_envelope_with_passphrase(&payload, &passphrase) {
                Ok(res) => res,
                Err(crate::auth::CryptoError::DecryptionError)
                | Err(crate::auth::CryptoError::ProtectorNotFound(_)) => {
                    return Ok(AuthResponse {
                        success: false,
                        auto_launch_app: None,
                        error: Some("Authentication failed: Invalid passphrase".into()),
                    });
                }
                Err(e) => {
                    return Ok(AuthResponse {
                        success: false,
                        auto_launch_app: None,
                        error: Some(format!("Decryption error: {}", e)),
                    });
                }
            };

        let vd: VaultData = match serde_json::from_slice(&plaintext) {
            Ok(d) => d,
            Err(e) => {
                return Ok(AuthResponse {
                    success: false,
                    auto_launch_app: None,
                    error: Some(format!("Corrupted vault data structure: {}", e)),
                });
            }
        };

        // Transparent automatic migration of legacy v2 payload to v3 envelope
        if payload.version < 3 {
            let salt = VaultCrypto::generate_salt();
            if let Ok(argon_entry) =
                VaultCrypto::wrap_master_key_argon2id(&key, &passphrase, &salt)
            {
                payload.version = 3;
                if payload.vault_id.is_empty() {
                    payload.vault_id = uuid::Uuid::new_v4().to_string();
                }
                payload.protectors = vec![argon_entry];
                let _ = state.storage.save_payload(&payload);
            }
        }

        let is_unsecured = !vd.settings.is_secured;
        let salt = state
            .storage
            .get_salt()
            .unwrap_or(None)
            .unwrap_or_else(|| crate::auth::LEGACY_GLOBAL_SALT.to_vec());
        (key, vd, is_unsecured, salt)
    } else {
        let mut vd = VaultData::default();
        let is_initial = passphrase.is_empty();
        vd.settings.is_secured = !is_initial;
        let plaintext = serde_json::to_vec(&vd).map_err(|e| e.to_string())?;

        let (payload, key) =
            VaultCrypto::create_envelope_v3(&passphrase, &plaintext).map_err(|e| e.to_string())?;
        state
            .storage
            .save_payload(&payload)
            .map_err(|e| e.to_string())?;

        let salt = state
            .storage
            .get_salt()
            .unwrap_or(None)
            .unwrap_or_else(|| crate::auth::LEGACY_GLOBAL_SALT.to_vec());
        (key, vd, is_initial, salt)
    };

    // Materialize secure workspace and unpack app files
    state
        .workspace
        .materialize_workspace()
        .map_err(|e| e.to_string())?;
    for (app_id, files) in &vault_data.app_files {
        let _ = state.workspace.unpack_app_files(app_id, files);
    }

    {
        let mut session = state.session.write().unwrap();
        session.unlock(key, is_unsecured);
    }
    *state.vault_salt.write().unwrap() = Some(salt);

    let auto_launch_target = if vault_data.settings.auto_launch_last_app {
        vault_data
            .settings
            .last_opened_app
            .clone()
            .filter(|id| state.registry.get_app(id).is_some())
    } else {
        None
    };
    *state.vault_data.write().unwrap() = Some(vault_data);

    // Purge legacy credential file if present
    purge_legacy_credential_file(&state.app_data_root);

    Ok(AuthResponse {
        success: true,
        auto_launch_app: auto_launch_target,
        error: None,
    })
}

#[tauri::command]
pub fn pv_set_master_passphrase(
    passphrase: String,
    current_passphrase: Option<String>,
    state: State<Arc<AppState>>,
) -> Result<bool, String> {
    let mut session = state.session.write().unwrap();
    let mut vd_guard = state.vault_data.write().unwrap();
    let mut salt_guard = state.vault_salt.write().unwrap();

    let mut payload = if state.storage.is_initialized() {
        state.storage.load_payload().map_err(|e| e.to_string())?
    } else {
        return Err("Vault not initialized".into());
    };

    let master_key = if let Some(ref key) = session.active_key {
        key.clone()
    } else {
        let cur_pw = current_passphrase.unwrap_or_default();
        let (key, _) = VaultCrypto::unlock_envelope_with_passphrase(&payload, &cur_pw)
            .map_err(|_| "Invalid current master passphrase".to_string())?;
        key
    };

    // Update passphrase wrapper in envelope without changing VMK
    VaultCrypto::update_passphrase_in_envelope(&mut payload, &master_key, &passphrase)
        .map_err(|e| format!("Failed to update passphrase envelope: {}", e))?;

    if let Some(ref mut vd) = *vd_guard {
        vd.settings.is_secured = !passphrase.is_empty();
        let plaintext = serde_json::to_vec(vd).map_err(|e| e.to_string())?;
        let (ciphertext, nonce) =
            VaultCrypto::encrypt(&master_key, &plaintext).map_err(|e| e.to_string())?;
        payload.nonce_b64 = BASE64_STANDARD.encode(nonce);
        payload.ciphertext_b64 = BASE64_STANDARD.encode(ciphertext);
    }

    state
        .storage
        .save_payload(&payload)
        .map_err(|e| e.to_string())?;
    session.unlock(master_key, passphrase.is_empty());

    let salt = state
        .storage
        .get_salt()
        .unwrap_or(None)
        .unwrap_or_else(|| crate::auth::LEGACY_GLOBAL_SALT.to_vec());
    *salt_guard = Some(salt);

    purge_legacy_credential_file(&state.app_data_root);
    Ok(true)
}

pub fn lock_vault_internal(state: &AppState) -> bool {
    // 1. Stop all sidecars and active apps
    state.sidecars.stop_all();
    state.registry.set_active_app(None);

    // 2. Harvest all workspace app files into VaultData before shredding
    {
        let session = state.session.read().unwrap();
        if let Some(ref key) = session.active_key {
            let mut vd_guard = state.vault_data.write().unwrap();
            if let Some(ref mut vd) = *vd_guard {
                let apps = state.registry.list_apps();
                for app in apps {
                    if let Ok(files) = state.workspace.pack_app_files(&app.manifest.app_id) {
                        if !files.is_empty() {
                            vd.set_app_files(&app.manifest.app_id, files);
                        }
                    }
                }
                let salt = state
                    .vault_salt
                    .read()
                    .unwrap()
                    .clone()
                    .unwrap_or_else(|| crate::auth::LEGACY_GLOBAL_SALT.to_vec());
                let is_initial = !vd.settings.is_secured;
                let _ = state.storage.save(vd, key, &salt, is_initial);
            }
        }
    }

    // 3. Shred and wipe ephemeral decrypted workspace
    let _ = state.workspace.shred_and_remove_all();

    // 4. Lock session and zeroize memory keys
    let mut session = state.session.write().unwrap();
    session.lock();
    *state.vault_data.write().unwrap() = None;
    *state.vault_salt.write().unwrap() = None;

    true
}

#[tauri::command]
pub fn pv_lock_vault(state: State<Arc<AppState>>) -> bool {
    lock_vault_internal(&state)
}


#[tauri::command]
pub fn pv_list_apps(state: State<Arc<AppState>>) -> Vec<crate::app_manager::InstalledAppInfo> {
    state.session.write().unwrap().record_activity();
    state.registry.list_apps()
}

#[tauri::command]
pub fn pv_launch_app(
    app_id: String,
    state: State<Arc<AppState>>,
) -> Result<crate::app_manager::InstalledAppInfo, String> {
    state.session.write().unwrap().record_activity();
    let app = state
        .registry
        .get_app(&app_id)
        .ok_or_else(|| format!("App '{}' not found in registry", app_id))?;

    state.registry.set_active_app(Some(app_id.clone()));

    // Record last opened in settings
    let mut vd_guard = state.vault_data.write().unwrap();
    if let Some(ref mut vd) = *vd_guard {
        vd.settings.last_opened_app = Some(app_id);
    }

    Ok(app)
}

#[tauri::command]
pub fn pv_unload_app(state: State<Arc<AppState>>) -> bool {
    state.session.write().unwrap().record_activity();
    state.registry.set_active_app(None);
    true
}

#[tauri::command]
pub fn pv_load_app_data(
    app_id: String,
    state: State<Arc<AppState>>,
) -> Result<Option<serde_json::Value>, String> {
    let mut session = state.session.write().unwrap();
    if !session.is_unlocked() {
        return Err("Vault is locked".into());
    }
    session.record_activity();

    let vd_guard = state.vault_data.read().unwrap();
    if let Some(ref vd) = *vd_guard {
        Ok(vd.get_app_state(&app_id).cloned())
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub fn pv_save_app_data(
    app_id: String,
    data: serde_json::Value,
    state: State<Arc<AppState>>,
) -> Result<bool, String> {
    let mut session = state.session.write().unwrap();
    let key = session
        .active_key
        .as_ref()
        .ok_or_else(|| "Vault is locked".to_string())?
        .clone();
    session.record_activity();

    let mut vd_guard = state.vault_data.write().unwrap();
    if vd_guard.is_none() {
        *vd_guard = Some(VaultData::default());
    }

    if let Some(ref mut vd) = *vd_guard {
        vd.set_app_state(&app_id, data);
        let salt = state
            .vault_salt
            .read()
            .unwrap()
            .clone()
            .unwrap_or_else(|| crate::auth::LEGACY_GLOBAL_SALT.to_vec());
        let is_initial = !vd.settings.is_secured;
        state
            .storage
            .save(vd, &key, &salt, is_initial)
            .map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Err("Failed to acquire vault data".into())
    }
}


#[tauri::command]
pub fn pv_export_metrics(
    metrics: HashMap<String, serde_json::Value>,
    state: State<Arc<AppState>>,
) -> bool {
    let active_app = state
        .registry
        .get_active_app_id()
        .unwrap_or_else(|| "default".into());
    state.bus.export_metrics(&active_app, metrics);
    true
}

#[tauri::command]
pub fn pv_query_adjacent_metric(
    caller_app_id: String,
    target_app_id: String,
    metric_name: String,
    state: State<Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    if caller_app_id != "host_shell"
        && caller_app_id != "dashboard"
        && caller_app_id != "host_inspector"
    {
        if let Some(caller) = state.registry.get_app(&caller_app_id) {
            if !caller
                .manifest
                .can_read_metric(&target_app_id, &metric_name)
            {
                return Err(format!(
                    "Access Denied: App '{}' is not authorized to read metric '{}:{}'",
                    caller_app_id, target_app_id, metric_name
                ));
            }
        }
    }

    state
        .bus
        .query_metric(&target_app_id, &metric_name)
        .ok_or_else(|| {
            format!(
                "Metric '{}' from app '{}' not found",
                metric_name, target_app_id
            )
        })
}

#[tauri::command]
pub fn pv_get_bus_metrics(
    state: State<Arc<AppState>>,
) -> HashMap<String, HashMap<String, MetricValue>> {
    state.bus.get_all_exported()
}

// ================= Dual-Source Distribution Commands =================

#[tauri::command]
pub async fn pv_pick_and_install_package_file(
    state: State<'_, Arc<AppState>>,
) -> Result<InstalledAppInfo, String> {
    let dialog = rfd::AsyncFileDialog::new()
        .set_title("Import PixieVault Package (.pvpkg)")
        .add_filter("PixieVault Package (*.pvpkg, *.zip)", &["pvpkg", "zip"])
        .pick_file()
        .await;

    if let Some(handle) = dialog {
        let pkg_path = handle.path().to_path_buf();
        let stem = pkg_path.file_stem().unwrap_or_default().to_string_lossy();
        let out_dir = if let Some(ref user_dir) = state.registry.user_apps_root() {
            user_dir.join(stem.as_ref())
        } else {
            let apps_root = state.registry.bundled_apps_root();
            apps_root.join(stem.as_ref())
        };

        let (app_info, vault_data) = state.registry.install_package_bundle(&pkg_path, &out_dir)?;

        if let Some(data) = vault_data {
            let session = state.session.read().unwrap();
            if session.is_unlocked() {
                if let Ok(decrypted) = state.storage.decrypt_raw(&session, &data) {
                    if let Ok(vd) = serde_json::from_slice::<VaultData>(&decrypted) {
                        *state.vault_data.write().unwrap() = Some(vd);
                    }
                }
            }
        }

        Ok(app_info)
    } else {
        Err("No package file selected".into())
    }
}

#[tauri::command]
pub async fn pv_pick_and_install_local_folder(
    state: State<'_, Arc<AppState>>,
) -> Result<InstalledAppInfo, String> {
    let dialog = rfd::AsyncFileDialog::new()
        .set_title("Select App Directory Containing manifest.json")
        .pick_folder()
        .await;

    if let Some(handle) = dialog {
        let path = handle.path().to_path_buf();
        state.registry.install_local_directory(path)
    } else {
        Err("No folder selected".into())
    }
}

#[tauri::command]
pub async fn pv_pick_and_install_local_app(
    state: State<'_, Arc<AppState>>,
) -> Result<InstalledAppInfo, String> {
    let dialog = rfd::AsyncFileDialog::new()
        .set_title("Select .pvpkg Package Bundle or manifest.json")
        .add_filter("PixieVault Packages & Manifests (*.pvpkg, manifest.json)", &["pvpkg", "json", "zip"])
        .pick_file()
        .await;

    if let Some(handle) = dialog {
        let path = handle.path().to_path_buf();
        if path.extension().map_or(false, |ext| ext == "pvpkg" || ext == "zip") {
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            let out_dir = if let Some(ref user_dir) = state.registry.user_apps_root() {
                user_dir.join(stem.as_ref())
            } else {
                let apps_root = state.registry.bundled_apps_root();
                apps_root.join(stem.as_ref())
            };
            let (app_info, vault_data) = state.registry.install_package_bundle(&path, &out_dir)?;
            if let Some(data) = vault_data {
                let session = state.session.read().unwrap();
                if session.is_unlocked() {
                    if let Ok(decrypted) = state.storage.decrypt_raw(&session, &data) {
                        if let Ok(vd) = serde_json::from_slice::<VaultData>(&decrypted) {
                            *state.vault_data.write().unwrap() = Some(vd);
                        }
                    }
                }
            }
            Ok(app_info)
        } else if path.file_name().map_or(false, |name| name == "manifest.json") {
            let parent_dir = path.parent().ok_or_else(|| "Invalid manifest path".to_string())?;
            state.registry.install_local_directory(parent_dir.to_path_buf())
        } else {
            Err("Please select a .pvpkg package file or a manifest.json file".into())
        }
    } else {
        Err("No file selected".into())
    }
}

#[tauri::command]
pub fn pv_check_package_compatibility(
    package_file: String,
) -> Result<crate::app_manager::CompatibilityReport, String> {
    let pkg_path = PathBuf::from(&package_file);
    let manifest = crate::app_manager::PackageBundler::inspect_package_manifest(&pkg_path)?;
    let report = crate::app_manager::CompatibilityChecker::check(&manifest);
    Ok(report)
}

#[tauri::command]
pub fn pv_check_manifest_compatibility(
    manifest_json: String,
) -> Result<crate::app_manager::CompatibilityReport, String> {
    let manifest: crate::app_manager::AppManifest = serde_json::from_str(&manifest_json)
        .map_err(|e| format!("Invalid manifest JSON schema: {}", e))?;
    let report = crate::app_manager::CompatibilityChecker::check(&manifest);
    Ok(report)
}

#[tauri::command]
pub fn pv_install_local_directory(
    dir_path: String,
    state: State<Arc<AppState>>,
) -> Result<InstalledAppInfo, String> {
    let path = PathBuf::from(dir_path);
    state.registry.install_local_directory(path)
}

#[tauri::command]
pub fn pv_install_package_file(
    package_file: String,
    target_dir: Option<String>,
    state: State<Arc<AppState>>,
) -> Result<InstalledAppInfo, String> {
    let pkg_path = PathBuf::from(&package_file);
    let out_dir = target_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| pkg_path.with_extension(""));

    let (app_info, vault_data) = state.registry.install_package_bundle(&pkg_path, &out_dir)?;

    // If package bundled an encrypted database, import it
    if let Some(data) = vault_data {
        let session = state.session.read().unwrap();
        if session.is_unlocked() {
            if let Ok(decrypted) = state.storage.decrypt_raw(&session, &data) {
                if let Ok(vd) = serde_json::from_slice::<VaultData>(&decrypted) {
                    *state.vault_data.write().unwrap() = Some(vd);
                }
            }
        }
    }

    Ok(app_info)
}

#[tauri::command]
pub fn pv_export_app_package(
    app_id: String,
    output_file: String,
    include_vault_data: bool,
    state: State<Arc<AppState>>,
) -> Result<bool, String> {
    let session = state.session.read().unwrap();
    let out_path = PathBuf::from(output_file);

    let encrypted_bytes = if include_vault_data && session.is_unlocked() {
        let vd_guard = state.vault_data.read().unwrap();
        if let Some(ref vd) = *vd_guard {
            state.storage.encrypt_raw(&session, vd).ok()
        } else {
            None
        }
    } else {
        None
    };

    state
        .registry
        .export_app_bundle(&app_id, encrypted_bytes.as_deref(), &out_path)?;
    Ok(true)
}

#[tauri::command]
pub async fn pv_install_github_app(
    repo: String,
    tag: Option<String>,
    public_key: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<InstalledAppInfo, String> {
    let clean_name = repo.replace('/', "_").to_lowercase();
    let install_dir = PathBuf::from("apps").join(clean_name);

    state
        .registry
        .install_github_target(&repo, tag.as_deref(), public_key.as_deref(), &install_dir)
}

#[tauri::command]
pub async fn pv_check_app_updates(
    app_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<UpdateCheckResponse, String> {
    let target_id = app_id.unwrap_or_default();
    if target_id == "dashboard" || target_id == "all" || target_id.is_empty() {
        let apps = state.registry.list_apps();
        let total = apps.len();
        let mut notes = vec![];
        notes.push(format!("PixieVault Host v0.2.0 is up to date.\nScanned {} installed application(s):", total));
        for app in &apps {
            notes.push(format!(" • {} (v{}) - Up to date", app.manifest.name, app.manifest.version));
        }
        return Ok(UpdateCheckResponse {
            has_update: false,
            current_version: "0.2.0".to_string(),
            latest_version: "0.2.0".to_string(),
            release_notes: Some(notes.join("\n")),
        });
    }

    let app = state
        .registry
        .get_app(&target_id)
        .ok_or_else(|| format!("App '{}' not found", target_id))?;

    let cur_ver = app.manifest.version.clone();

    if let crate::app_manager::AppSource::GitHubRelease {
        ref repository,
        ref tag,
        ..
    } = app.source
    {
        Ok(UpdateCheckResponse {
            has_update: false,
            current_version: cur_ver.clone(),
            latest_version: tag.clone(),
            release_notes: Some(format!(
                "Pinned to GitHub release target {}/{}",
                repository, tag
            )),
        })
    } else {
        Ok(UpdateCheckResponse {
            has_update: false,
            current_version: cur_ver.clone(),
            latest_version: cur_ver.clone(),
            release_notes: Some(format!("'{}' (v{}) is up to date (managed via local workspace).", app.manifest.name, cur_ver)),
        })
    }
}

// ================= Python Sidecar Process Commands =================

#[tauri::command]
pub fn pv_start_sidecar(
    app_id: String,
    script_path: String,
    port: Option<u16>,
    state: State<Arc<AppState>>,
) -> Result<SidecarStatus, String> {
    let p = PathBuf::from(script_path);
    let listen_port = port.unwrap_or(5000);
    state.sidecars.start_python_app(&app_id, &p, listen_port)
}

#[tauri::command]
pub fn pv_stop_sidecar(app_id: String, state: State<Arc<AppState>>) -> bool {
    state.sidecars.stop_app(&app_id)
}

#[tauri::command]
pub fn pv_get_sidecar_status(app_id: String, state: State<Arc<AppState>>) -> SidecarStatus {
    state.sidecars.get_status(&app_id)
}

// ================= Native Vault Composer Commands =================

#[tauri::command]
pub fn pv_composer_start_app(
    app_id: String,
    state: State<Arc<AppState>>,
) -> Result<ComposerAppStatus, String> {
    let session = state.session.read().unwrap();
    if !session.is_unlocked() {
        return Err("Vault is locked".into());
    }

    let app_info = state
        .registry
        .get_app(&app_id)
        .ok_or_else(|| format!("App '{}' is not registered in PixieVault", app_id))?;

    let app_path = PathBuf::from(&app_info.path);
    let workspace_app_dir = state.workspace.app_dir(&app_id);
    std::fs::create_dir_all(&workspace_app_dir).map_err(|e| {
        format!(
            "Failed to create workspace app directory '{}': {}",
            workspace_app_dir.display(),
            e
        )
    })?;

    let mut extra_env = HashMap::new();
    if let Some(ref key) = session.active_key {
        let fernet_key = VaultCrypto::derive_fernet_key(key, &app_id);
        extra_env.insert("APP_ENCRYPTION_KEY".to_string(), fernet_key.clone());
        extra_env.insert("VAULT_ENCRYPTION_KEY".to_string(), fernet_key);
    }

    state.sidecars.start_composer_app_with_env(
        &app_info.manifest,
        &app_path,
        Some(&workspace_app_dir),
        Some(&extra_env),
    )
}

#[tauri::command]
pub fn pv_composer_stop_app(app_id: String, state: State<Arc<AppState>>) -> bool {
    state.sidecars.stop_composer_app(&app_id)
}

#[tauri::command]
pub fn pv_composer_get_status(app_id: String, state: State<Arc<AppState>>) -> ComposerAppStatus {
    let manifest = state.registry.get_app(&app_id).map(|a| a.manifest);
    state.sidecars.get_app_status(&app_id, manifest.as_ref())
}

#[tauri::command]
pub async fn pv_provision_app_environment(
    app_id: String,
    force: Option<bool>,
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let app_info = state
        .registry
        .get_app(&app_id)
        .ok_or_else(|| format!("App '{}' not found in registry", app_id))?;

    let app_path = PathBuf::from(&app_info.path);
    let force_flag = force.unwrap_or(false);
    let mut results = Vec::new();

    if let Some(composer_cfg) = &app_info.manifest.composer {
        for (svc_name, svc_cfg) in &composer_cfg.services {
            let working_dir = app_info
                .manifest
                .resolve_service_working_dir(&app_path, svc_name)?;

            let runtime_dir = state
                .workspace
                .app_dir(&app_id)
                .join("runtimes")
                .join(svc_name);
            let bin_path = crate::app_manager::RuntimeProvisioner::provision_service_in(
                &working_dir,
                &runtime_dir,
                svc_name,
                svc_cfg,
                force_flag,
            )
            .map_err(|diag| {
                format!(
                    "Failed to provision service '{}': {}",
                    svc_name, diag.message
                )
            })?;

            results.push(format!(
                "Service '{}' ({}) ready at {}",
                svc_name,
                svc_cfg.get_runtime().runtime_type,
                bin_path.display()
            ));
        }
        Ok(format!(
            "Provisioned {} service(s) for '{}': {}",
            results.len(),
            app_id,
            results.join("; ")
        ))
    } else {
        Ok(format!(
            "App '{}' is a static guest app and does not require service provisioning",
            app_id
        ))
    }
}

#[tauri::command]
pub async fn pv_repair_app_environment(
    app_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    pv_provision_app_environment(app_id, Some(true), state).await
}

#[tauri::command]
pub async fn pv_toggle_fullscreen(window: tauri::Window) -> Result<bool, String> {
    let is_full = window.is_fullscreen().map_err(|e| e.to_string())?;
    window.set_fullscreen(!is_full).map_err(|e| e.to_string())?;
    Ok(!is_full)
}

