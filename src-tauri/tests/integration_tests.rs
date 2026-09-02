use pixievault_lib::app_manager::{AppRegistry, InterAppBus};
use pixievault_lib::auth::{AuthStatus, MasterKey, VaultCrypto, VaultSession};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;

#[test]
fn test_crypto_argon2_and_aes_roundtrip_multi_apps() {
    let passphrase = "MasterVaultSecurePassword2026!";
    let data = json!({
        "mikrotik_fleet_mgr": { "devices": 14, "online": 12 },
        "cairn_dead_reckoning": { "netWorth": 1850000, "budget": 35000 }
    });

    let plaintext = serde_json::to_vec(&data).expect("Serialization failed");
    let encrypted =
        VaultCrypto::encrypt_with_passphrase(passphrase, &plaintext).expect("Encryption failed");

    let decrypted_bytes =
        VaultCrypto::decrypt_with_passphrase(passphrase, &encrypted).expect("Decryption failed");
    let decrypted_json: serde_json::Value =
        serde_json::from_slice(&decrypted_bytes).expect("Deserialization failed");

    assert_eq!(decrypted_json["mikrotik_fleet_mgr"]["devices"], 14);
    assert_eq!(decrypted_json["cairn_dead_reckoning"]["netWorth"], 1850000);
}

#[test]
fn test_session_lifecycle_and_zeroize() {
    let mut session = VaultSession::default();
    assert_eq!(session.status, AuthStatus::Locked);
    assert!(!session.is_unlocked());

    let key = MasterKey([0xFF; 32]);
    session.unlock(key);
    assert_eq!(session.status, AuthStatus::Unlocked);
    assert!(session.is_unlocked());
    assert!(session.unlocked_at.is_some());

    session.lock();
    assert_eq!(session.status, AuthStatus::Locked);
    assert!(!session.is_unlocked());
    assert!(session.active_key.is_none());
}

#[test]
fn test_app_registry_discovers_real_apps() {
    let mut manifest_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_root.pop(); // Go to workspace root
    let apps_dir = manifest_root.join("apps");

    let registry = AppRegistry::new(apps_dir);
    let apps = registry.list_apps();

    assert!(
        apps.len() >= 2,
        "Expected at least 2 installed apps in workspace"
    );
    let ids: Vec<String> = apps.iter().map(|a| a.manifest.app_id.clone()).collect();
    assert!(ids.contains(&"mikrotik_fleet_mgr".to_string()));
    assert!(ids.contains(&"cairn_dead_reckoning".to_string()));
}

#[test]
fn test_inter_app_bus_multi_app_telemetry() {
    let bus = InterAppBus::new();

    // 1. MikroTik Fleet Manager exports device metrics
    let mut fleet_metrics = HashMap::new();
    fleet_metrics.insert("onlineDevices".into(), json!(12));
    fleet_metrics.insert("totalDevices".into(), json!(14));
    bus.export_metrics("mikrotik_fleet_mgr", fleet_metrics);

    // 2. Cairn exports wealth architecture metrics
    let mut cairn_metrics = HashMap::new();
    cairn_metrics.insert("netWorth".into(), json!(1850000));
    cairn_metrics.insert("totalBuildCostBudget".into(), json!(35000));
    bus.export_metrics("cairn_dead_reckoning", cairn_metrics);

    // 3. Cairn queries MikroTik fleet metrics
    let queried_devices = bus.query_metric("mikrotik_fleet_mgr", "onlineDevices");
    assert_eq!(queried_devices, Some(json!(12)));

    // 4. MikroTik queries Cairn budget metrics
    let queried_budget = bus.query_metric("cairn_dead_reckoning", "totalBuildCostBudget");
    assert_eq!(queried_budget, Some(json!(35000)));
}

