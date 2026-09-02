use super::manifest::AppManifest;
use super::source::{AppSource, CryptoVerifier, PackageBundler};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledAppInfo {
    pub manifest: AppManifest,
    pub path: String,
    pub is_active: bool,
    pub source: AppSource,
    pub launch_url: String,
    pub is_composer: bool,
}

pub struct AppRegistry {
    bundled_apps_root: PathBuf,
    user_apps_root: Option<PathBuf>,
    installed_apps: RwLock<HashMap<String, InstalledAppInfo>>,
    active_app_id: RwLock<Option<String>>,
}

impl AppRegistry {
    pub fn new(bundled_apps_root: PathBuf) -> Self {
        Self::new_with_user_apps(bundled_apps_root, None)
    }

    pub fn new_with_user_apps(bundled_apps_root: PathBuf, user_apps_root: Option<PathBuf>) -> Self {
        let registry = Self {
            bundled_apps_root,
            user_apps_root,
            installed_apps: RwLock::new(HashMap::new()),
            active_app_id: RwLock::new(None),
        };
        registry.scan_installed_apps();
        registry
    }

    pub fn bundled_apps_root(&self) -> &Path {
        &self.bundled_apps_root
    }

    pub fn user_apps_root(&self) -> Option<&PathBuf> {
        self.user_apps_root.as_ref()
    }

    /// Scan bundled and user apps directories for valid application manifests
    pub fn scan_installed_apps(&self) {
        let mut apps_map = self.installed_apps.write().unwrap();
        apps_map.clear();

        // 1. Scan bundled apps directory
        Self::scan_directory_into_map(&self.bundled_apps_root, &mut apps_map);

        // 2. Scan user installed apps directory (if configured)
        if let Some(ref user_dir) = self.user_apps_root {
            if user_dir.exists() {
                Self::scan_directory_into_map(user_dir, &mut apps_map);
            }
        }
    }

