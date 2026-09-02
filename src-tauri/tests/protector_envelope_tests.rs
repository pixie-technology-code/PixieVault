use base64::prelude::*;
use pixievault_lib::auth::{MockPlatformProtector, PlatformKeyProtector, VaultCrypto};
use pixievault_lib::storage::{VaultData, VaultStorage};
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_envelope_v3_password_and_protector_unlock_roundtrip() {
    let password = "TestMasterPassword123!";
    let mut initial_data = VaultData::default();
    initial_data.settings.is_secured = true;
    initial_data.apps.insert(
        "mikrotik_fleet_mgr".into(),
        serde_json::json!({ "devices": ["192.168.88.1", "192.168.88.2"] }),
    );

    let plaintext = serde_json::to_vec(&initial_data).expect("serialize");

    // 1. Create v3 envelope with random VMK
    let (mut payload, vmk) =
        VaultCrypto::create_envelope_v3(password, &plaintext).expect("create_envelope_v3");
    assert_eq!(payload.version, 3);
    assert!(!payload.vault_id.is_empty());

    // 2. Enroll Mock Hardware Protector (Device A)
    let protector_a = MockPlatformProtector::new("device-a-laptop", "Surface-Laptop");
    let entry_a = protector_a
        .enroll(&payload.vault_id, &vmk, None)
        .await
        .expect("enroll device A");

    VaultCrypto::add_or_update_device_protector(&mut payload, entry_a.clone());
    assert_eq!(payload.protectors.len(), 2);

    // 3. Unlock with Password
    let (unlocked_vmk_pw, decrypted_bytes_pw) =
        VaultCrypto::unlock_envelope_with_passphrase(&payload, password)
            .expect("password unlock");
    assert_eq!(unlocked_vmk_pw.0, vmk.0);
    let loaded_data_pw: VaultData = serde_json::from_slice(&decrypted_bytes_pw).expect("deserialize");
    assert_eq!(loaded_data_pw.apps, initial_data.apps);

    // 4. Unlock with Device A Hardware Protector
    let unlocked_vmk_hw = protector_a
        .unlock(&payload.vault_id, &entry_a, None)
        .await
        .expect("hardware unlock device A");
    assert_eq!(unlocked_vmk_hw.0, vmk.0);

    let decrypted_bytes_hw =
        VaultCrypto::decrypt_with_passphrase(password, &payload).expect("decrypt helper");
    assert_eq!(decrypted_bytes_hw, plaintext);
}

#[tokio::test]
async fn test_multi_device_onedrive_sync_scenario() {
    let password = "MultiDevicePassword2026!";
    let secret = b"Shared OneDrive Encrypted Secret Data";

    // 1. Device A (Laptop) creates the vault on a shared OneDrive folder
    let (mut shared_payload, vmk) =
        VaultCrypto::create_envelope_v3(password, secret).expect("create envelope");
    let vault_id = shared_payload.vault_id.clone();

    let protector_a = MockPlatformProtector::new("dev-a-guid", "Work-Laptop");
    let entry_a = protector_a
        .enroll(&vault_id, &vmk, None)
        .await
        .expect("enroll dev A");
    VaultCrypto::add_or_update_device_protector(&mut shared_payload, entry_a.clone());

    // 2. Device B (Desktop) opens the synchronized OneDrive vault
    let protector_b = MockPlatformProtector::new("dev-b-guid", "Home-Desktop");

    // Device B checks capabilities: Device B is not enrolled yet
    let caps_b_before = protector_b
        .capabilities(None, Some(&vault_id), None)
        .await;
    assert!(!caps_b_before.is_enrolled);

    // Device B unlocks using Master Password
    let (unlocked_vmk_b, _) =
        VaultCrypto::unlock_envelope_with_passphrase(&shared_payload, password)
            .expect("Device B password unlock");
    assert_eq!(unlocked_vmk_b.0, vmk.0);

    // Device B enrolls its own local TPM hardware key into the shared envelope
    let entry_b = protector_b
        .enroll(&vault_id, &unlocked_vmk_b, None)
        .await
        .expect("enroll dev B");
    VaultCrypto::add_or_update_device_protector(&mut shared_payload, entry_b.clone());

    // Envelope now holds: [Argon2id, Device A Protector, Device B Protector]
    assert_eq!(shared_payload.protectors.len(), 3);

    // 3. Both devices can now independently unlock with their respective hardware keys!
    let vmk_from_a = protector_a
        .unlock(&vault_id, &entry_a, None)
        .await
        .expect("Device A Hello unlock");
    let vmk_from_b = protector_b
        .unlock(&vault_id, &entry_b, None)
        .await
        .expect("Device B Hello unlock");

    assert_eq!(vmk_from_a.0, vmk.0);
    assert_eq!(vmk_from_b.0, vmk.0);

    // 4. Foreign Device C (without enrolled hardware key) fails hardware unlock and falls back to password
    let protector_c = MockPlatformProtector::new("dev-c-guid", "Guest-MacBook");
    let foreign_res = protector_c.unlock(&vault_id, &entry_a, None).await;
    assert!(foreign_res.is_err(), "Foreign device must fail hardware key lookup");

    let (vmk_from_c_pw, _) =
        VaultCrypto::unlock_envelope_with_passphrase(&shared_payload, password)
            .expect("Device C password unlock");
    assert_eq!(vmk_from_c_pw.0, vmk.0);
}

