use tauri::{
    menu::{Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    AppHandle, Emitter, Manager, Runtime,
};

pub struct HostMenu;

impl HostMenu {
    /// Build and attach cross-platform native menu bar
    pub fn build<R: Runtime>(app: &AppHandle<R>) -> Result<Menu<R>, tauri::Error> {
        // --- 1. File Menu ---
        let file_open_app =
            MenuItemBuilder::with_id("file_open_app", "Open App Bundle / Dashboard")
                .accelerator("CmdOrCtrl+O")
                .build(app)?;
        let file_close_app =
            MenuItemBuilder::with_id("file_close_app", "Close Active App (Return to Dashboard)")
                .accelerator("CmdOrCtrl+W")
                .build(app)?;
        let file_save_vault = MenuItemBuilder::with_id("file_save_vault", "Save Vault State")
            .accelerator("CmdOrCtrl+S")
            .build(app)?;
        let file_lock_vault = MenuItemBuilder::with_id("file_lock_vault", "Lock Vault / Logout")
            .accelerator("CmdOrCtrl+L")
            .build(app)?;

        let file_menu = SubmenuBuilder::new(app, "File")
            .item(&file_open_app)
            .item(&file_close_app)
            .separator()
            .item(&file_save_vault)
            .item(&file_lock_vault)
            .separator()
            .item(&PredefinedMenuItem::quit(app, Some("Exit"))?)
            .build()?;

        // --- 2. Security & Auth Menu ---
        let bio_label = if cfg!(target_os = "windows") {
            "Authenticate with Windows Hello"
        } else if cfg!(target_os = "macos") {
            "Authenticate with Touch ID"
        } else {
            "Authenticate with Linux PAM / Keyring"
        };

        let auth_biometric = MenuItemBuilder::with_id("auth_biometric", bio_label)
            .accelerator("CmdOrCtrl+Shift+A")
            .build(app)?;
        let auth_password =
            MenuItemBuilder::with_id("auth_password", "Unlock with Master Password...")
                .build(app)?;
        let auth_change_pass =
            MenuItemBuilder::with_id("auth_change_pass", "Change Master Passphrase...")
                .build(app)?;

        let autolock_5m = MenuItemBuilder::with_id("autolock_5m", "5 Minutes").build(app)?;
        let autolock_15m =
            MenuItemBuilder::with_id("autolock_15m", "15 Minutes (Default)").build(app)?;
        let autolock_1h = MenuItemBuilder::with_id("autolock_1h", "1 Hour").build(app)?;
        let autolock_never = MenuItemBuilder::with_id("autolock_never", "Never").build(app)?;

        let autolock_submenu = SubmenuBuilder::new(app, "Auto-Lock Timeout")
            .item(&autolock_5m)
            .item(&autolock_15m)
            .item(&autolock_1h)
            .item(&autolock_never)
            .build()?;

        let security_menu = SubmenuBuilder::new(app, "Security & Auth")
            .item(&auth_biometric)
            .item(&auth_password)
            .item(&auth_change_pass)
            .separator()
            .item(&autolock_submenu)
            .build()?;

        // --- 3. Apps & Catalog Menu ---
        let apps_dashboard =
            MenuItemBuilder::with_id("apps_dashboard", "Host App Launcher / Dashboard")
                .accelerator("CmdOrCtrl+Shift+H")
                .build(app)?;
        let apps_install_package = MenuItemBuilder::with_id(
            "apps_install_package",
            "Import .pvpkg Package Bundle...",
        )
        .accelerator("CmdOrCtrl+Shift+P")
        .build(app)?;
        let apps_install_local = MenuItemBuilder::with_id(
            "apps_install_local",
            "Mount App from Local Folder / USB...",
        )
        .accelerator("CmdOrCtrl+Shift+O")
        .build(app)?;
        let apps_install_github =
            MenuItemBuilder::with_id("apps_install_github", "Install App from GitHub Target...")
                .build(app)?;
        let apps_check_updates =
            MenuItemBuilder::with_id("apps_check_updates", "Check for App Updates").build(app)?;
        let apps_reload = MenuItemBuilder::with_id("apps_reload", "Reload Active App")
            .accelerator("F5")
            .build(app)?;

        let apps_menu = SubmenuBuilder::new(app, "Apps")
            .item(&apps_dashboard)
            .separator()
            .item(&apps_install_package)
            .item(&apps_install_local)
            .item(&apps_install_github)
            .item(&apps_check_updates)
            .separator()
            .item(&apps_reload)
            .build()?;

        // --- 4. Storage & Data Menu ---
        let data_export_package =
            MenuItemBuilder::with_id("data_export_package", "Export Portable Package (.pvpkg)...")
                .accelerator("CmdOrCtrl+E")
                .build(app)?;
        let data_export =
            MenuItemBuilder::with_id("data_export", "Export Decrypted App Snapshot (JSON)...")
                .build(app)?;
        let data_import =
            MenuItemBuilder::with_id("data_import", "Import Snapshot into Vault...").build(app)?;
        let data_bus = MenuItemBuilder::with_id("data_bus", "Inter-App Bus Monitor").build(app)?;
        let data_clear =
            MenuItemBuilder::with_id("data_clear", "Clear Local App Cache").build(app)?;

        let storage_menu = SubmenuBuilder::new(app, "Storage & Data")
            .item(&data_export_package)
            .separator()
            .item(&data_export)
            .item(&data_import)
            .separator()
            .item(&data_bus)
            .item(&data_clear)
            .build()?;

        // --- 5. View Menu ---
        let theme_slate =
            MenuItemBuilder::with_id("theme_slate", "Slate Dark (Default)").build(app)?;
        let theme_emerald =
            MenuItemBuilder::with_id("theme_emerald", "Cyber Emerald").build(app)?;
        let theme_sunset = MenuItemBuilder::with_id("theme_sunset", "Sunset Amber").build(app)?;
        let theme_solar = MenuItemBuilder::with_id("theme_solar", "Solar Light").build(app)?;

        let theme_submenu = SubmenuBuilder::new(app, "Theme")
            .item(&theme_slate)
            .item(&theme_emerald)
            .item(&theme_sunset)
            .item(&theme_solar)
            .build()?;

        let view_fullscreen = MenuItemBuilder::with_id("view_fullscreen", "Toggle Fullscreen")
            .accelerator("F11")
            .build(app)?;

        let view_menu = SubmenuBuilder::new(app, "View")
            .item(&theme_submenu)
            .separator()
            .item(&view_fullscreen)
            .build()?;

        // --- 6. Help Menu ---
        let help_docs =
            MenuItemBuilder::with_id("help_docs", "PixieVault Documentation").build(app)?;
        let help_verify =
            MenuItemBuilder::with_id("help_verify", "Verify Ed25519 Signatures").build(app)?;
        let help_about =
            MenuItemBuilder::with_id("help_about", "About PixieVault Host").build(app)?;

        let help_menu = SubmenuBuilder::new(app, "Help")
            .item(&help_docs)
            .item(&help_verify)
            .separator()
            .item(&help_about)
            .build()?;

        // --- Root Menu Bar Assembly ---
        let root_menu = MenuBuilder::new(app)
            .item(&file_menu)
            .item(&security_menu)
            .item(&apps_menu)
            .item(&storage_menu)
            .item(&view_menu)
            .item(&help_menu)
            .build()?;

        Ok(root_menu)
    }

    /// Handle menu selection events: dispatches to both app.emit and directly evaluates in webview
    pub fn handle_event<R: Runtime>(app: &AppHandle<R>, event_id: &str) {
        println!("[PixieVault Native Menu Event] Selected: {}", event_id);

        // Native Fullscreen toggle handling directly on OS window
        if event_id == "view_fullscreen" {
            for (_label, window) in app.webview_windows() {
                if let Ok(is_full) = window.is_fullscreen() {
                    let _ = window.set_fullscreen(!is_full);
                }
            }
        }

        // 1. Emit to Tauri event bus
        let _ = app.emit("menu_event", event_id);

        // 2. Directly evaluate JavaScript in all open windows
        let js = format!(
            "if (window.PixieVaultShell && typeof window.PixieVaultShell.handleNativeMenu === 'function') {{ window.PixieVaultShell.handleNativeMenu('{}'); }}",
            event_id
        );

        for (_label, window) in app.webview_windows() {
            let _ = window.eval(&js);
        }
    }
}
