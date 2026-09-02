use pixievault_lib::app_manager::{AppManifest, CryptoVerifier, PythonProvisioningManager};
use pixievault_lib::auth::{MasterKey, VaultCrypto};
use std::collections::HashMap;

#[test]
fn test_argon2id_key_derivation_deterministic() {
    let passphrase = "SecretPassword2026!";
    let salt = b"PixieVaultSaltTest";

    let key1 = VaultCrypto::derive_key(passphrase, salt).expect("Failed key derivation 1");
    let key2 = VaultCrypto::derive_key(passphrase, salt).expect("Failed key derivation 2");

    assert_eq!(key1.0, key2.0, "Argon2id derivation must be deterministic");
}

#[test]
fn test_aes_gcm_encryption_and_decryption() {
    let key = MasterKey([0x77; 32]);
    let plaintext = b"Confidential PixieVault Payload Data";

    let (ciphertext, nonce) = VaultCrypto::encrypt(&key, plaintext).expect("Encryption failed");
    assert_ne!(ciphertext, plaintext);

    let decrypted = VaultCrypto::decrypt(&key, &ciphertext, &nonce).expect("Decryption failed");
    assert_eq!(decrypted, plaintext);
}

#[test]
fn test_ed25519_signature_verification() {
    use base64::prelude::*;
    use ed25519_dalek::Signer;
    use ed25519_dalek::SigningKey;

    let signing_key = SigningKey::from_bytes(&[0x42; 32]);
    let verifying_key = signing_key.verifying_key();
    let data = b"PixieVault Package Manifest v1.0.0";

    let signature = signing_key.sign(data);
    let sig_b64 = BASE64_STANDARD.encode(signature.to_bytes());
    let pub_b64 = BASE64_STANDARD.encode(verifying_key.to_bytes());

    let is_valid =
        CryptoVerifier::verify_signature(data, &sig_b64, &pub_b64).expect("Verification error");
    assert!(is_valid, "Valid signature must pass verification");

    let is_invalid =
        CryptoVerifier::verify_signature(b"Tampered Data", &sig_b64, &pub_b64).is_err();
    assert!(is_invalid, "Tampered data must fail verification");
}

#[test]
fn test_manifest_schema_and_launch_url_resolution() {
    let manifest_json = r#"{
      "app_id": "test_static_app",
      "name": "Test Static App",
      "version": "1.0.0",
      "min_pixievault_version": "0.2.0",
      "entrypoint": "index.html"
    }"#;

    let manifest: AppManifest =
        serde_json::from_str(manifest_json).expect("Failed to parse manifest");
    assert_eq!(manifest.app_id, "test_static_app");
    assert!(!manifest.has_composer());

    let launch_url = manifest.resolve_launch_url("test_static_app", None);
    assert_eq!(launch_url, "../test_static_app/index.html");
}

#[test]
fn test_manifest_composer_healthcheck_expected_status() {
    let manifest_json = r#"{
      "app_id": "test_composer_app",
      "name": "Test Composer App",
      "version": "1.0.0",
      "min_pixievault_version": "0.2.0",
      "entrypoint": "http://127.0.0.1:{{services.web.port}}/",
      "composer": {
        "version": "1",
        "services": {
          "web": {
            "command": ["python3", "app.py"],
            "port": "auto",
            "healthcheck": {
              "endpoint": "/api/health",
              "expected_status": 200,
              "expected_body": "OK"
            }
          }
        }
      }
    }"#;

    let manifest: AppManifest =
        serde_json::from_str(manifest_json).expect("Failed to parse composer manifest");
    assert!(manifest.has_composer());

    let composer = manifest.composer.as_ref().unwrap();
    let svc = composer.services.get("web").unwrap();
    let hc = svc.healthcheck.as_ref().unwrap();

    assert_eq!(hc.endpoint, "/api/health");
    assert_eq!(hc.expected_status, Some(200));
    assert_eq!(hc.expected_body, Some("OK".to_string()));

    let mut port_map = HashMap::new();
    port_map.insert("web".to_string(), 8080);
    assert_eq!(
        manifest.resolve_entrypoint(&port_map),
        "http://127.0.0.1:8080/"
    );
}