    fn scan_directory_into_map(dir: &Path, apps_map: &mut HashMap<String, InstalledAppInfo>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let folder_name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    if folder_name.starts_with('.') {
                        continue;
                    }

                    let manifest_path = path.join("manifest.json");
                    if manifest_path.exists() {
                        if let Ok(mut manifest) = AppManifest::load_from_file(&manifest_path) {
                            let app_id = manifest.app_id.clone();
                            let source = manifest
                                .source
                                .clone()
                                .unwrap_or_else(|| AppSource::LocalDirectory(path.clone()));
                            let is_composer = manifest.has_composer();
                            let launch_url = manifest.resolve_launch_url(&folder_name, None);

                            // Inline icon file content to prevent webview relative path 404s
                            if let Some(ref mut pres) = manifest.presentation {
                                if let Some(ref icon_str) = pres.icon {
                                    let icon_path = path.join(icon_str);
                                    if icon_path.exists() {
                                        if icon_str.ends_with(".svg") {
                                            if let Ok(svg_text) = fs::read_to_string(&icon_path) {
                                                pres.icon = Some(svg_text);
                                            }
                                        } else if icon_str.ends_with(".png") {
                                            if let Ok(png_bytes) = fs::read(&icon_path) {
                                                use base64::prelude::*;
                                                let b64 = BASE64_STANDARD.encode(&png_bytes);
                                                pres.icon = Some(format!("data:image/png;base64,{}", b64));
                                            }
                                        }
                                    }
                                }
                            }

                            apps_map.insert(
                                app_id,
                                InstalledAppInfo {
                                    manifest,
                                    path: path.to_string_lossy().to_string(),
                                    is_active: false,
                                    source,
                                    launch_url,
                                    is_composer,
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    /// List all discovered installed apps
    pub fn list_apps(&self) -> Vec<InstalledAppInfo> {
        let apps_map = self.installed_apps.read().unwrap();
        let active = self.active_app_id.read().unwrap();

        apps_map
            .values()
            .map(|info| {
                let mut app = info.clone();
                app.is_active = active.as_deref() == Some(&app.manifest.app_id);
                app
            })
            .collect()
    }

    /// Get specific app by ID
    pub fn get_app(&self, app_id: &str) -> Option<InstalledAppInfo> {
        let apps_map = self.installed_apps.read().unwrap();
        apps_map.get(app_id).cloned()
    }

    /// Get active running application ID
    pub fn get_active_app_id(&self) -> Option<String> {
        let active = self.active_app_id.read().unwrap();
        active.clone()
    }

    /// Set active running application ID
    pub fn set_active_app(&self, app_id: Option<String>) {
        let mut active = self.active_app_id.write().unwrap();
        *active = app_id;
    }

    /// Register an arbitrary local directory (e.g. from USB / local filesystem)
    pub fn install_local_directory(&self, dir_path: PathBuf) -> Result<InstalledAppInfo, String> {
        let manifest_path = dir_path.join("manifest.json");
        if !manifest_path.exists() {
            return Err(format!("No manifest.json found at {}", dir_path.display()));
        }

        let mut manifest = AppManifest::load_from_file(&manifest_path)
            .map_err(|e| format!("Invalid manifest: {}", e))?;

        // Pre-mutation compatibility check before copying or mounting
        let compat = super::compatibility::CompatibilityChecker::check(&manifest);
        if !compat.is_compatible {
            return Err(format!(
                "Incompatible application '{}' (status: {:?}): {}",
                manifest.app_id,
                compat.status,
                compat.reasons.join("; ")
            ));
        }

        let source = AppSource::LocalDirectory(dir_path.clone());
        manifest.source = Some(source.clone());

        // Copy imported local application into managed user apps directory if configured
        let effective_path = if let Some(ref user_apps) = self.user_apps_root {
            let target = user_apps.join(&manifest.app_id);
            if dir_path != target && !dir_path.starts_with(&self.bundled_apps_root) {
                let _ = Self::copy_dir_recursive(&dir_path, &target);
                target
            } else {
                dir_path.clone()
            }
        } else {
            dir_path.clone()
        };

        let folder_name = effective_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let is_composer = manifest.has_composer();
        let launch_url = manifest.resolve_launch_url(&folder_name, None);

        let app_info = InstalledAppInfo {
            manifest: manifest.clone(),
            path: effective_path.to_string_lossy().to_string(),
            is_active: false,
            source,
            launch_url,
            is_composer,
        };

        let mut apps_map = self.installed_apps.write().unwrap();
        apps_map.insert(manifest.app_id.clone(), app_info.clone());

        Ok(app_info)
    }

    fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            let path = entry.path();
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if PackageBundler::is_excluded(&name_str) {
                continue;
            }
            if ft.is_dir() {
                Self::copy_dir_recursive(&path, &dst.join(file_name))?;
            } else {
                fs::copy(&path, dst.join(file_name))?;
            }
        }
        Ok(())
    }

    /// Install / mount an all-in-one .pvpkg portable package file
    pub fn install_package_bundle(
        &self,
        package_file: &Path,
        install_target_dir: &Path,
    ) -> Result<(InstalledAppInfo, Option<Vec<u8>>), String> {
        let (mut manifest, vault_data) =
            PackageBundler::extract_package(package_file, install_target_dir)?;

        let source = AppSource::PortablePackage(package_file.to_path_buf());
        manifest.source = Some(source.clone());
        let folder_name = install_target_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let is_composer = manifest.has_composer();
        let launch_url = manifest.resolve_launch_url(&folder_name, None);

        let app_info = InstalledAppInfo {
            manifest: manifest.clone(),
            path: install_target_dir.to_string_lossy().to_string(),
            is_active: false,
            source,
            launch_url,
            is_composer,
        };

        let mut apps_map = self.installed_apps.write().unwrap();
        apps_map.insert(manifest.app_id.clone(), app_info.clone());

        Ok((app_info, vault_data))
    }

    /// Export an app to an all-in-one .pvpkg portable package file
    pub fn export_app_bundle(
        &self,
        app_id: &str,
        vault_data: Option<&[u8]>,
        output_file: &Path,
    ) -> Result<(), String> {
        let apps_map = self.installed_apps.read().unwrap();
        let app = apps_map
            .get(app_id)
            .ok_or_else(|| format!("App '{}' not found", app_id))?;
        let app_dir = PathBuf::from(&app.path);

        PackageBundler::export_package(&app_dir, vault_data, output_file)
    }

    /// Install or register a GitHub release target with downloading, verification, and extraction
    pub fn install_github_target(
        &self,
        repo: &str,
        tag: Option<&str>,
        public_key: Option<&str>,
        install_dir: &Path,
    ) -> Result<InstalledAppInfo, String> {
        let target_tag = tag.unwrap_or("latest").to_string();
        let manifest_path = install_dir.join("manifest.json");

        if !manifest_path.exists() {
            fs::create_dir_all(install_dir)
                .map_err(|e| format!("Failed to create install dir: {}", e))?;

            // Attempt to download release from GitHub API
            let client = reqwest::blocking::Client::builder()
                .user_agent("PixieVault-AppManager/1.0")
                .build()
                .map_err(|e| format!("HTTP client error: {}", e))?;

            let api_url = if target_tag == "latest" {
                format!("https://api.github.com/repos/{}/releases/latest", repo)
            } else {
                format!(
                    "https://api.github.com/repos/{}/releases/tags/{}",
                    repo, target_tag
                )
            };

            let release_resp = client.get(&api_url).send();
            let mut installed_from_download = false;

            if let Ok(resp) = release_resp {
                if resp.status().is_success() {
                    if let Ok(release_json) = resp.json::<serde_json::Value>() {
                        if let Some(assets) = release_json["assets"].as_array() {
                            // Find .pvpkg asset
                            let pkg_asset = assets.iter().find(|a| {
                                a["name"]
                                    .as_str()
                                    .map(|n| n.ends_with(".pvpkg"))
                                    .unwrap_or(false)
                            });

                            if let Some(asset) = pkg_asset {
                                if let Some(download_url) = asset["browser_download_url"].as_str() {
                                    let pkg_bytes = client
                                        .get(download_url)
                                        .send()
                                        .map_err(|e| format!("Failed to download package: {}", e))?
                                        .bytes()
                                        .map_err(|e| {
                                            format!("Failed to read package body: {}", e)
                                        })?;

                                    // Verify signature if public key provided and signature asset exists
                                    if let Some(pubkey) = public_key {
                                        let sig_asset = assets.iter().find(|a| {
                                            a["name"]
                                                .as_str()
                                                .map(|n| {
                                                    n.ends_with(".pvpkg.sig") || n.ends_with(".sig")
                                                })
                                                .unwrap_or(false)
                                        });

                                        if let Some(sig_a) = sig_asset {
                                            if let Some(sig_url) =
                                                sig_a["browser_download_url"].as_str()
                                            {
                                                let sig_str = client
                                                    .get(sig_url)
                                                    .send()
                                                    .map_err(|e| {
                                                        format!(
                                                            "Failed to download signature: {}",
                                                            e
                                                        )
                                                    })?
                                                    .text()
                                                    .map_err(|e| {
                                                        format!("Failed to read signature: {}", e)
                                                    })?;

                                                CryptoVerifier::verify_signature(
                                                    &pkg_bytes, &sig_str, pubkey,
                                                )?;
                                            }
                                        }
                                    }

                                    // Extract into install_dir
                                    let temp_pkg_path = install_dir.join("downloaded.pvpkg");
                                    fs::write(&temp_pkg_path, &pkg_bytes)
                                        .map_err(|e| e.to_string())?;
                                    let (extracted_manifest, _) = PackageBundler::extract_package(
                                        &temp_pkg_path,
                                        install_dir,
                                    )?;
                                    let _ = fs::remove_file(&temp_pkg_path);

                                    let mut manifest = extracted_manifest;
                                    let source = AppSource::GitHubRelease {
                                        repository: repo.to_string(),
                                        tag: target_tag.clone(),
                                        public_key: public_key.map(|s| s.to_string()),
                                        last_checked: Some(chrono::Utc::now().to_rfc3339()),
                                    };
                                    manifest.source = Some(source.clone());
                                    let folder_name = install_dir
                                        .file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .to_string();
                                    let is_composer = manifest.has_composer();
                                    let launch_url =
                                        manifest.resolve_launch_url(&folder_name, None);

                                    let app_info = InstalledAppInfo {
                                        manifest: manifest.clone(),
                                        path: install_dir.to_string_lossy().to_string(),
                                        is_active: false,
                                        source,
                                        launch_url,
                                        is_composer,
                                    };

                                    let mut apps_map = self.installed_apps.write().unwrap();
                                    apps_map.insert(manifest.app_id.clone(), app_info.clone());
                                    installed_from_download = true;
                                }
                            }
                        }
                    }
                }
            }

            if !installed_from_download && !manifest_path.exists() {
                return Err(format!(
                    "Failed to download release asset from GitHub repository '{}/{}'",
                    repo, target_tag
                ));
            }
        }

        let mut manifest = AppManifest::load_from_file(&manifest_path)
            .map_err(|e| format!("Invalid manifest: {}", e))?;

        let source = AppSource::GitHubRelease {
            repository: repo.to_string(),
            tag: target_tag,
            public_key: public_key.map(|s| s.to_string()),
            last_checked: Some(chrono::Utc::now().to_rfc3339()),
        };
        manifest.source = Some(source.clone());
        let folder_name = install_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let is_composer = manifest.has_composer();
        let launch_url = manifest.resolve_launch_url(&folder_name, None);

        let app_info = InstalledAppInfo {
            manifest: manifest.clone(),
            path: install_dir.to_string_lossy().to_string(),
            is_active: false,
            source,
            launch_url,
            is_composer,
        };

        let mut apps_map = self.installed_apps.write().unwrap();
        apps_map.insert(manifest.app_id.clone(), app_info.clone());

        Ok(app_info)
    }
}
