use super::source::AppSource;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ManifestError {
    #[error("Failed to read manifest file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid manifest JSON schema: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("Missing required field: {0}")]
    MissingField(String),
    #[error("Invalid version format: {0}")]
    InvalidVersion(String),
    #[error("Incompatible host version: required {required}, but host is {host}")]
    IncompatibleHostVersion { required: String, host: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeCompatibility {
    #[serde(default = "default_true")]
    pub supports_dark_mode: bool,
    #[serde(default = "default_true")]
    pub supports_light_mode: bool,
    pub custom_accent_override: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppPermissions {
    #[serde(default)]
    pub requested_read: Vec<String>,
    #[serde(default)]
    pub requested_write: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthcheckConfig {
    #[serde(default = "default_healthcheck_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_interval_ms")]
    pub interval_ms: u64,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub expected_status: Option<u16>,
    #[serde(default)]
    pub expected_body: Option<String>,
}

fn default_healthcheck_endpoint() -> String {
    "/".to_string()
}
fn default_interval_ms() -> u64 {
    200
}
fn default_timeout_ms() -> u64 {
    8000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub writable_dirs: Vec<String>,
    #[serde(default = "default_true")]
    pub isolate_network_loopback: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            writable_dirs: Vec::new(),
            isolate_network_loopback: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PresentationConfig {
    pub icon: Option<String>,
    pub accent: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(rename = "type", default = "default_runtime_type")]
    pub runtime_type: String, // "python", "node", "binary", "custom"
    pub requirements: Option<String>,
    pub install_command: Option<Vec<String>>,
    pub fingerprint_files: Option<Vec<String>>,
}

fn default_runtime_type() -> String {
    "binary".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub command: Vec<String>,
    pub working_dir: Option<String>,
    #[serde(default = "default_auto_port")]
    pub port: String, // "auto" or explicit numeric port e.g. "5000"
    #[serde(default)]
    pub environment: HashMap<String, String>,
    pub healthcheck: Option<HealthcheckConfig>,
    pub runtime: Option<RuntimeConfig>,
    #[serde(default)]
    pub requirements: Option<String>, // Legacy / shorthand path to requirements.txt
    #[serde(default)]
    pub auto_install: Option<bool>, // Whether to auto-install dependencies (defaults to true)
    pub sandbox: Option<SandboxConfig>, // Optional Sandbox policy configuration
}

impl ServiceConfig {
    pub fn get_runtime(&self) -> RuntimeConfig {
        if let Some(ref r) = self.runtime {
            return r.clone();
        }
        let first_cmd = self
            .command
            .first()
            .map(|b| b.to_lowercase())
            .unwrap_or_default();
        let is_py = first_cmd == "python"
            || first_cmd == "python3"
            || first_cmd == "py"
            || first_cmd == "python.exe"
            || first_cmd.ends_with(".py");
        let is_node = first_cmd == "node"
            || first_cmd == "nodejs"
            || first_cmd == "node.exe"
            || first_cmd == "npm"
            || first_cmd == "npm.cmd"
            || first_cmd == "npx"
            || first_cmd.ends_with(".js")
            || first_cmd.ends_with(".mjs");

        if is_py || self.requirements.is_some() {
            RuntimeConfig {
                runtime_type: "python".to_string(),
                requirements: self.requirements.clone(),
                install_command: None,
                fingerprint_files: self.requirements.as_ref().map(|r| vec![r.clone()]),
            }
        } else if is_node {
            RuntimeConfig {
                runtime_type: "node".to_string(),
                requirements: None,
                install_command: None,
                fingerprint_files: Some(vec![
                    "package.json".to_string(),
                    "package-lock.json".to_string(),
                ]),
            }
        } else {
            RuntimeConfig {
                runtime_type: "binary".to_string(),
                requirements: None,
                install_command: None,
                fingerprint_files: None,
            }
        }
    }
}

fn default_auto_port() -> String {
    "auto".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposerConfig {
    #[serde(default = "default_composer_version")]
    pub version: String,
    #[serde(default)]
    pub services: HashMap<String, ServiceConfig>,
}

fn default_composer_version() -> String {
    "1".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppManifest {
    pub app_id: String,
    pub name: String,
    pub version: String,
    pub min_pixievault_version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_entrypoint")]
    pub entrypoint: String,
    pub author: Option<String>,
    pub presentation: Option<PresentationConfig>,
    #[serde(default)]
    pub permissions: AppPermissions,
    pub theme_compatibility: Option<ThemeCompatibility>,
    #[serde(default)]
    pub required_capabilities: Vec<String>,

    // Declarative Native Composer Configuration (Optional)
    pub composer: Option<ComposerConfig>,

    // Distribution & Verification metadata
    pub source: Option<AppSource>,
    pub signature: Option<String>,
    pub public_key: Option<String>,
}

fn default_entrypoint() -> String {
    "index.html".to_string()
}

impl AppManifest {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ManifestError> {
        let content = fs::read_to_string(path)?;
        let manifest: AppManifest = serde_json::from_str(&content)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.app_id.trim().is_empty() {
            return Err(ManifestError::MissingField("app_id".to_string()));
        }
        if self.name.trim().is_empty() {
            return Err(ManifestError::MissingField("name".to_string()));
        }
        if self.version.trim().is_empty() {
            return Err(ManifestError::MissingField("version".to_string()));
        }
        if self.min_pixievault_version.trim().is_empty() {
            return Err(ManifestError::MissingField("min_pixievault_version".to_string()));
        }
        semver::Version::parse(self.min_pixievault_version.trim()).map_err(|e| {
            ManifestError::InvalidVersion(format!(
                "Invalid 'min_pixievault_version' '{}': {}",
                self.min_pixievault_version, e
            ))
        })?;
        semver::Version::parse(self.version.trim()).map_err(|e| {
            ManifestError::InvalidVersion(format!(
                "Invalid 'version' '{}': {}",
                self.version, e
            ))
        })?;
        Ok(())
    }

    pub fn is_compatible_with_host(&self, host_version: &str) -> Result<bool, ManifestError> {
        self.validate()?;
        let req_ver = semver::Version::parse(self.min_pixievault_version.trim()).map_err(|e| {
            ManifestError::InvalidVersion(format!(
                "Invalid 'min_pixievault_version' '{}': {}",
                self.min_pixievault_version, e
            ))
        })?;
        let host_ver = semver::Version::parse(host_version.trim()).map_err(|e| {
            ManifestError::InvalidVersion(format!(
                "Invalid host version '{}': {}",
                host_version, e
            ))
        })?;

        if req_ver > host_ver {
            return Err(ManifestError::IncompatibleHostVersion {
                required: self.min_pixievault_version.clone(),
                host: host_version.to_string(),
            });
        }
        Ok(true)
    }

    pub fn has_composer(&self) -> bool {
        self.composer
            .as_ref()
            .map(|c| !c.services.is_empty())
            .unwrap_or(false)
    }

    pub fn resolve_entrypoint(&self, port_map: &HashMap<String, u16>) -> String {
        let mut entry = self.entrypoint.clone();
        for (svc_name, port) in port_map {
            let var1 = format!("{{{{services.{}.port}}}}", svc_name);
            let var2 = format!("{{{{port}}}}");
            entry = entry.replace(&var1, &port.to_string());
            entry = entry.replace(&var2, &port.to_string());
        }
        entry
    }

    pub fn resolve_launch_url(
        &self,
        folder_name: &str,
        port_map: Option<&HashMap<String, u16>>,
    ) -> String {
        if self.has_composer() {
            if let Some(ports) = port_map {
                self.resolve_entrypoint(ports)
            } else {
                self.entrypoint.clone()
            }
        } else if self.entrypoint.starts_with("http://") || self.entrypoint.starts_with("https://")
        {
            self.entrypoint.clone()
        } else {
            format!("../{}/{}", folder_name, self.entrypoint)
        }
    }

    /// Centralized resolver for a Composer service's canonical working directory
    pub fn resolve_service_working_dir(
        &self,
        app_root: &Path,
        service_name: &str,
    ) -> Result<PathBuf, String> {
        let composer = self
            .composer
            .as_ref()
            .ok_or_else(|| "Manifest does not contain composer config".to_string())?;
        let svc = composer
            .services
            .get(service_name)
            .ok_or_else(|| format!("Service '{}' not found in manifest", service_name))?;

        let working_dir = if let Some(ref wd) = svc.working_dir {
            app_root.join(wd)
        } else {
            app_root.to_path_buf()
        };

        if !working_dir.exists() {
            return Err(format!(
                "Service '{}' working directory does not exist at {}",
                service_name,
                working_dir.display()
            ));
        }

        Ok(super::canonicalize_clean(&working_dir))
    }


    pub fn can_read_metric(&self, target_app_id: &str, metric_name: &str) -> bool {
        let pattern = format!("{}:{}", target_app_id, metric_name);
        let wildcard = format!("{}:*", target_app_id);
        self.permissions
            .requested_read
            .iter()
            .any(|r| r == &pattern || r == &wildcard || r == "*:*" || r == metric_name)
    }
}