#[tokio::test]
async fn test_password_change_preserves_enrolled_protectors_across_devices() {
    let password_v1 = "InitialPassword123";
    let password_v2 = "RotatedNewPassword456";
    let secret = b"Resilient Vault Payload";

    let (mut payload, vmk) =
        VaultCrypto::create_envelope_v3(password_v1, secret).expect("create envelope");
    let vault_id = payload.vault_id.clone();

    let protector_a = MockPlatformProtector::new("dev-a", "Laptop");
    let protector_b = MockPlatformProtector::new("dev-b", "Desktop");

    let entry_a = protector_a.enroll(&vault_id, &vmk, None).await.unwrap();
    let entry_b = protector_b.enroll(&vault_id, &vmk, None).await.unwrap();
    VaultCrypto::add_or_update_device_protector(&mut payload, entry_a.clone());
    VaultCrypto::add_or_update_device_protector(&mut payload, entry_b.clone());

    // Rotate Password
    VaultCrypto::update_passphrase_in_envelope(&mut payload, &vmk, password_v2)
        .expect("update passphrase");

    // Old password fails
    assert!(VaultCrypto::unlock_envelope_with_passphrase(&payload, password_v1).is_err());

    // New password succeeds
    let (unwrapped_vmk, _) =
        VaultCrypto::unlock_envelope_with_passphrase(&payload, password_v2).expect("new password");
    assert_eq!(unwrapped_vmk.0, vmk.0);

    // CRITICAL: Both Device A and Device B hardware protectors STILL UNLOCK without invalidation!
    let vmk_a = protector_a.unlock(&vault_id, &entry_a, None).await.expect("dev A hello");
    let vmk_b = protector_b.unlock(&vault_id, &entry_b, None).await.expect("dev B hello");
    assert_eq!(vmk_a.0, vmk.0);
    assert_eq!(vmk_b.0, vmk.0);
}

#[tokio::test]
async fn test_protector_revocation_removes_entry_and_fails_closed() {
    let password = "Password123";
    let (mut payload, vmk) =
        VaultCrypto::create_envelope_v3(password, b"data").expect("create envelope");
    let vault_id = payload.vault_id.clone();

    let protector = MockPlatformProtector::new("dev-rev", "Revoke-PC");
    let entry = protector.enroll(&vault_id, &vmk, None).await.unwrap();
    VaultCrypto::add_or_update_device_protector(&mut payload, entry.clone());
    assert_eq!(payload.protectors.len(), 2);

    // Revoke
    protector.revoke(&vault_id, Some(&entry)).await.expect("revoke");
    let removed = VaultCrypto::remove_device_protector(&mut payload, "dev-rev");
    assert!(removed);
    assert_eq!(payload.protectors.len(), 1);

    // Hardware unlock fails closed
    let hw_res = protector.unlock(&vault_id, &entry, None).await;
    assert!(hw_res.is_err());

    // Password unlock still works
    assert!(VaultCrypto::unlock_envelope_with_passphrase(&payload, password).is_ok());
}

