use super::manifest::AppManifest;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

/// Application Distribution & Sourcing Modes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "details")]
pub enum AppSource {
    /// 100% Offline Local Directory (for active dev, cold storage, USB)
    LocalDirectory(PathBuf),
    /// All-in-One Portable Archive (.pvpkg / .pvapp)
    PortablePackage(PathBuf),
    /// Remote GitHub Release Target (with optional Ed25519 signature verification)
    GitHubRelease {
        repository: String,
        tag: String,
        public_key: Option<String>,
        last_checked: Option<String>,
    },
}

impl Default for AppSource {
    fn default() -> Self {
        AppSource::LocalDirectory(PathBuf::from("."))
    }
}

/// Package Bundler for all-in-one .pvpkg portable archives
pub struct PackageBundler;

impl PackageBundler {
    /// Export an app directory and optional encrypted vault database into a .pvpkg archive
    pub fn export_package(
        app_dir: &Path,
        vault_data: Option<&[u8]>,
        output_file: &Path,
    ) -> Result<(), String> {
        let manifest_path = app_dir.join("manifest.json");
        if !manifest_path.exists() {
            return Err(format!("Manifest missing at {}", manifest_path.display()));
        }

        if let Some(parent) = output_file.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let file = File::create(output_file)
            .map_err(|e| format!("Failed to create output file: {}", e))?;
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // 1. Write manifest.json
        let manifest_bytes = fs::read(&manifest_path).map_err(|e| e.to_string())?;
        zip.start_file("manifest.json", options)
            .map_err(|e| e.to_string())?;
        zip.write_all(&manifest_bytes).map_err(|e| e.to_string())?;

        // 2. Write code files recursively
        Self::add_directory_to_zip(&mut zip, app_dir, app_dir, options)?;

        // 3. Write encrypted data container if provided
        if let Some(data) = vault_data {
            zip.start_file("data/encrypted_state.pvlt", options)
                .map_err(|e| e.to_string())?;
            zip.write_all(data).map_err(|e| e.to_string())?;
        }

        zip.finish()
            .map_err(|e| format!("Failed to finalize zip archive: {}", e))?;
        Ok(())
    }

    /// Extract an all-in-one .pvpkg archive into a destination directory safely with ZipSlip protection
    pub fn extract_package(
        package_file: &Path,
        destination_dir: &Path,
    ) -> Result<(AppManifest, Option<Vec<u8>>), String> {
        let file =
            File::open(package_file).map_err(|e| format!("Failed to open package file: {}", e))?;
        let mut archive =
            ZipArchive::new(file).map_err(|e| format!("Invalid zip archive: {}", e))?;

        fs::create_dir_all(destination_dir).map_err(|e| e.to_string())?;

        let mut manifest_opt: Option<AppManifest> = None;
        let mut vault_data_opt: Option<Vec<u8>> = None;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
            let name = file.name().to_string();

            // Validate against ZipSlip path traversal
            if name.contains("..") || name.starts_with('/') || name.starts_with('\\') {
                return Err(format!(
                    "Security violation: Invalid or traversing path detected in package: '{}'",
                    name
                ));
            }

            if name == "manifest.json" {
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).map_err(|e| e.to_string())?;
                let manifest: AppManifest = serde_json::from_slice(&buf)
                    .map_err(|e| format!("Failed to parse manifest.json: {}", e))?;
                manifest_opt = Some(manifest);

                // Also write manifest to destination
                let outpath = destination_dir.join("manifest.json");
                let mut outfile = File::create(outpath).map_err(|e| e.to_string())?;
                outfile.write_all(&buf).map_err(|e| e.to_string())?;
            } else if name == "data/encrypted_state.pvlt" {
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).map_err(|e| e.to_string())?;
                vault_data_opt = Some(buf);
            } else {
                // Extract code / asset file
                let clean_name = if name.starts_with("code/") {
                    name.strip_prefix("code/").unwrap_or(&name)
                } else {
                    &name
                };

                let clean_path = Path::new(clean_name);
                for component in clean_path.components() {
                    if let std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_) = component
                    {
                        return Err(format!(
                            "Security violation: Path traversal component in package entry '{}'",
                            name
                        ));
                    }
                }

                let outpath = destination_dir.join(clean_path);
                if file.is_dir() {
                    fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
                } else {
                    if let Some(p) = outpath.parent() {
                        fs::create_dir_all(p).map_err(|e| e.to_string())?;
                    }
                    let mut outfile = File::create(&outpath).map_err(|e| e.to_string())?;
                    std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
                }
            }
        }

        let manifest =
            manifest_opt.ok_or_else(|| "Package missing root manifest.json".to_string())?;
        Ok((manifest, vault_data_opt))
    }

    pub fn is_excluded(name: &str) -> bool {
        let n = name.to_lowercase();
        n == ".venv"
            || n == "venv"
            || n == "env"
            || n == "__pycache__"
            || n == ".pytest_cache"
            || n == ".git"
            || n == ".secrets"
            || n == "node_modules"
            || n == "temp"
            || n.ends_with(".pyc")
            || n.ends_with(".db")
            || n.ends_with(".sqlite")
            || n.ends_with(".sqlite3")
            || n.ends_with(".tmp")
            || n.ends_with(":zone.identifier")
            || n.ends_with(".zone.identifier")
    }

    fn add_directory_to_zip<W: Write + std::io::Seek>(
        zip: &mut ZipWriter<W>,
        current_dir: &Path,
        root_dir: &Path,
        options: SimpleFileOptions,
    ) -> Result<(), String> {
        for entry in fs::read_dir(current_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            if Self::is_excluded(&file_name) {
                continue;
            }

            let relative = path.strip_prefix(root_dir).map_err(|e| e.to_string())?;

            // Avoid duplicating root manifest.json
            if relative == Path::new("manifest.json") {
                continue;
            }

            let zip_path = format!("code/{}", relative.to_string_lossy().replace('\\', "/"));

            if path.is_dir() {
                zip.add_directory(&zip_path, options)
                    .map_err(|e| e.to_string())?;
                Self::add_directory_to_zip(zip, &path, root_dir, options)?;
            } else {
                zip.start_file(&zip_path, options)
                    .map_err(|e| e.to_string())?;
                let mut f = File::open(&path).map_err(|e| e.to_string())?;
                std::io::copy(&mut f, zip).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

/// Cryptographic Signature Validator for GitHub Releases & Packages
pub struct CryptoVerifier;

impl CryptoVerifier {
    /// Verify Ed25519 signature against data using public key
    pub fn verify_signature(
        data: &[u8],
        signature_base64: &str,
        public_key_base64: &str,
    ) -> Result<bool, String> {
        let pubkey_bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            public_key_base64.trim(),
        )
        .map_err(|e| format!("Invalid base64 public key: {}", e))?;

        let pubkey_array: [u8; 32] = pubkey_bytes
            .try_into()
            .map_err(|_| "Public key must be exactly 32 bytes".to_string())?;

        let verifying_key = VerifyingKey::from_bytes(&pubkey_array)
            .map_err(|e| format!("Invalid Ed25519 public key: {}", e))?;

        let sig_bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            signature_base64.trim(),
        )
        .map_err(|e| format!("Invalid base64 signature: {}", e))?;

        let sig_array: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| "Signature must be exactly 64 bytes".to_string())?;

        let signature = Signature::from_bytes(&sig_array);

        verifying_key
            .verify(data, &signature)
            .map(|_| true)
            .map_err(|e| format!("Ed25519 signature verification failed: {}", e))
    }
}
