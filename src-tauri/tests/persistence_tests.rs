use pixievault_lib::app_manager::PackageBundler;
use pixievault_lib::auth::{MasterKey, VaultCrypto, VaultSession};
use pixievault_lib::storage::{StorageError, VaultData, VaultStorage, WorkspaceManager};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[test]
fn test_vault_persistence_creates_parent_directories() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let nested_vault_path = temp_dir
        .path()
        .join("nested")
        .join("deep")
        .join("vault_data.pvlt");
    let storage = VaultStorage::new(Some(nested_vault_path.clone()));

    let salt = VaultCrypto::generate_salt();
    let key = MasterKey([0x55; 32]);
    let mut data = VaultData::default();
    data.apps.insert(
        "test_app".into(),
        serde_json::json!({ "setting": "active" }),
    );

    storage
        .save(&data, &key, &salt, false)
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

    let salt = VaultCrypto::generate_salt();
    let key = MasterKey([0x99; 32]);
    let mut data = VaultData::default();
    data.apps.insert(
        "critical_app".into(),
        serde_json::json!({ "inventory": [1, 2, 3] }),
    );

    // 1. Initial valid save
    storage
        .save(&data, &key, &salt, false)
        .expect("Initial save must succeed");

    // 2. Modify and save again (this generates .bak of the first save)
    let mut updated_data = data.clone();
    updated_data.apps.insert(
        "critical_app".into(),
        serde_json::json!({ "inventory": [1, 2, 3, 4] }),
    );
    storage
        .save(&updated_data, &key, &salt, false)
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
fn test_invalid_password_is_rejected_without_overwriting_vault() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let vault_path = temp_dir.path().join("vault_auth_test.pvlt");
    let storage = VaultStorage::new(Some(vault_path.clone()));

    let password = "CorrectMasterSecret123!";
    let salt = VaultCrypto::generate_salt();
    let correct_key = VaultCrypto::derive_key(password, &salt).expect("Key derivation");

    let mut data = VaultData::default();
    data.apps.insert(
        "cairn_dead_reckoning".into(),
        serde_json::json!({
            "netWorth": 2500000,
            "secretNotes": "CANARY_CONFIDENTIAL_ASSETS_9912"
        }),
    );

    // Save with correct key
    storage
        .save(&data, &correct_key, &salt, false)
        .expect("Save must succeed");

    // Try to load with wrong key
    let wrong_key = VaultCrypto::derive_key("WrongPassword456!", &salt).expect("Key derivation");
    let load_result = storage.load(&wrong_key);

    assert!(
        load_result.is_err(),
        "Loading with wrong password MUST fail with error"
    );
    match load_result {
        Err(StorageError::Crypto(_)) => {}
        other => panic!("Expected StorageError::Crypto, got {:?}", other),
    }

    // Verify correct key still decrypts original data accurately
    let reloaded = storage
        .load(&correct_key)
        .expect("Loading with correct password must succeed");
    assert_eq!(
        reloaded.apps.get("cairn_dead_reckoning"),
        data.apps.get("cairn_dead_reckoning")
    );
}

#[test]
fn test_per_vault_random_salt_generation() {
    let salt1 = VaultCrypto::generate_salt();
    let salt2 = VaultCrypto::generate_salt();
    assert_ne!(salt1, salt2, "Two generated salts must be distinct");

    let password = "SamePasswordAcrossVaults";
    let key1 = VaultCrypto::derive_key(password, &salt1).expect("Key derivation 1");
    let key2 = VaultCrypto::derive_key(password, &salt2).expect("Key derivation 2");
    assert_ne!(
        key1.0, key2.0,
        "Keys derived with different salts must produce distinct master keys"
    );
}

