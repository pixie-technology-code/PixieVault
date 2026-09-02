use base64::prelude::*;
use ed25519_dalek::{Signer, SigningKey};
use pixievault_lib::app_manager::{AppRegistry, AppSource, CryptoVerifier, PackageBundler};
use rand::rngs::OsRng;
use std::fs;
use std::path::PathBuf;


#[test]
fn test_package_bundler_export_and_extract_roundtrip() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let app_dir = temp_dir.path().join("my_test_app");
    fs::create_dir_all(&app_dir).unwrap();

    let manifest_content = r#"{
      "app_id": "test_app_v1",
      "name": "Test Portable App",
      "version": "1.0.0",
      "description": "Portable all-in-one bundle test",
      "entrypoint": "index.html"
    }"#;
    fs::write(app_dir.join("manifest.json"), manifest_content).unwrap();
    fs::write(app_dir.join("index.html"), "<h1>Hello Portable World</h1>").unwrap();
    fs::write(app_dir.join("styles.css"), "body { background: black; }").unwrap();

    let output_package = temp_dir.path().join("test_app.pvpkg");
    let vault_data = b"AES256GCM_ENCRYPTED_VAULT_BYTES_12345";

    // 1. Export Package
    PackageBundler::export_package(&app_dir, Some(vault_data), &output_package)
        .expect("Export package failed");
    assert!(output_package.exists());

    // 2. Extract Package into isolated location
    let extract_dir = temp_dir.path().join("extracted_app");
    let (manifest, extracted_vault) =
        PackageBundler::extract_package(&output_package, &extract_dir)
            .expect("Extract package failed");

    assert_eq!(manifest.app_id, "test_app_v1");
    assert_eq!(manifest.name, "Test Portable App");
    assert_eq!(extracted_vault, Some(vault_data.to_vec()));

    let extracted_html = fs::read_to_string(extract_dir.join("index.html")).unwrap();
    assert_eq!(extracted_html, "<h1>Hello Portable World</h1>");
}

#[test]
fn test_ed25519_signature_verification_valid_and_tampered() {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();

    let data = b"PIXIEVAULT_RELEASE_PAYLOAD_V1.2.0_HASH";
    let signature = signing_key.sign(data);

    let pubkey_b64 = BASE64_STANDARD.encode(verifying_key.to_bytes());
    let sig_b64 = BASE64_STANDARD.encode(signature.to_bytes());

    // 1. Valid signature must pass
    let is_valid = CryptoVerifier::verify_signature(data, &sig_b64, &pubkey_b64)
        .expect("Verification function failed");
    assert!(is_valid, "Valid signature must verify to true");

    // 2. Tampered data must fail
    let tampered_data = b"PIXIEVAULT_TAMPERED_MALICIOUS_PAYLOAD";
    let is_tampered_valid = CryptoVerifier::verify_signature(tampered_data, &sig_b64, &pubkey_b64);
    assert!(
        is_tampered_valid.is_err(),
        "Tampered payload must fail verification"
    );
}

