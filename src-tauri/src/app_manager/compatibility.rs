use super::manifest::AppManifest;
use serde::{Deserialize, Serialize};

/// Canonical PixieVault host version from crate metadata
pub const CURRENT_HOST_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityStatus {
    Compatible,
    IncompatibleVersion,
    MissingCapabilities,
    InvalidManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityReport {
    pub is_compatible: bool,
    pub status: CompatibilityStatus,
    pub app_id: String,
    pub app_name: String,
    pub app_version: String,
    pub min_pixievault_version: String,
    pub host_version: String,
    pub missing_capabilities: Vec<String>,
    pub reasons: Vec<String>,
}

pub struct CompatibilityChecker;

impl CompatibilityChecker {
    /// Known and implemented platform capabilities supported by this host
    pub const KNOWN_CAPABILITIES: &'static [&'static str] = &[
        "runtime.python",
        "runtime.node",
        "runtime.binary",
        "runtime.custom",
        "storage.volume.v1",
        "storage.vault.v3",
        "broker.file.import",
        "broker.file.export",
        "broker.file.download",
        "sandbox.loopback",
        "permission.telemetry",
    ];

    /// Check compatibility of an AppManifest against the current host version and capabilities
    pub fn check(manifest: &AppManifest) -> CompatibilityReport {
        let host_ver_str = CURRENT_HOST_VERSION;
        let mut reasons = Vec::new();
        let mut missing_caps = Vec::new();

        // 1. Validate manifest self-consistency (required fields and valid SemVer)
        if let Err(err) = manifest.validate() {
            reasons.push(err.to_string());
            return CompatibilityReport {
                is_compatible: false,
                status: CompatibilityStatus::InvalidManifest,
                app_id: manifest.app_id.clone(),
                app_name: manifest.name.clone(),
                app_version: manifest.version.clone(),
                min_pixievault_version: manifest.min_pixievault_version.clone(),
                host_version: host_ver_str.to_string(),
                missing_capabilities: Vec::new(),
                reasons,
            };
        }

        // 2. Validate SemVer compatibility
        let mut version_compatible = true;
        match (
            semver::Version::parse(manifest.min_pixievault_version.trim()),
            semver::Version::parse(host_ver_str.trim()),
        ) {
            (Ok(req), Ok(host)) => {
                if req > host {
                    version_compatible = false;
                    reasons.push(format!(
                        "Host version {} is older than required minimum PixieVault version {}",
                        host, req
                    ));
                }
            }
            (Err(e), _) => {
                reasons.push(format!("Malformed min_pixievault_version: {}", e));
                version_compatible = false;
            }
            (_, Err(e)) => {
                reasons.push(format!("Malformed host version: {}", e));
                version_compatible = false;
            }
        }

        // 3. Validate declared capabilities against known host capabilities
        for cap in &manifest.required_capabilities {
            if !Self::KNOWN_CAPABILITIES.contains(&cap.as_str()) {
                missing_caps.push(cap.clone());
                reasons.push(format!("Host does not support required capability '{}'", cap));
            }
        }

        let is_compatible = version_compatible && missing_caps.is_empty();
        let status = if !version_compatible {
            CompatibilityStatus::IncompatibleVersion
        } else if !missing_caps.is_empty() {
            CompatibilityStatus::MissingCapabilities
        } else {
            CompatibilityStatus::Compatible
        };

        CompatibilityReport {
            is_compatible,
            status,
            app_id: manifest.app_id.clone(),
            app_name: manifest.name.clone(),
            app_version: manifest.version.clone(),
            min_pixievault_version: manifest.min_pixievault_version.clone(),
            host_version: host_ver_str.to_string(),
            missing_capabilities: missing_caps,
            reasons,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_manifest(min_ver: &str, caps: Vec<&str>) -> AppManifest {
        AppManifest {
            app_id: "test_compat_app".to_string(),
            name: "Test Compatibility App".to_string(),
            version: "1.0.0".to_string(),
            min_pixievault_version: min_ver.to_string(),
            description: "Test".to_string(),
            entrypoint: "index.html".to_string(),
            author: None,
            presentation: None,
            permissions: Default::default(),
            theme_compatibility: None,
            required_capabilities: caps.into_iter().map(|s| s.to_string()).collect(),
            composer: None,
            source: None,
            signature: None,
            public_key: None,
        }
    }

    #[test]
    fn test_compatible_manifest() {
        let manifest = create_test_manifest("0.2.0", vec!["runtime.python", "storage.vault.v3"]);
        let report = CompatibilityChecker::check(&manifest);
        assert!(report.is_compatible);
        assert_eq!(report.status, CompatibilityStatus::Compatible);
        assert!(report.missing_capabilities.is_empty());
        assert!(report.reasons.is_empty());
    }

    #[test]
    fn test_older_required_version_is_compatible() {
        let manifest = create_test_manifest("0.1.0", vec![]);
        let report = CompatibilityChecker::check(&manifest);
        assert!(report.is_compatible);
        assert_eq!(report.status, CompatibilityStatus::Compatible);
    }

    #[test]
    fn test_future_version_incompatible() {
        let manifest = create_test_manifest("99.0.0", vec![]);
        let report = CompatibilityChecker::check(&manifest);
        assert!(!report.is_compatible);
        assert_eq!(report.status, CompatibilityStatus::IncompatibleVersion);
        assert!(report.reasons.iter().any(|r| r.contains("is older than")));
    }

    #[test]
    fn test_unknown_capability_incompatible() {
        let manifest = create_test_manifest("0.2.0", vec!["quantum_hyperdrive_v9"]);
        let report = CompatibilityChecker::check(&manifest);
        assert!(!report.is_compatible);
        assert_eq!(report.status, CompatibilityStatus::MissingCapabilities);
        assert_eq!(report.missing_capabilities, vec!["quantum_hyperdrive_v9"]);
    }

    #[test]
    fn test_malformed_version_rejected() {
        let manifest = create_test_manifest("v0.2.0", vec![]); // Leading 'v' is invalid in strict SemVer
        let report = CompatibilityChecker::check(&manifest);
        assert!(!report.is_compatible);
        assert_eq!(report.status, CompatibilityStatus::InvalidManifest);
    }
}
