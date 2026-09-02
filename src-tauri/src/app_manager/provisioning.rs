use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "details")]
pub enum PythonError {
    MissingPython(String),
    VenvCreationFailed {
        details: String,
        logs: String,
    },
    DependencyInstallFailed {
        req_file: String,
        details: String,
        logs: String,
    },
    MissingEntrypoint {
        path: String,
    },
    IncompatibleEnvironment(String),
}

impl std::fmt::Display for PythonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingPython(msg) => write!(f, "System Python not found: {}", msg),
            Self::VenvCreationFailed { details, logs } => {
                write!(
                    f,
                    "Failed to create virtualenv: {}\nOutput:\n{}",
                    details, logs
                )
            }
            Self::DependencyInstallFailed {
                req_file,
                details,
                logs,
            } => {
                write!(
                    f,
                    "Dependency installation failed for '{}': {}\nOutput:\n{}",
                    req_file, details, logs
                )
            }
            Self::MissingEntrypoint { path } => write!(f, "Entrypoint not found: {}", path),
            Self::IncompatibleEnvironment(msg) => write!(f, "Incompatible environment: {}", msg),
        }
    }
}

pub struct PythonProvisioningManager;

impl PythonProvisioningManager {
    /// Compute SHA-256 fingerprint of requirements.txt
    pub fn compute_fingerprint(req_file: &Path) -> Result<String, String> {
        if !req_file.exists() {
            return Ok("no_requirements".to_string());
        }
        let content =
            fs::read(req_file).map_err(|e| format!("Failed to read requirements file: {}", e))?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Resolve virtualenv python binary path
    pub fn get_venv_python_path(venv_dir: &Path) -> PathBuf {
        if cfg!(target_os = "windows") {
            venv_dir.join("Scripts").join("python.exe")
        } else {
            venv_dir.join("bin").join("python")
        }
    }

    /// Resolve virtualenv pip binary path
    pub fn get_venv_pip_path(venv_dir: &Path) -> PathBuf {
        if cfg!(target_os = "windows") {
            venv_dir.join("Scripts").join("pip.exe")
        } else {
            venv_dir.join("bin").join("pip")
        }
    }

    /// Fast, deterministic readiness check (<10ms) without running pip
    pub fn verify_environment_ready(
        working_dir: &Path,
        req_filename_opt: Option<&str>,
    ) -> Result<PathBuf, PythonError> {
        Self::verify_environment_ready_in(working_dir, &working_dir.join(".venv"), req_filename_opt)
    }

    pub fn verify_environment_ready_in(
        working_dir: &Path,
        venv_dir: &Path,
        req_filename_opt: Option<&str>,
    ) -> Result<PathBuf, PythonError> {
        let canonical_dir = super::canonicalize_clean(working_dir);
        let venv_py = super::clean_path(&Self::get_venv_python_path(&venv_dir));

        let req_file = if let Some(rf) = req_filename_opt {
            canonical_dir.join(rf)
        } else {
            canonical_dir.join("requirements.txt")
        };

        if !req_file.exists() {
            // No requirements specified, return venv if exists or system python
            if venv_py.exists() {
                return Ok(venv_py);
            }
            if let Some(sys_py) = Self::find_system_python() {
                return Ok(PathBuf::from(sys_py));
            }
            return Err(PythonError::MissingPython(
                "No Python interpreter found".into(),
            ));
        }

        if !venv_dir.exists() || !venv_py.exists() {
            return Err(PythonError::DependencyInstallFailed {
                req_file: req_file.to_string_lossy().to_string(),
                details: "Python virtual environment (.venv) is not created. Please provision dependencies.".into(),
                logs: String::new(),
            });
        }

        let expected_fingerprint = Self::compute_fingerprint(&req_file)
            .map_err(|e| PythonError::IncompatibleEnvironment(e))?;

        let fingerprint_file = venv_dir.join(".deps_fingerprint");
        if !fingerprint_file.exists() {
            return Err(PythonError::DependencyInstallFailed {
                req_file: req_file.to_string_lossy().to_string(),
                details: "Dependencies have not been provisioned into .venv. Please run environment provisioning.".into(),
                logs: String::new(),
            });
        }

        let recorded_fingerprint = fs::read_to_string(&fingerprint_file)
            .unwrap_or_default()
            .trim()
            .to_string();
        if recorded_fingerprint != expected_fingerprint {
            return Err(PythonError::DependencyInstallFailed {
                req_file: req_file.to_string_lossy().to_string(),
                details: format!(
                    "Requirements file has changed since last provisioning (hash mismatch). Please repair environment."
                ),
                logs: String::new(),
            });
        }

        Ok(venv_py)
    }

    /// Explicit, observable Python environment provisioning
    pub fn provision_environment(
        working_dir: &Path,
        req_filename_opt: Option<&str>,
        force: bool,
    ) -> Result<PathBuf, PythonError> {
        Self::provision_environment_in(
            working_dir,
            &working_dir.join(".venv"),
            req_filename_opt,
            force,
        )
    }

    pub fn provision_environment_in(
        working_dir: &Path,
        venv_dir: &Path,
        req_filename_opt: Option<&str>,
        force: bool,
    ) -> Result<PathBuf, PythonError> {
        let canonical_dir = super::canonicalize_clean(working_dir);
        let venv_dir = super::clean_path(venv_dir);
        let venv_py = super::clean_path(&Self::get_venv_python_path(&venv_dir));
        let venv_pip = super::clean_path(&Self::get_venv_pip_path(&venv_dir));

        let req_file = if let Some(rf) = req_filename_opt {
            canonical_dir.join(rf)
        } else {
            canonical_dir.join("requirements.txt")
        };

        let sys_py = Self::find_system_python().ok_or_else(|| {
            PythonError::MissingPython(
                "No Python interpreter (python3/python/py) found on PATH".into(),
            )
        })?;

        // 1. Create .venv if needed or forced
        if force && venv_dir.exists() {
            fs::remove_dir_all(&venv_dir).map_err(|e| PythonError::VenvCreationFailed {
                details: format!(
                    "Failed to replace runtime directory '{}': {}",
                    venv_dir.display(),
                    e
                ),
                logs: String::new(),
            })?;
        }
        if !venv_dir.exists() || !venv_py.exists() {
            if let Some(parent) = venv_dir.parent() {
                fs::create_dir_all(parent).map_err(|e| PythonError::VenvCreationFailed {
                    details: format!(
                        "Failed to create runtime parent '{}': {}",
                        parent.display(),
                        e
                    ),
                    logs: String::new(),
                })?;
            }
            println!(
                "[PythonProvisioning] Creating virtualenv at {:?} using {}",
                venv_dir, sys_py
            );
            let mut venv_output = Command::new(&sys_py)
                .args(["-m", "venv", "--system-site-packages"])
                .arg(&venv_dir)
                .env("PYTHONIOENCODING", "utf-8")
                .env("PYTHONUTF8", "1")
                .env("PYTHONDONTWRITEBYTECODE", "1")
                .current_dir(&canonical_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .map_err(|e| PythonError::VenvCreationFailed {
                    details: format!("Failed to spawn python venv: {}", e),
                    logs: String::new(),
                })?;

            if !venv_output.status.success() {
                // Try with --without-pip if ensurepip is unavailable on debian/ubuntu
                let fallback = Command::new(&sys_py)
                    .args(["-m", "venv", "--without-pip", "--system-site-packages"])
                    .arg(&venv_dir)
                    .env("PYTHONIOENCODING", "utf-8")
                    .env("PYTHONUTF8", "1")
                    .env("PYTHONDONTWRITEBYTECODE", "1")
                    .current_dir(&canonical_dir)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output();



                if let Ok(fallback_out) = fallback {
                    if fallback_out.status.success() {
                        venv_output = fallback_out;
                    }
                }
            }

            if !venv_output.status.success() {
                let err_logs = format!(
                    "STDOUT:\n{}\nSTDERR:\n{}",
                    String::from_utf8_lossy(&venv_output.stdout),
                    String::from_utf8_lossy(&venv_output.stderr)
                );
                return Err(PythonError::VenvCreationFailed {
                    details: format!("python -m venv exited with status: {}", venv_output.status),
                    logs: err_logs,
                });
            }
        }

        // 2. Install dependencies if requirements.txt exists
        if req_file.exists() {
            println!(
                "[PythonProvisioning] Installing dependencies from {:?}...",
                req_file
            );
            let pip_output = if venv_pip.exists() {
                Command::new(&venv_pip)
                    .args(["install", "-r", req_file.to_string_lossy().as_ref()])
                    .env("PYTHONIOENCODING", "utf-8")
                    .env("PYTHONUTF8", "1")
                    .current_dir(&canonical_dir)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
            } else {
                Command::new(&venv_py)
                    .args([
                        "-m",
                        "pip",
                        "install",
                        "-r",
                        req_file.to_string_lossy().as_ref(),
                    ])
                    .env("PYTHONIOENCODING", "utf-8")
                    .env("PYTHONUTF8", "1")
                    .current_dir(&canonical_dir)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
            };

            let pip_output = match pip_output {
                Ok(out) => out,
                Err(e) => {
                    // Fallback to system python -m pip if venv pip failed to spawn
                    Command::new(&sys_py)
                        .args([
                            "-m",
                            "pip",
                            "install",
                            "-r",
                            req_file.to_string_lossy().as_ref(),
                        ])
                        .env("PYTHONIOENCODING", "utf-8")
                        .env("PYTHONUTF8", "1")
                        .current_dir(&canonical_dir)
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .output()
                        .map_err(|e2| PythonError::DependencyInstallFailed {
                            req_file: req_file.to_string_lossy().to_string(),
                            details: format!("Failed to spawn pip (venv: {}, system: {})", e, e2),
                            logs: String::new(),
                        })?
                }
            };


            let logs = format!(
                "STDOUT:\n{}\nSTDERR:\n{}",
                String::from_utf8_lossy(&pip_output.stdout),
                String::from_utf8_lossy(&pip_output.stderr)
            );

            if !pip_output.status.success() {
                return Err(PythonError::DependencyInstallFailed {
                    req_file: req_file.to_string_lossy().to_string(),
                    details: format!(
                        "pip install returned non-zero status: {}",
                        pip_output.status
                    ),
                    logs,
                });
            }

            // Write fingerprint
            let fingerprint = Self::compute_fingerprint(&req_file)
                .map_err(|e| PythonError::IncompatibleEnvironment(e))?;
            let fingerprint_file = venv_dir.join(".deps_fingerprint");
            let _ = fs::write(&fingerprint_file, fingerprint);
        }

        Ok(venv_py)
    }

    /// System Python discovery with executable validation
    pub fn find_system_python() -> Option<String> {
        let candidates = if cfg!(target_os = "windows") {
            vec!["py", "python", "python3"]
        } else {
            vec!["python3", "python"]
        };

        for cmd in candidates {
            if Self::which_exists(cmd) {
                // Verify candidate actually executes and isn't a dead stub / Microsoft Store alias
                let test_run = Command::new(cmd)
                    .args(["--version"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output();
                if let Ok(out) = test_run {
                    if out.status.success() {
                        return Some(cmd.to_string());
                    }
                }
            }
        }
        None
    }

    pub fn which_exists(cmd: &str) -> bool {
        RuntimeProvisioner::which_exists(cmd)
    }
}

/// Generic Runtime Provisioning Diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisioningDiagnostic {
    pub code: String, // e.g. "ready", "runtime_not_provisioned", "error", "executable_not_found"
    pub service: String,
    pub runtime_type: String,
    pub action: Option<String>,
    pub action_label: Option<String>,
    pub message: String,
}

pub struct RuntimeProvisioner;

impl RuntimeProvisioner {
    pub fn which_exists(cmd: &str) -> bool {
        let probe = if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        };
        Command::new(probe)
            .arg(cmd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub fn compute_files_fingerprint(
        working_dir: &Path,
        files: &[String],
    ) -> Result<String, String> {
        let mut hasher = Sha256::new();
        let mut found_any = false;
        for file_name in files {
            let p = working_dir.join(file_name);
            if p.exists() {
                found_any = true;
                let content =
                    fs::read(&p).map_err(|e| format!("Failed to read {}: {}", p.display(), e))?;
                hasher.update(file_name.as_bytes());
                hasher.update(&content);
            }
        }
        if !found_any {
            return Err("None of the specified fingerprint files exist".to_string());
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    pub fn provision_service(
        working_dir: &Path,
        service_name: &str,
        service_cfg: &super::manifest::ServiceConfig,
        force: bool,
    ) -> Result<PathBuf, ProvisioningDiagnostic> {
        Self::provision_service_in(
            working_dir,
            &working_dir.join(".venv"),
            service_name,
            service_cfg,
            force,
        )
    }

    pub fn provision_service_in(
        working_dir: &Path,
        runtime_dir: &Path,
        service_name: &str,
        service_cfg: &super::manifest::ServiceConfig,
        force: bool,
    ) -> Result<PathBuf, ProvisioningDiagnostic> {
        let runtime = service_cfg.get_runtime();
        match runtime.runtime_type.as_str() {
            "python" => PythonProvisioningManager::provision_environment_in(
                working_dir,
                runtime_dir,
                runtime.requirements.as_deref(),
                force,
            )
            .map_err(|e| ProvisioningDiagnostic {
                code: "runtime_not_provisioned".to_string(),
                service: service_name.to_string(),
                runtime_type: "python".to_string(),
                action: Some("provision".to_string()),
                action_label: Some("Prepare application runtime".to_string()),
                message: e.to_string(),
            }),
            "node" => {
                let pkg_json = working_dir.join("package.json");
                let node_modules = runtime_dir.join("node_modules");
                let current_fingerprint = runtime
                    .fingerprint_files
                    .as_ref()
                    .and_then(|files| Self::compute_files_fingerprint(working_dir, files).ok());
                let fingerprint_matches = current_fingerprint.as_ref().is_some_and(|current| {
                    fs::read_to_string(runtime_dir.join(".deps_fingerprint"))
                        .map(|saved| saved.trim() == current.trim())
                        .unwrap_or(false)
                });
                let dependencies_changed = current_fingerprint.is_some() && !fingerprint_matches;

                if pkg_json.exists() && (force || !node_modules.exists() || dependencies_changed) {
                    fs::create_dir_all(runtime_dir).map_err(|e| ProvisioningDiagnostic {
                        code: "runtime_directory_failed".to_string(),
                        service: service_name.to_string(),
                        runtime_type: "node".to_string(),
                        action: Some("provision".to_string()),
                        action_label: Some("Prepare application runtime".to_string()),
                        message: e.to_string(),
                    })?;
                    for name in ["package.json", "package-lock.json", "npm-shrinkwrap.json"] {
                        let source = working_dir.join(name);
                        if source.exists() {
                            fs::copy(&source, runtime_dir.join(name)).map_err(|e| {
                                ProvisioningDiagnostic {
                                    code: "runtime_copy_failed".to_string(),
                                    service: service_name.to_string(),
                                    runtime_type: "node".to_string(),
                                    action: Some("provision".to_string()),
                                    action_label: Some("Prepare application runtime".to_string()),
                                    message: format!(
                                        "Failed to copy '{}': {}",
                                        source.display(),
                                        e
                                    ),
                                }
                            })?;
                        }
                    }
                    let mut cmd = Command::new(if cfg!(target_os = "windows") {
                        "npm.cmd"
                    } else {
                        "npm"
                    });
                    cmd.arg("install").current_dir(runtime_dir);
                    let out = cmd.output().map_err(|e| ProvisioningDiagnostic {
                        code: "npm_install_failed".to_string(),
                        service: service_name.to_string(),
                        runtime_type: "node".to_string(),
                        action: Some("provision".to_string()),
                        action_label: Some("Run npm install".to_string()),
                        message: e.to_string(),
                    })?;
                    if !out.status.success() {
                        return Err(ProvisioningDiagnostic {
                            code: "npm_install_failed".to_string(),
                            service: service_name.to_string(),
                            runtime_type: "node".to_string(),
                            action: Some("provision".to_string()),
                            action_label: Some("Run npm install".to_string()),
                            message: String::from_utf8_lossy(&out.stderr).to_string(),
                        });
                    }
                    if let Some(fingerprint) = current_fingerprint {
                        fs::write(runtime_dir.join(".deps_fingerprint"), fingerprint).map_err(
                            |e| ProvisioningDiagnostic {
                                code: "runtime_fingerprint_failed".to_string(),
                                service: service_name.to_string(),
                                runtime_type: "node".to_string(),
                                action: Some("provision".to_string()),
                                action_label: Some("Prepare application runtime".to_string()),
                                message: format!(
                                    "Failed to record dependency state in '{}': {}",
                                    runtime_dir.display(),
                                    e
                                ),
                            },
                        )?;
                    }
                }
                let node_cmd = if cfg!(target_os = "windows") {
                    "node.exe"
                } else {
                    "node"
                };
                if Self::which_exists(node_cmd) {
                    Ok(PathBuf::from(node_cmd))
                } else if Self::which_exists("node") {
                    Ok(PathBuf::from("node"))
                } else {
                    let raw_bin = service_cfg
                        .command
                        .first()
                        .map(|s| s.as_str())
                        .unwrap_or("node");
                    let direct = working_dir.join(raw_bin);
                    if direct.exists() {
                        Ok(direct)
                    } else {
                        Err(ProvisioningDiagnostic {
                            code: "node_not_found".to_string(),
                            service: service_name.to_string(),
                            runtime_type: "node".to_string(),
                            action: Some("provision".to_string()),
                            action_label: Some("Install Node.js runtime".to_string()),
                            message: format!(
                                "Node.js runtime executable was not found on system PATH"
                            ),
                        })
                    }
                }
            }
            _ => {
                let bin_name = service_cfg
                    .command
                    .first()
                    .map(|s| s.as_str())
                    .unwrap_or("");
                if bin_name.is_empty() {
                    return Err(ProvisioningDiagnostic {
                        code: "invalid_command".to_string(),
                        service: service_name.to_string(),
                        runtime_type: runtime.runtime_type.clone(),
                        action: None,
                        action_label: None,
                        message: format!("Service '{}' specifies an empty command", service_name),
                    });
                }

                let direct_bin = working_dir.join(bin_name);
                let binary_present = direct_bin.exists() || Self::which_exists(bin_name);

                if let Some(ref install_cmd) = runtime.install_command {
                    if !install_cmd.is_empty() {
                        let mut needs_install = force || !binary_present;
                        let mut current_fp = None;

                        if let Some(ref fp_files) = runtime.fingerprint_files {
                            if let Ok(fp) = Self::compute_files_fingerprint(working_dir, fp_files) {
                                current_fp = Some(fp.clone());
                                let fp_file = runtime_dir.join(".deps_fingerprint");
                                if let Ok(saved_fp) = fs::read_to_string(&fp_file) {
                                    if saved_fp.trim() != fp.trim() {
                                        needs_install = true;
                                    }
                                } else {
                                    needs_install = true;
                                }
                            }
                        }

                        if needs_install {
                            fs::create_dir_all(runtime_dir).map_err(|e| {
                                ProvisioningDiagnostic {
                                    code: "runtime_directory_failed".to_string(),
                                    service: service_name.to_string(),
                                    runtime_type: runtime.runtime_type.clone(),
                                    action: Some("provision".to_string()),
                                    action_label: Some("Prepare application runtime".to_string()),
                                    message: e.to_string(),
                                }
                            })?;
                            let mut cmd = Command::new(&install_cmd[0]);
                            cmd.args(&install_cmd[1..])
                                .current_dir(working_dir)
                                .env("PIXIEVAULT_RUNTIME_DIR", runtime_dir);
                            let out = cmd.output().map_err(|e| ProvisioningDiagnostic {
                                code: "custom_install_failed".to_string(),
                                service: service_name.to_string(),
                                runtime_type: runtime.runtime_type.clone(),
                                action: Some("provision".to_string()),
                                action_label: Some(format!(
                                    "Run install command: {}",
                                    install_cmd.join(" ")
                                )),
                                message: format!("Failed to execute custom install command: {}", e),
                            })?;

                            if !out.status.success() {
                                let stderr = String::from_utf8_lossy(&out.stderr);
                                let stdout = String::from_utf8_lossy(&out.stdout);
                                let detail = if !stderr.trim().is_empty() {
                                    stderr.to_string()
                                } else {
                                    stdout.to_string()
                                };
                                return Err(ProvisioningDiagnostic {
                                    code: "custom_install_failed".to_string(),
                                    service: service_name.to_string(),
                                    runtime_type: runtime.runtime_type.clone(),
                                    action: Some("provision".to_string()),
                                    action_label: Some(format!(
                                        "Run install command: {}",
                                        install_cmd.join(" ")
                                    )),
                                    message: format!(
                                        "Custom install command failed with status {}:\n{}",
                                        out.status, detail
                                    ),
                                });
                            }

                            if let Some(fp) = current_fp {
                                let _ = fs::write(runtime_dir.join(".deps_fingerprint"), fp);
                            }
                        }
                    }
                }

                // Verify the final executable
                if direct_bin.exists() {
                    Ok(direct_bin)
                } else if Self::which_exists(bin_name) {
                    Ok(PathBuf::from(bin_name))
                } else {
                    Err(ProvisioningDiagnostic {
                        code: "executable_not_found".to_string(),
                        service: service_name.to_string(),
                        runtime_type: runtime.runtime_type.clone(),
                        action: Some("provision".to_string()),
                        action_label: Some("Install or configure required binary".to_string()),
                        message: format!(
                            "Binary '{}' for service '{}' not found at '{}' or on system PATH",
                            bin_name,
                            service_name,
                            direct_bin.display()
                        ),
                    })
                }
            }
        }
    }

    /// Authoritative resolution of service executable and arguments template for process launch
    pub fn resolve_service_execution(
        working_dir: &Path,
        service_name: &str,
        service_cfg: &super::manifest::ServiceConfig,
    ) -> Result<(PathBuf, Vec<String>), String> {
        Self::resolve_service_execution_in(
            working_dir,
            &working_dir.join(".venv"),
            service_name,
            service_cfg,
        )
    }

    /// Prepare a service runtime on first launch, then resolve its executable.
    ///
    /// Python readiness can be checked without running pip, so an already valid
    /// environment takes the fast path. Other runtime types have idempotent
    /// provisioners which also account for dependency fingerprint changes.
    pub fn prepare_service_execution_in(
        working_dir: &Path,
        runtime_dir: &Path,
        service_name: &str,
        service_cfg: &super::manifest::ServiceConfig,
    ) -> Result<(PathBuf, Vec<String>), String> {
        let auto_install = service_cfg.auto_install.unwrap_or(true);
        let runtime = service_cfg.get_runtime();

        if runtime.runtime_type == "python" {
            match Self::resolve_service_execution_in(
                working_dir,
                runtime_dir,
                service_name,
                service_cfg,
            ) {
                Ok(execution) => return Ok(execution),
                Err(readiness_error) if !auto_install => {
                    return Err(format!(
                        "{}. Automatic runtime provisioning is disabled for this service.",
                        readiness_error
                    ));
                }
                Err(_) => {}
            }
        } else if !auto_install {
            return Self::resolve_service_execution_in(
                working_dir,
                runtime_dir,
                service_name,
                service_cfg,
            );
        }

        Self::provision_service_in(working_dir, runtime_dir, service_name, service_cfg, false)
            .map_err(|diagnostic| {
                format!(
                    "Automatic runtime provisioning failed for service '{}': {}",
                    service_name, diagnostic.message
                )
            })?;

        Self::resolve_service_execution_in(working_dir, runtime_dir, service_name, service_cfg)
    }

    pub fn resolve_service_execution_in(
        working_dir: &Path,
        runtime_dir: &Path,
        service_name: &str,
        service_cfg: &super::manifest::ServiceConfig,
    ) -> Result<(PathBuf, Vec<String>), String> {
        if service_cfg.command.is_empty() {
            return Err(format!(
                "Service '{}' defines an empty command",
                service_name
            ));
        }

        let runtime = service_cfg.get_runtime();
        match runtime.runtime_type.as_str() {
            "python" => {
                let venv_py = PythonProvisioningManager::verify_environment_ready_in(
                    working_dir,
                    runtime_dir,
                    runtime.requirements.as_deref(),
                )
                .map_err(|e| {
                    format!("Python runtime error for service '{}': {}", service_name, e)
                })?;

                let first_arg = &service_cfg.command[0];
                let is_py_launcher = first_arg == "python"
                    || first_arg == "python3"
                    || first_arg == "py"
                    || first_arg == "python.exe";

                let args = if is_py_launcher {
                    service_cfg.command[1..].to_vec()
                } else {
                    service_cfg.command.clone()
                };

                Ok((venv_py, args))
            }
            "node" => {
                let node_cmd = if cfg!(target_os = "windows") {
                    "node.exe"
                } else {
                    "node"
                };
                let first_arg = &service_cfg.command[0];
                let is_node_launcher =
                    first_arg == "node" || first_arg == "nodejs" || first_arg == "node.exe";

                let args = if is_node_launcher {
                    service_cfg.command[1..].to_vec()
                } else {
                    service_cfg.command.clone()
                };

                let bin_path = if Self::which_exists(node_cmd) {
                    PathBuf::from(node_cmd)
                } else if Self::which_exists("node") {
                    PathBuf::from("node")
                } else {
                    let direct = working_dir.join(first_arg);
                    if direct.exists() {
                        direct
                    } else {
                        PathBuf::from(node_cmd)
                    }
                };

                Ok((bin_path, args))
            }
            _ => {
                let raw_bin = &service_cfg.command[0];
                let direct = working_dir.join(raw_bin);
                let bin_path = if direct.exists() {
                    direct
                } else if Self::which_exists(raw_bin) {
                    PathBuf::from(raw_bin)
                } else {
                    return Err(format!(
                        "Binary '{}' for service '{}' was not found at '{}' or on system PATH",
                        raw_bin,
                        service_name,
                        direct.display()
                    ));
                };

                let args = service_cfg.command[1..].to_vec();
                Ok((bin_path, args))
            }
        }
    }

    /// Authoritative resolution of service executable command path
    pub fn resolve_service_command(
        working_dir: &Path,
        service_name: &str,
        service_cfg: &super::manifest::ServiceConfig,
    ) -> Result<PathBuf, String> {
        Self::resolve_service_execution(working_dir, service_name, service_cfg).map(|(bin, _)| bin)
    }
}