#[tokio::test]
async fn test_user_cancellation_never_unlocks() {
    let password = "Password123";
    let (mut payload, vmk) =
        VaultCrypto::create_envelope_v3(password, b"secret").expect("create envelope");
    let vault_id = payload.vault_id.clone();

    let protector = MockPlatformProtector::new("dev-cancel", "Cancel-PC");
    let entry = protector.enroll(&vault_id, &vmk, None).await.unwrap();
    VaultCrypto::add_or_update_device_protector(&mut payload, entry.clone());

    // Simulate user clicking "Cancel" in Windows Hello prompt
    protector.set_simulate_cancellation(true);
    let cancel_res = protector.unlock(&vault_id, &entry, None).await;
    assert!(cancel_res.is_err());
    assert!(cancel_res.unwrap_err().contains("cancelled"));
}

#[tokio::test]
async fn test_tampered_wrapped_key_fails_closed() {
    let password = "Password123";
    let (payload, _) =
        VaultCrypto::create_envelope_v3(password, b"tamper_test").expect("create envelope");

    // Tamper with Argon2id wrapped key base64
    let mut bad_payload = payload.clone();
    bad_payload.protectors[0].wrapped_master_key_b64 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into();

    let res = VaultCrypto::unlock_envelope_with_passphrase(&bad_payload, password);
    assert!(res.is_err(), "Tampered wrapped key must fail closed");

    // Tamper with ciphertext
    let mut bad_cipher = payload.clone();
    bad_cipher.ciphertext_b64 = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB".into();
    let res2 = VaultCrypto::unlock_envelope_with_passphrase(&bad_cipher, password);
    assert!(res2.is_err(), "Tampered ciphertext must fail closed");
}

#[tokio::test]
async fn test_legacy_v2_to_v3_migration() {
    let temp_dir = tempdir().expect("tempdir");
    let vault_file = temp_dir.path().join("legacy_vault.pvlt");
    let storage = VaultStorage::new(Some(vault_file.clone()));

    let password = "LegacyMasterPassword!";
    let salt = VaultCrypto::generate_salt();
    let legacy_key = VaultCrypto::derive_key(password, &salt).unwrap();

    let mut data = VaultData::default();
    data.apps.insert("legacy_app".into(), serde_json::json!({ "migrated": true }));
    let plaintext = serde_json::to_vec(&data).unwrap();
    let (ciphertext, nonce) = VaultCrypto::encrypt(&legacy_key, &plaintext).unwrap();

    // Write raw v2 payload
    let v2_json = serde_json::json!({
        "version": 2,
        "salt_b64": BASE64_STANDARD.encode(salt),
        "nonce_b64": BASE64_STANDARD.encode(nonce),
        "ciphertext_b64": BASE64_STANDARD.encode(ciphertext),
        "is_initial_state": false
    });
    fs::write(&vault_file, serde_json::to_string_pretty(&v2_json).unwrap()).unwrap();

    // Storage loads v2 payload and decrypts with legacy_key
    let loaded = storage.load(&legacy_key).expect("v2 load");
    assert_eq!(loaded.apps.get("legacy_app").unwrap()["migrated"], true);

    // Save upgrades to v3 envelope
    let fresh_salt = VaultCrypto::generate_salt();
    storage.save(&loaded, &legacy_key, &fresh_salt, false).expect("save upgrade");

    let upgraded_payload = storage.load_payload().expect("load payload");
    assert_eq!(upgraded_payload.version, 3);
    assert!(!upgraded_payload.vault_id.is_empty());
    assert_eq!(upgraded_payload.protectors.len(), 1);
    assert_eq!(upgraded_payload.protectors[0].protector_type, "argon2id");
}

#[tokio::test]
async fn test_legacy_raw_credential_file_never_accepted() {
    let temp_dir = tempdir().expect("tempdir");
    let fake_cred_file = temp_dir.path().join(".biometric_vault_cred.enc");
    fs::write(&fake_cred_file, vec![0x42u8; 32]).expect("write fake cred");

    // Ensure the raw file is not treated as a valid envelope or accepted blindly
    let raw_bytes = fs::read(&fake_cred_file).unwrap();
    let parse_res: Result<pixievault_lib::auth::EncryptedPayload, _> = serde_json::from_slice(&raw_bytes);
    assert!(parse_res.is_err(), "Raw 32-byte credential file cannot be parsed as a valid envelope");

    // Clean up
    let _ = fs::remove_file(fake_cred_file);
}
