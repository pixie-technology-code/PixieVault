use pixievault_lib::app_manager::PackageBundler;
use pixievault_lib::auth::MasterKey;
use pixievault_lib::storage::{VaultData, VaultStorage};
use std::fs;

#[test]
fn test_vault_persistence_creates_parent_directories() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let nested_vault_path = temp_dir
        .path()
        .join("nested")
        .join("deep")
        .join("vault_data.pvlt");
    let storage = VaultStorage::new(Some(nested_vault_path.clone()));

    let key = MasterKey([0x55; 32]);
    let mut data = VaultData::default();
    data.apps.insert(
        "test_app".into(),
        serde_json::json!({ "setting": "active" }),
    );

    storage
        .save(&data, &key)
        .expect("Saving to non-existent parent dirs must succeed");
    assert!(
        nested_vault_path.exists(),
        "Vault file must exist at nested path"
    );

    let loaded = storage
        .load(&key)
        .expect("Loading from nested path must succeed");
    assert_eq!(loaded.apps.get("test_app"), data.apps.get("test_app"));
}

#[test]
fn test_vault_persistence_corrupted_recovery() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let vault_path = temp_dir.path().join("vault_recovery.pvlt");
    let storage = VaultStorage::new(Some(vault_path.clone()));

    let key = MasterKey([0x99; 32]);
    let mut data = VaultData::default();
    data.apps.insert(
        "critical_app".into(),
        serde_json::json!({ "inventory": [1, 2, 3] }),
    );

    // 1. Initial valid save
    storage
        .save(&data, &key)
        .expect("Initial save must succeed");

    // 2. Modify and save again (this generates .bak of the first save)
    let mut updated_data = data.clone();
    updated_data.apps.insert(
        "critical_app".into(),
        serde_json::json!({ "inventory": [1, 2, 3, 4] }),
    );
    storage
        .save(&updated_data, &key)
        .expect("Second save must succeed");

    // 3. Corrupt primary file
    fs::write(&vault_path, "TRUNCATED_OR_GARBLED_PAYLOAD").expect("Failed to simulate corruption");

    // 4. Load must automatically recover from .bak
    let recovered = storage
        .load(&key)
        .expect("Corrupted vault must automatically recover from .bak");
    assert!(recovered.apps.contains_key("critical_app"));
}

#[test]
fn test_portable_app_data_exclusions() {
    assert!(PackageBundler::is_excluded(".venv"));
    assert!(PackageBundler::is_excluded("venv"));
    assert!(PackageBundler::is_excluded("env"));
    assert!(PackageBundler::is_excluded("__pycache__"));
    assert!(PackageBundler::is_excluded(".pytest_cache"));
    assert!(PackageBundler::is_excluded(".git"));
    assert!(PackageBundler::is_excluded(".secrets"));
    assert!(PackageBundler::is_excluded("node_modules"));
    assert!(PackageBundler::is_excluded("temp"));
    assert!(PackageBundler::is_excluded("app.pyc"));
    assert!(PackageBundler::is_excluded("database.db"));
    assert!(PackageBundler::is_excluded("state.sqlite"));
    assert!(PackageBundler::is_excluded("dump.sqlite3"));
    assert!(PackageBundler::is_excluded("scratch.tmp"));
    assert!(PackageBundler::is_excluded("app.py:Zone.Identifier"));

    // Code assets must NOT be excluded
    assert!(!PackageBundler::is_excluded("manifest.json"));
    assert!(!PackageBundler::is_excluded("index.html"));
    assert!(!PackageBundler::is_excluded("app.js"));
    assert!(!PackageBundler::is_excluded("styles.css"));
    assert!(!PackageBundler::is_excluded("requirements.txt"));
    assert!(!PackageBundler::is_excluded("app.py"));
}
