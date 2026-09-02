pub mod app_manager;
pub mod auth;
pub mod commands;
pub mod menu;
pub mod storage;

use app_manager::{AppRegistry, InterAppBus};
use auth::VaultSession;
use commands::AppState;
use menu::HostMenu;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use storage::{VaultStorage, WorkspaceManager};
use tauri::{Emitter, Manager};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let path_resolver = app.path();

            // 1. Resolve read-only bundled applications directory
            // Strict canonical contract: resource_dir/apps/ in release, or <workspace>/apps/ in debug
            let bundled_apps_dir = if cfg!(debug_assertions) {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .map(|p| p.join("apps"))
                    .unwrap_or_else(|| PathBuf::from("apps"))
            } else {
                path_resolver
                    .resource_dir()
                    .map_err(|e| format!("Unable to resolve resource directory: {e}"))?
                    .join("apps")
            };

            let _ = std::fs::create_dir_all(&bundled_apps_dir);

            if !bundled_apps_dir.is_dir() {
                return Err(format!(
                    "Bundled applications directory is missing: {}",
                    bundled_apps_dir.display()
                )
                .into());
            }

            // 2. Resolve mutable application data directory
            let app_data_dir = path_resolver.app_data_dir().unwrap_or_else(|_| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join("data")
            });
            let _ = std::fs::create_dir_all(&app_data_dir);

            let vault_file = app_data_dir.join("vault_data.pvlt");
            let user_apps_dir = app_data_dir.join("installed_apps");
            let _ = std::fs::create_dir_all(&user_apps_dir);

            let registry = AppRegistry::new_with_user_apps(bundled_apps_dir, Some(user_apps_dir));
            let workspace = WorkspaceManager::new(&app_data_dir);

            let app_state = Arc::new(AppState {
                session: RwLock::new(VaultSession::default()),
                storage: VaultStorage::new(Some(vault_file)),
                workspace,
                registry,
                bus: InterAppBus::new(),
                sidecars: app_manager::SidecarManager::new(),
                vault_data: RwLock::new(None),
                vault_salt: RwLock::new(None),
                app_data_root: app_data_dir,
            });

            app.manage(app_state.clone());

            // 3. Spawn background auto-lock supervisor ticker (checks every 5 seconds)
            let auto_lock_state = app_state.clone();
            let app_handle_for_lock = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
                loop {
                    interval.tick().await;
                    let should_lock = {
                        let session = auto_lock_state.session.read().unwrap();
                        session.is_auto_lock_expired()
                    };

                    if should_lock {
                        eprintln!("[PixieVault Host] Inactivity timeout reached. Auto-locking vault...");
                        commands::lock_vault_internal(&auto_lock_state);
                        let _ = app_handle_for_lock.emit("pv_vault_autolocked", ());
                    }

                }
            });

            // Build and attach native menu
            let menu = HostMenu::build(app.handle())?;
            app.set_menu(menu)?;

            // Register native menu event handler
            let app_handle = app.handle().clone();
            app.on_menu_event(move |_window, event| {
                HostMenu::handle_event(&app_handle, event.id().as_ref());
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::pv_get_vault_status,
            commands::pv_windows_hello_capabilities,
            commands::pv_windows_hello_enroll,
            commands::pv_windows_hello_unlock,
            commands::pv_windows_hello_revoke,
            commands::pv_authenticate_biometric,
            commands::pv_authenticate_password,
            commands::pv_set_master_passphrase,
            commands::pv_lock_vault,
            commands::pv_list_apps,
            commands::pv_launch_app,
            commands::pv_unload_app,
            commands::pv_load_app_data,
            commands::pv_save_app_data,
            commands::pv_export_metrics,
            commands::pv_query_adjacent_metric,
            commands::pv_get_bus_metrics,
            commands::pv_pick_and_install_local_app,
            commands::pv_pick_and_install_package_file,
            commands::pv_pick_and_install_local_folder,
            commands::pv_check_package_compatibility,
            commands::pv_check_manifest_compatibility,
            commands::pv_install_local_directory,
            commands::pv_install_package_file,
            commands::pv_export_app_package,
            commands::pv_install_github_app,
            commands::pv_check_app_updates,
            commands::pv_start_sidecar,
            commands::pv_stop_sidecar,
            commands::pv_get_sidecar_status,
            commands::pv_composer_start_app,
            commands::pv_composer_stop_app,
            commands::pv_composer_get_status,
            commands::pv_provision_app_environment,
            commands::pv_repair_app_environment,
            commands::pv_toggle_fullscreen
        ])
        .run(tauri::generate_context!())
        .expect("error while running PixieVault host application");
}