#[test]
fn test_locked_vault_leaves_zero_plaintext_inventory() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let data_root = temp_dir.path().to_path_buf();
    let vault_path = data_root.join("vault_data.pvlt");
    let storage = VaultStorage::new(Some(vault_path.clone()));
    let workspace = WorkspaceManager::new(&data_root);

    let canary_secrets = vec![
        "MIKROTIK_SUPER_SECRET_ROUTER_PASSWORD_881923",
        "CAIRN_FINANCIAL_PORTFOLIO_CONFIDENTIAL_BALANCE_9921",
        "CANARY_API_KEY_TOKEN_SECRET_XYZZY_7718",
    ];

    let password = "UltraSecureMasterVaultPassword2026!";
    let salt = VaultCrypto::generate_salt();
    let key = VaultCrypto::derive_key(password, &salt).expect("Key derivation");

    // 1. UNLOCKED SESSION: Materialize workspace and write sensitive files & state
    workspace
        .materialize_workspace()
        .expect("Materialize workspace");

    let mut mikrotik_files = HashMap::new();
    mikrotik_files.insert(
        "mac_finder.db".to_string(),
        format!(
            "SQLite format 3\x00 [DATABASE CONTAINING: {}]",
            canary_secrets[0]
        )
        .into_bytes(),
    );
    mikrotik_files.insert(
        "secrets/db_key.txt".to_string(),
        canary_secrets[2].as_bytes().to_vec(),
    );
    workspace
        .unpack_app_files("mikrotik_fleet", &mikrotik_files)
        .expect("Unpack mikrotik files");

    let mut cairn_files = HashMap::new();
    cairn_files.insert(
        "portfolio.json".to_string(),
        format!("{{\"secretBalance\": \"{}\"}}", canary_secrets[1]).into_bytes(),
    );
    workspace
        .unpack_app_files("cairn_dead_reckoning", &cairn_files)
        .expect("Unpack cairn files");

    let mut vault_data = VaultData::default();
    vault_data.apps.insert(
        "cairn_dead_reckoning".into(),
        serde_json::json!({
            "financialNote": canary_secrets[1]
        }),
    );

    // Verify canary strings ARE present during unlocked state in the workspace
    let mut found_before_lock = 0;
    for secret in &canary_secrets {
        if scan_directory_for_plaintext(&data_root, secret) {
            found_before_lock += 1;
        }
    }
    assert_eq!(
        found_before_lock,
        canary_secrets.len(),
        "All canary secrets must exist on disk during unlocked session"
    );

    // 2. LOCK VAULT: Harvest workspace files into vault data, save encrypted container, shred workspace
    let packed_mikrotik = workspace
        .pack_app_files("mikrotik_fleet")
        .expect("Pack mikrotik files");
    vault_data.set_app_files("mikrotik_fleet", packed_mikrotik);

    let packed_cairn = workspace
        .pack_app_files("cairn_dead_reckoning")
        .expect("Pack cairn files");
    vault_data.set_app_files("cairn_dead_reckoning", packed_cairn);

    storage
        .save(&vault_data, &key, &salt, false)
        .expect("Save encrypted vault");
    workspace
        .shred_and_remove_all()
        .expect("Shred and remove workspace");

    let mut session = VaultSession::default();
    session.unlock(key.clone(), false);
    session.lock(); // Zeroizes key


    // 3. DISK-INVENTORY SCAN: Traverse entire data directory tree and assert ZERO plaintext occurrences
    for secret in &canary_secrets {
        let found = scan_directory_for_plaintext(&data_root, secret);
        assert!(
            !found,
            "CRITICAL SECURITY VIOLATION: Sensitive plaintext marker '{}' was discovered on disk after vault was locked!",
            secret
        );
    }

    // Assert that the encrypted container does exist
    assert!(vault_path.exists(), "vault_data.pvlt must exist on disk");

    // 4. RE-UNLOCK: Verify data is 100% recovered on valid unlock
    let reloaded = storage
        .load(&key)
        .expect("Reloading vault with correct key must succeed");
    assert_eq!(
        reloaded.apps["cairn_dead_reckoning"]["financialNote"],
        canary_secrets[1]
    );
    let reloaded_files = reloaded
        .get_app_files("mikrotik_fleet")
        .expect("Reloaded app files");
    assert!(reloaded_files.contains_key("mac_finder.db"));
    assert!(reloaded_files.contains_key("secrets/db_key.txt"));
}

fn scan_directory_for_plaintext(dir: &Path, marker: &str) -> bool {
    let marker_bytes = marker.as_bytes();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if scan_directory_for_plaintext(&path, marker) {
                    return true;
                }
            } else if path.is_file() {
                // Ignore the encrypted container file (.pvlt) because it holds ciphertext
                if path.extension().and_then(|e| e.to_str()) == Some("pvlt")
                    || path.extension().and_then(|e| e.to_str()) == Some("bak")
                {
                    continue;
                }
                if let Ok(bytes) = fs::read(&path) {
                    if bytes
                        .windows(marker_bytes.len())
                        .any(|window| window == marker_bytes)
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
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

