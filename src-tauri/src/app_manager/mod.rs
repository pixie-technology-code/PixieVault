use std::path::{Path, PathBuf};

pub mod bus;
pub mod manifest;
pub mod provisioning;
pub mod registry;
pub mod sandbox;
pub mod sidecar;
pub mod source;

/// Clean verbatim extended-length Windows prefixes (`\\?\` and `\\?\UNC\`) to standard paths
pub fn clean_path(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{}", stripped))
    } else if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        p.to_path_buf()
    }
}

/// Canonicalize a path while ensuring extended Windows verbatim prefixes are safely normalized
pub fn canonicalize_clean(p: &Path) -> PathBuf {
    let canonical = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    clean_path(&canonical)
}

pub use bus::{InterAppBus, MetricValue};
pub use manifest::{
    AppManifest, AppPermissions, ComposerConfig, HealthcheckConfig, ManifestError,
    PresentationConfig, RuntimeConfig, SandboxConfig, ServiceConfig, ThemeCompatibility,
};
pub use provisioning::{
    ProvisioningDiagnostic, PythonError, PythonProvisioningManager, RuntimeProvisioner,
};
pub use registry::{AppRegistry, InstalledAppInfo};
pub use sandbox::{SandboxEngine, SandboxManager, SandboxPolicy};
pub use sidecar::{
    allocate_ephemeral_port, ComposerAppStatus, ServiceRuntimeStatus, SidecarManager,
    SidecarStatus, VaultComposer,
};
pub use source::{AppSource, CryptoVerifier, PackageBundler};

