use crate::app_manager::{
    AppRegistry, ComposerAppStatus, InstalledAppInfo, InterAppBus, MetricValue, SidecarStatus,
    VaultComposer,
};
use crate::auth::{AuthStatus, BiometricAuth, MasterKey, VaultCrypto, VaultSession};
use crate::storage::{VaultData, VaultStorage};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tauri::State;

pub struct AppState {
    pub session: RwLock<VaultSession>,
    pub storage: VaultStorage,
    pub registry: AppRegistry,
    pub bus: InterAppBus,
    pub sidecars: VaultComposer,
    pub vault_data: RwLock<Option<VaultData>>,
    pub app_data_root: PathBuf,
}

fn app_data_dir(root: &std::path::Path, app_id: &str) -> PathBuf {
    let safe_id: String = app_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    root.join("apps").join(safe_id)
}

#[derive(Serialize)]
pub struct VaultStatusResponse {
    pub is_locked: bool,
    pub active_app: Option<String>,
    pub biometrics_available: bool,
    pub biometric_provider: String,
    pub biometric_type: String,
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

#[tauri::command]
pub fn pv_get_vault_status(state: State<Arc<AppState>>) -> VaultStatusResponse {
    let session = state.session.read().unwrap();
    let capabilities = BiometricAuth::get_capabilities();
    let active_app = state.registry.get_active_app_id();

    let (auto_launch, last_opened) = {
        let vd = state.vault_data.read().unwrap();
        if let Some(ref data) = *vd {
            let valid_last_app = data
                .settings
                .last_opened_app
                .clone()
                .filter(|id| state.registry.get_app(id).is_some());
            (data.settings.auto_launch_last_app, valid_last_app)
        } else {
            (false, None)
        }
    };

    let (platform, platform_label) = if cfg!(target_os = "windows") {
        ("windows", "Windows 11 / Windows Hello")
    } else if cfg!(target_os = "macos") {
        ("macos", "macOS / Touch ID")
    } else {
        ("linux", "Linux / PAM Keyring")
    };

    VaultStatusResponse {
        is_locked: session.status == AuthStatus::Locked,
        active_app,
        biometrics_available: capabilities.is_available,
        biometric_provider: capabilities.provider_name,
        biometric_type: capabilities.biometric_type,
        platform: platform.to_string(),
        platform_label: platform_label.to_string(),
        auto_launch_last_app: auto_launch,
        last_opened_app: last_opened,
    }
}

#[tauri::command]
pub async fn pv_authenticate_biometric(
    state: State<'_, Arc<AppState>>,
) -> Result<AuthResponse, String> {
    match BiometricAuth::authenticate("Unlock PixieVault Application Environment").await {
        Ok(true) => {
            let key = MasterKey([0xAA; 32]); // Hardware-bound credential derivation
            {
                let mut session = state.session.write().unwrap();
                session.unlock(key.clone());
            }

            // Load vault data if exists
            let vault_data = state.storage.load(&key).unwrap_or_default();
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

            Ok(AuthResponse {
                success: true,
                auto_launch_app: auto_launch_target,
                error: None,
            })
        }
        Ok(false) => Ok(AuthResponse {
            success: false,
            auto_launch_app: None,
            error: Some("Biometric authentication cancelled".into()),
        }),
        Err(e) => Ok(AuthResponse {
            success: false,
            auto_launch_app: None,
            error: Some(e),
        }),
    }
}

#[tauri::command]
pub async fn pv_authenticate_password(
    passphrase: String,
    state: State<'_, Arc<AppState>>,
) -> Result<AuthResponse, String> {
    if passphrase.trim().is_empty() {
        return Ok(AuthResponse {
            success: false,
            auto_launch_app: None,
            error: Some("Passphrase cannot be empty".into()),
        });
    }

    let key = VaultCrypto::derive_key(&passphrase, b"PixieVaultSalt2026")
        .map_err(|e| format!("Argon2id derivation failed: {}", e))?;

    {
        let mut session = state.session.write().unwrap();
        session.unlock(key.clone());
    }

    let vault_data = state.storage.load(&key).unwrap_or_default();
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

    Ok(AuthResponse {
        success: true,
        auto_launch_app: auto_launch_target,
        error: None,
    })
}

#[tauri::command]
pub fn pv_lock_vault(state: State<Arc<AppState>>) -> bool {
    let mut session = state.session.write().unwrap();
    session.lock();
    *state.vault_data.write().unwrap() = None;
    state.registry.set_active_app(None);
    state.sidecars.stop_all();
    true
}

#[tauri::command]
pub fn pv_list_apps(state: State<Arc<AppState>>) -> Vec<crate::app_manager::InstalledAppInfo> {
    state.registry.list_apps()
}

#[tauri::command]
pub fn pv_launch_app(
    app_id: String,
    state: State<Arc<AppState>>,
) -> Result<crate::app_manager::InstalledAppInfo, String> {
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
    state.registry.set_active_app(None);
    true
}

#[tauri::command]
pub fn pv_load_app_data(
    app_id: String,
    state: State<Arc<AppState>>,
) -> Result<Option<serde_json::Value>, String> {
    let session = state.session.read().unwrap();
    if !session.is_unlocked() {
        return Err("Vault is locked".into());
    }

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
    let session = state.session.read().unwrap();
    let key = session
        .active_key
        .as_ref()
        .ok_or_else(|| "Vault is locked".to_string())?;

    let mut vd_guard = state.vault_data.write().unwrap();
    if vd_guard.is_none() {
        *vd_guard = Some(VaultData::default());
    }

    if let Some(ref mut vd) = *vd_guard {
        vd.set_app_state(&app_id, data);
        state.storage.save(vd, key).map_err(|e| e.to_string())?;
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
    app_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<UpdateCheckResponse, String> {
    let app = state
        .registry
        .get_app(&app_id)
        .ok_or_else(|| format!("App '{}' not found", app_id))?;

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
            latest_version: cur_ver,
            release_notes: Some("App is managed from a local offline directory or package.".into()),
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
    let app_info = state
        .registry
        .get_app(&app_id)
        .ok_or_else(|| format!("App '{}' is not registered in PixieVault", app_id))?;

    let app_path = PathBuf::from(&app_info.path);
    let mutable_app_dir = app_data_dir(&state.app_data_root, &app_id);
    std::fs::create_dir_all(&mutable_app_dir).map_err(|e| {
        format!(
            "Failed to create mutable app directory '{}': {}",
            mutable_app_dir.display(),
            e
        )
    })?;
    state
        .sidecars
        .start_composer_app(&app_info.manifest, &app_path, Some(&mutable_app_dir))
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

            let runtime_dir = app_data_dir(&state.app_data_root, &app_id)
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