#[test]
fn test_manifest_validation_compatibility_floor() {
    use pixievault_lib::app_manager::{CompatibilityChecker, CompatibilityStatus, CURRENT_HOST_VERSION};

    // Valid manifest matching current host
    let valid_json = format!(
        r#"{{
            "app_id": "valid_compat_app",
            "name": "Valid App",
            "version": "1.0.0",
            "min_pixievault_version": "{}",
            "entrypoint": "index.html"
        }}"#,
        CURRENT_HOST_VERSION
    );
    let manifest: AppManifest = serde_json::from_str(&valid_json).unwrap();
    assert!(manifest.validate().is_ok());
    let report = CompatibilityChecker::check(&manifest);
    assert!(report.is_compatible);
    assert_eq!(report.status, CompatibilityStatus::Compatible);

    // Missing min_pixievault_version
    let missing_ver_json = r#"{
        "app_id": "missing_ver_app",
        "name": "Missing Ver App",
        "version": "1.0.0",
        "entrypoint": "index.html"
    }"#;
    let parse_res: Result<AppManifest, _> = serde_json::from_str(missing_ver_json);
    assert!(parse_res.is_err(), "Missing min_pixievault_version must fail deserialization");

    // Malformed min_pixievault_version (leading 'v' or non-semver)
    let malformed_ver_json = r#"{
        "app_id": "malformed_ver_app",
        "name": "Malformed Ver App",
        "version": "1.0.0",
        "min_pixievault_version": "v0.2.0",
        "entrypoint": "index.html"
    }"#;
    let manifest: AppManifest = serde_json::from_str(malformed_ver_json).unwrap();
    let val_res = manifest.validate();
    assert!(val_res.is_err(), "Malformed SemVer with 'v' prefix must fail validation");

    // Incompatible future host version
    let future_ver_json = r#"{
        "app_id": "future_app",
        "name": "Future App",
        "version": "1.0.0",
        "min_pixievault_version": "99.0.0",
        "entrypoint": "index.html"
    }"#;
    let manifest: AppManifest = serde_json::from_str(future_ver_json).unwrap();
    assert!(manifest.validate().is_ok());
    let report = CompatibilityChecker::check(&manifest);
    assert!(!report.is_compatible);
    assert_eq!(report.status, CompatibilityStatus::IncompatibleVersion);
    assert!(manifest.is_compatible_with_host(CURRENT_HOST_VERSION).is_err());
}

#[test]
fn test_dependency_fingerprint_computation() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let req_file = temp_dir.path().join("requirements.txt");
    std::fs::write(&req_file, "flask==3.0.0\nrequests>=2.31.0\n").unwrap();

    let fp1 =
        PythonProvisioningManager::compute_fingerprint(&req_file).expect("Failed fingerprint 1");
    let fp2 =
        PythonProvisioningManager::compute_fingerprint(&req_file).expect("Failed fingerprint 2");

    assert_eq!(fp1, fp2, "Fingerprint must be deterministic");
    assert_eq!(fp1.len(), 64, "SHA-256 hash string must be 64 characters");
}

#[test]
fn test_service_config_get_runtime_inference() {
    use pixievault_lib::app_manager::ServiceConfig;

    // 1. Python inference
    let py_svc: ServiceConfig = serde_json::from_str(
        r#"{
        "command": ["python3", "server.py"],
        "port": "auto"
    }"#,
    )
    .unwrap();
    assert_eq!(py_svc.get_runtime().runtime_type, "python");

    // 2. Node inference
    let node_svc: ServiceConfig = serde_json::from_str(
        r#"{
        "command": ["node", "index.js"],
        "port": "auto"
    }"#,
    )
    .unwrap();
    assert_eq!(node_svc.get_runtime().runtime_type, "node");

    // 3. Binary / Custom fallback
    let bin_svc: ServiceConfig = serde_json::from_str(
        r#"{
        "command": ["./my_native_app", "--port", "5000"],
        "port": "5000"
    }"#,
    )
    .unwrap();
    assert_eq!(bin_svc.get_runtime().runtime_type, "binary");
}