#[test]
fn test_registry_local_directory_and_github_targets() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let apps_root = temp_dir.path().join("apps");
    fs::create_dir_all(&apps_root).unwrap();

    let registry = AppRegistry::new(apps_root.clone());

    // 1. Register a local folder
    let local_dev_dir = temp_dir.path().join("local_dev_tool");
    fs::create_dir_all(&local_dev_dir).unwrap();
    let manifest = r#"{
      "app_id": "local_dev_tool",
      "name": "Local Dev Tool",
      "version": "0.1.0",
      "entrypoint": "index.html"
    }"#;
    fs::write(local_dev_dir.join("manifest.json"), manifest).unwrap();

    let installed_local = registry
        .install_local_directory(local_dev_dir.clone())
        .expect("Install local directory failed");
    assert_eq!(installed_local.manifest.app_id, "local_dev_tool");
    assert_eq!(
        installed_local.source,
        AppSource::LocalDirectory(local_dev_dir)
    );

    // 2. Register a GitHub target (pre-downloaded / provisioned target directory)
    let gh_install_dir = apps_root.join("williamhart_dyno_tuner");
    fs::create_dir_all(&gh_install_dir).unwrap();
    let gh_manifest = r#"{
      "app_id": "williamhart_dyno_tuner",
      "name": "Dyno Tuner",
      "version": "v1.4.0",
      "entrypoint": "index.html"
    }"#;
    fs::write(gh_install_dir.join("manifest.json"), gh_manifest).unwrap();

    let installed_gh = registry
        .install_github_target(
            "williamhart-az/dyno-tuner",
            Some("v1.4.0"),
            Some("base64_pubkey_placeholder"),
            &gh_install_dir,
        )
        .expect("Install GitHub target failed");

    assert_eq!(installed_gh.manifest.version, "v1.4.0");
    if let AppSource::GitHubRelease {
        repository, tag, ..
    } = installed_gh.source
    {
        assert_eq!(repository, "williamhart-az/dyno-tuner");
        assert_eq!(tag, "v1.4.0");
    } else {
        panic!("Expected GitHubRelease source type");
    }
}

#[test]
fn test_mikrotik_fleet_pvpkg_package_bundle_verification() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let app_dir = root.join("apps").join("mikrotik_fleet");
    assert!(app_dir.join("manifest.json").exists(), "apps/mikrotik_fleet/manifest.json must exist");
    assert!(app_dir.join("assets").join("app-icon.svg").exists(), "app-icon.svg must exist");

    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let pkg_path = temp_dir.path().join("mikrotik_fleet_mgr.pvpkg");

    // 1. Export package bundle
    PackageBundler::export_package(&app_dir, None, &pkg_path).expect("Export failed");
    assert!(pkg_path.exists());

    // 2. Extract and verify contents
    let extract_dir = temp_dir.path().join("extracted");
    let (manifest, _) = PackageBundler::extract_package(&pkg_path, &extract_dir).expect("Extract failed");
    assert_eq!(manifest.app_id, "mikrotik_fleet_mgr");
    assert_eq!(manifest.name, "MikroTik Fleet Manager");
    assert!(extract_dir.join("manifest.json").exists());
    assert!(extract_dir.join("assets").join("app-icon.svg").exists());
    assert!(extract_dir.join("backend").join("app.py").exists());
    assert!(extract_dir.join("backend").join("requirements.txt").exists());

    // Verify zero leaked state inside package
    assert!(!extract_dir.join("backend").join(".venv").exists());
    assert!(!extract_dir.join("backend").join("__pycache__").exists());
    assert!(!extract_dir.join("backend").join(".secrets").exists());
}

#[test]
fn test_cairn_dead_reckoning_pvpkg_package_bundle_verification() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let app_dir = root.join("apps").join("cairn_dead_reckoning");
    assert!(app_dir.join("manifest.json").exists(), "apps/cairn_dead_reckoning/manifest.json must exist");
    assert!(app_dir.join("assets").join("app-icon.svg").exists(), "app-icon.svg must exist");

    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let pkg_path = temp_dir.path().join("cairn_dead_reckoning.pvpkg");

    // 1. Export package bundle
    PackageBundler::export_package(&app_dir, None, &pkg_path).expect("Export failed");
    assert!(pkg_path.exists());

    // 2. Extract and verify contents
    let extract_dir = temp_dir.path().join("extracted");
    let (manifest, _) = PackageBundler::extract_package(&pkg_path, &extract_dir).expect("Extract failed");
    assert_eq!(manifest.app_id, "cairn_dead_reckoning");
    assert_eq!(manifest.name, "Cairn: Dead Reckoning");
    assert!(extract_dir.join("manifest.json").exists());
    assert!(extract_dir.join("assets").join("app-icon.svg").exists());
    assert!(extract_dir.join("index.html").exists());
    assert!(extract_dir.join("styles.css").exists());
    assert!(extract_dir.join("cairn_app.js").exists());
    assert!(extract_dir.join("cairn_storage.js").exists());
}


