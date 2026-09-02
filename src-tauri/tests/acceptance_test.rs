use pixievault_lib::app_manager::{AppRegistry, PythonProvisioningManager, VaultComposer};
use pixievault_lib::auth::MasterKey;
use pixievault_lib::storage::{VaultData, VaultStorage};
use std::path::PathBuf;

#[test]
fn test_core_task_end_to_end_acceptance_workflow() {
    println!("\n========================================================");
    println!("  PixieVault End-to-End Core Task Acceptance Gate        ");
    println!("========================================================");

    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let test_vault_file = temp_dir.path().join("acceptance_vault.pvlt");
    let storage = VaultStorage::new(Some(test_vault_file.clone()));

    // Step 1: Initialize PixieVault AppState & Discover Apps
    println!("\n[1/7] Initializing Registry & Discovering Bundled Apps...");
    let apps_dir_candidates = [PathBuf::from("apps"), PathBuf::from("../apps")];
    let apps_root = apps_dir_candidates
        .into_iter()
        .find(|p| p.exists())
        .expect("apps directory not found");
    let registry = AppRegistry::new(apps_root.clone());
    let installed_apps = registry.list_apps();

    assert!(installed_apps.len() >= 2, "Must discover all bundled apps");
    let mikrotik_app = installed_apps
        .iter()
        .find(|a| a.manifest.app_id == "mikrotik_fleet_mgr")
        .expect("MikroTik Fleet Manager must be discovered in registry");
    let cairn_app = installed_apps
        .iter()
        .find(|a| a.manifest.app_id == "cairn_dead_reckoning")
        .expect("Cairn: Dead Reckoning must be discovered in registry");

    println!(
        "✓ Discovered MikroTik Fleet Manager v{}",
        mikrotik_app.manifest.version
    );
    println!(
        "✓ Discovered Cairn: Dead Reckoning v{}",
        cairn_app.manifest.version
    );
    println!("  MikroTik App Path: {}", mikrotik_app.path);
    println!("  Is Composer: {}", mikrotik_app.is_composer);

    // Step 2: Verify and Provision Runtime Environment
    println!("\n[2/7] Verifying & Provisioning Python Runtime...");
    let app_dir = PathBuf::from(&mikrotik_app.path);
    let composer_cfg = mikrotik_app
        .manifest
        .composer
        .as_ref()
        .expect("Composer config required");
    let backend_cfg = composer_cfg
        .services
        .get("backend")
        .expect("Backend service required");

    let working_dir = mikrotik_app
        .manifest
        .resolve_service_working_dir(&app_dir, "backend")
        .expect("Failed to resolve backend working directory");

    println!("  Working directory: {:?}", working_dir);
    let runtime_dir = temp_dir.path().join("runtimes").join("backend");
    let py_path = PythonProvisioningManager::provision_environment_in(
        &working_dir,
        &runtime_dir,
        backend_cfg.requirements.as_deref(),
        false,
    )
    .expect("Python environment provisioning must succeed");
    println!("✓ Python Runtime Ready: {:?}", py_path);

    // Step 3: Start Backend Service on Dynamic Ephemeral Port
    println!("\n[3/7] Starting Native Composer Backend on Ephemeral Port...");
    let composer = VaultComposer::new();
    let status = composer
        .start_composer_app(&mikrotik_app.manifest, &app_dir, Some(temp_dir.path()))
        .expect("Composer startup and readiness probe must succeed");

    assert!(status.is_running, "Composer app must be reported running");
    assert!(
        status.entrypoint_url.starts_with("http://127.0.0.1:"),
        "Entrypoint URL must be resolved"
    );
    println!(
        "✓ Backend Service Started & Healthy: {}",
        status.entrypoint_url
    );

    // Step 4: Validate Guest Frame URL Resolution
    println!("\n[4/7] Validating Guest Viewport URL Resolution...");
    let resolved_url = &status.entrypoint_url;
    assert!(
        resolved_url.contains("127.0.0.1:"),
        "Resolved URL must point to loopback address"
    );
    println!("✓ Guest Viewport Target: {}", resolved_url);

    // Step 5: Persist Encrypted Application State in Vault
    println!("\n[5/7] Encrypting & Persisting State to Vault...");
    let master_key = MasterKey([0xFE; 32]);
    let salt = pixievault_lib::auth::VaultCrypto::generate_salt();
    let mut vault_data = VaultData::default();
    let fleet_state = serde_json::json!({
        "devices": [
            { "id": "router-01", "name": "Core-CCR2004", "ip": "192.168.88.1", "status": "online" },
            { "id": "switch-01", "name": "Dist-CRS328", "ip": "192.168.88.2", "status": "online" }
        ],
        "active_topology": "star",
        "last_scan_utc": "2026-09-01T23:30:00Z"
    });
    vault_data.set_app_state("mikrotik_fleet_mgr", fleet_state.clone());
    storage
        .save(&vault_data, &master_key, &salt, false)
        .expect("Vault persistence must succeed");
    println!("✓ State Encrypted & Saved to {}", test_vault_file.display());


    // Step 6: Stop Composer Services & Verify Teardown
    println!("\n[6/7] Terminating Composer Services (Zero-Trust Teardown)...");
    let stopped = composer.stop_composer_app("mikrotik_fleet_mgr");
    assert!(stopped, "Composer stop must return true");

    let post_status = composer.get_app_status("mikrotik_fleet_mgr", Some(&mikrotik_app.manifest));
    assert!(!post_status.is_running, "All services must be terminated");
    println!("✓ All child processes reaped and ports freed.");

    // Step 7: Reopen Vault & Verify State Roundtrip
    println!("\n[7/7] Reopening & Decrypting Vault to Verify Persistence Roundtrip...");
    let reloaded_storage = VaultStorage::new(Some(test_vault_file));
    let reloaded_vault = reloaded_storage
        .load(&master_key)
        .expect("Vault reload & decryption must succeed");

    let saved_fleet = reloaded_vault
        .get_app_state("mikrotik_fleet_mgr")
        .expect("App state must exist");
    assert_eq!(
        saved_fleet, &fleet_state,
        "Decrypted state must match original exactly"
    );
    println!("✓ Persisted state roundtrip verified perfectly!");

    println!("\n========================================================");
    println!("  ✓ CORE TASK ACCEPTANCE TEST PASSED SUCCESSFULLY!       ");
    println!("========================================================");
}