#[test]
fn test_runtime_provisioner_custom_install_failure_diagnostic() {
    use pixievault_lib::app_manager::{RuntimeProvisioner, ServiceConfig};
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");

    let svc_cfg: ServiceConfig = serde_json::from_str(
        r#"{
        "command": ["non_existent_binary_tool"],
        "port": "auto",
        "runtime": {
            "type": "custom",
            "install_command": ["false"]
        }
    }"#,
    )
    .unwrap();

    let diag = RuntimeProvisioner::provision_service(temp_dir.path(), "test_svc", &svc_cfg, true);
    assert!(diag.is_err());
    let err = diag.unwrap_err();
    assert_eq!(err.code, "custom_install_failed");
    assert_eq!(err.service, "test_svc");
}

#[test]
fn test_python_runtime_directory_is_separate_from_app_source() {
    use pixievault_lib::app_manager::PythonProvisioningManager;

    let temp = tempfile::tempdir().unwrap();
    let source_dir = temp.path().join("immutable_app");
    let runtime_dir = temp.path().join("app_data").join("runtime");
    std::fs::create_dir_all(&source_dir).unwrap();

    let python =
        PythonProvisioningManager::provision_environment_in(&source_dir, &runtime_dir, None, false)
            .expect("runtime provisioning should succeed outside the app source tree");

    assert!(python.exists());
    assert!(runtime_dir.exists());
    assert!(!source_dir.join(".venv").exists());
}

#[test]
fn test_first_launch_prepares_missing_python_runtime() {
    use pixievault_lib::app_manager::{RuntimeProvisioner, ServiceConfig};

    let temp = tempfile::tempdir().unwrap();
    let source_dir = temp.path().join("immutable_app");
    let runtime_dir = temp
        .path()
        .join("app_data")
        .join("runtimes")
        .join("backend");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(source_dir.join("requirements.txt"), "").unwrap();
    std::fs::write(source_dir.join("app.py"), "print('ready')\n").unwrap();

    let service: ServiceConfig = serde_json::from_str(
        r#"{
            "command": ["python3", "app.py"],
            "port": "auto",
            "runtime": {
                "type": "python",
                "requirements": "requirements.txt"
            }
        }"#,
    )
    .unwrap();

    let (python, args) = RuntimeProvisioner::prepare_service_execution_in(
        &source_dir,
        &runtime_dir,
        "backend",
        &service,
    )
    .expect("first launch should provision and resolve the Python runtime");

    assert!(python.exists());
    assert!(runtime_dir.join(".deps_fingerprint").exists());
    assert_eq!(args, vec!["app.py"]);
    assert!(!source_dir.join(".venv").exists());
}

#[test]
fn test_app_registry_scans_arbitrary_folders() {
    use pixievault_lib::app_manager::AppRegistry;
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let apps_dir = temp_dir.path().join("apps");
    std::fs::create_dir_all(&apps_dir).unwrap();

    // Create an app in a directory named "shell" (which was previously blacklisted)
    let shell_app_dir = apps_dir.join("shell");
    std::fs::create_dir_all(&shell_app_dir).unwrap();
    std::fs::write(
        shell_app_dir.join("manifest.json"),
        r#"{
        "app_id": "shell_utility_app",
        "name": "Shell Utility",
        "version": "1.0.0",
        "min_pixievault_version": "0.2.0",
        "entrypoint": "index.html"
    }"#,
    )
    .unwrap();

    // Create an app in a directory named "shared" (which was previously blacklisted)
    let shared_app_dir = apps_dir.join("shared");
    std::fs::create_dir_all(&shared_app_dir).unwrap();
    std::fs::write(
        shared_app_dir.join("manifest.json"),
        r#"{
        "app_id": "shared_resource_app",
        "name": "Shared Resource",
        "version": "1.0.0",
        "min_pixievault_version": "0.2.0",
        "entrypoint": "index.html"
    }"#,
    )
    .unwrap();

    let registry = AppRegistry::new(apps_dir);
    let apps = registry.list_apps();
    let app_ids: Vec<String> = apps.into_iter().map(|a| a.manifest.app_id).collect();

    assert!(
        app_ids.contains(&"shell_utility_app".to_string()),
        "Apps in 'shell' directory must now be discovered"
    );
    assert!(
        app_ids.contains(&"shared_resource_app".to_string()),
        "Apps in 'shared' directory must now be discovered"
    );
}
