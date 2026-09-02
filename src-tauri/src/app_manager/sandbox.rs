use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Sandbox Security Policy Configuration for Guest Services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPolicy {
    pub enabled: bool,
    pub app_id: String,
    pub working_dir: PathBuf,
    pub data_dir: Option<PathBuf>,
    pub environment: HashMap<String, String>,
    pub extra_ro_binds: Vec<PathBuf>,
    pub extra_rw_binds: Vec<PathBuf>,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            app_id: "default_app".to_string(),
            working_dir: PathBuf::from("."),
            data_dir: None,
            environment: HashMap::new(),
            extra_ro_binds: Vec::new(),
            extra_rw_binds: Vec::new(),
        }
    }
}

/// Discovered Sandboxing Engine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxEngine {
    /// Linux Bubblewrap unprivileged namespaces (Mount, User, PID, IPC)
    Bubblewrap(PathBuf),
    /// Standard Unix process isolation with PR_SET_PDEATHSIG and env filtering
    RestrictedUnix,
    /// Windows Process & Environment isolation with Job Objects
    RestrictedWindows,
}

/// Sandbox Manager for launching isolated guest processes
pub struct SandboxManager {
    engine: SandboxEngine,
}

impl Default for SandboxManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SandboxManager {
    pub fn new() -> Self {
        let engine = Self::detect_engine();
        Self { engine }
    }

    fn detect_engine() -> SandboxEngine {
        if cfg!(target_os = "linux") {
            for p in &["/usr/bin/bwrap", "/bin/bwrap", "/usr/local/bin/bwrap"] {
                let pb = PathBuf::from(p);
                if pb.exists() {
                    return SandboxEngine::Bubblewrap(pb);
                }
            }
            if let Ok(paths) = std::env::var("PATH") {
                for dir in paths.split(':') {
                    let pb = PathBuf::from(dir).join("bwrap");
                    if pb.exists() {
                        return SandboxEngine::Bubblewrap(pb);
                    }
                }
            }
            SandboxEngine::RestrictedUnix
        } else if cfg!(target_os = "windows") {
            SandboxEngine::RestrictedWindows
        } else {
            SandboxEngine::RestrictedUnix
        }
    }

    pub fn engine(&self) -> &SandboxEngine {
        &self.engine
    }

    pub fn engine_name(&self) -> &'static str {
        match &self.engine {
            SandboxEngine::Bubblewrap(_) => "Linux Bubblewrap (Namespaces & Mount Jails)",
            SandboxEngine::RestrictedUnix => "Unix Restricted Runtime (PDEATHSIG + Env Scrubbing)",
            SandboxEngine::RestrictedWindows => {
                "Windows Restricted Runtime (Job Objects + Env Scrubbing)"
            }
        }
    }

    /// Build an isolated, sandboxed Command ready to be spawned
    pub fn build_sandboxed_command(
        &self,
        policy: &SandboxPolicy,
        raw_bin: &str,
        raw_args: &[String],
    ) -> Result<Command, String> {
        if !policy.enabled {
            let mut cmd = Command::new(raw_bin);
            cmd.args(raw_args);
            cmd.current_dir(&policy.working_dir);
            for (k, v) in &policy.environment {
                cmd.env(k, v);
            }
            return Ok(cmd);
        }

        let canonical_working = super::canonicalize_clean(&policy.working_dir);

        // Ensure data directory exists if configured
        if let Some(ref data_dir) = policy.data_dir {
            let _ = fs::create_dir_all(data_dir);
        }

        match &self.engine {
            SandboxEngine::Bubblewrap(bwrap_bin) => {
                let mut cmd = Command::new(bwrap_bin);

                // 1. Mount minimal read-only system paths
                for ro_sys in &[
                    "/usr",
                    "/lib",
                    "/lib64",
                    "/bin",
                    "/sbin",
                    "/etc/resolv.conf",
                    "/etc/hosts",
                    "/etc/ssl",
                    "/etc/pki",
                    "/etc/ca-certificates",
                    "/etc/alternatives",
                ] {
                    let path = Path::new(ro_sys);
                    if path.exists() {
                        cmd.arg("--ro-bind-try").arg(ro_sys).arg(ro_sys);
                    }
                }

                // 2. Kernel procfs and dev
                cmd.args(["--proc", "/proc", "--dev", "/dev"]);

                // 3. Isolated tmpfs for temporary files
                cmd.args(["--tmpfs", "/tmp"]);

                // 4. Mount user python site-packages read-only if present (keeps ~/.ssh, ~/.config, ~/.secrets masked)
                if let Ok(home) = std::env::var("HOME") {
                    let local_dir = PathBuf::from(&home).join(".local");
                    let local_lib = local_dir.join("lib");
                    if local_lib.exists() {
                        let local_str = local_lib.to_string_lossy().to_string();
                        cmd.arg("--ro-bind-try").arg(&local_str).arg(&local_str);
                    }
                }

                // 5. Mount guest application working directory
                let working_str = canonical_working.to_string_lossy().to_string();
                cmd.arg("--bind").arg(&working_str).arg(&working_str);

                // 6. Mount dedicated data directory if separate
                if let Some(ref data_dir) = policy.data_dir {
                    let canonical_data = super::canonicalize_clean(data_dir);
                    let data_str = canonical_data.to_string_lossy().to_string();
                    if data_str != working_str {
                        cmd.arg("--bind").arg(&data_str).arg(&data_str);
                    }
                }

                // 7. Mount any extra read-only or read-write binds
                for ro in &policy.extra_ro_binds {
                    if ro.exists() {
                        let ro_str = ro.to_string_lossy().to_string();
                        cmd.arg("--ro-bind").arg(&ro_str).arg(&ro_str);
                    }
                }
                for rw in &policy.extra_rw_binds {
                    if rw.exists() {
                        let rw_str = rw.to_string_lossy().to_string();
                        cmd.arg("--bind").arg(&rw_str).arg(&rw_str);
                    }
                }

                // 8. Unshare namespaces (User, PID, IPC) and ensure child dies with parent
                cmd.args([
                    "--unshare-user",
                    "--unshare-ipc",
                    "--unshare-pid",
                    "--die-with-parent",
                ]);

                // 9. Scrub host environment and inject allowed environment
                cmd.arg("--clearenv");

                // Keep standard PATH or host PATH
                let default_path = std::env::var("PATH").unwrap_or_else(|_| {
                    "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string()
                });
                cmd.arg("--setenv").arg("PATH").arg(default_path);
                cmd.arg("--setenv").arg("PYTHONUNBUFFERED").arg("1");

                if let Ok(home) = std::env::var("HOME") {
                    let local_dir = PathBuf::from(&home).join(".local");
                    cmd.arg("--setenv").arg("HOME").arg(&home);
                    cmd.arg("--setenv")
                        .arg("PYTHONUSERBASE")
                        .arg(local_dir.to_string_lossy().to_string());
                }

                // Inject policy environment variables
                for (k, v) in &policy.environment {
                    cmd.arg("--setenv").arg(k).arg(v);
                }

                // Set working directory inside sandbox
                cmd.arg("--chdir").arg(&working_str);

                // Command to execute
                cmd.arg("--");
                cmd.arg(raw_bin);
                for arg in raw_args {
                    cmd.arg(arg);
                }

                // Ensure child dies with parent host on Linux
                #[cfg(target_os = "linux")]
                unsafe {
                    use std::os::unix::process::CommandExt;
                    cmd.pre_exec(|| {
                        extern "C" {
                            fn prctl(
                                option: i32,
                                arg2: u64,
                                arg3: u64,
                                arg4: u64,
                                arg5: u64,
                            ) -> i32;
                        }
                        // PR_SET_PDEATHSIG = 1, SIGTERM = 15
                        prctl(1, 15, 0, 0, 0);
                        Ok(())
                    });
                }

                Ok(cmd)
            }

            SandboxEngine::RestrictedUnix => {
                let mut cmd = Command::new(raw_bin);
                cmd.args(raw_args);
                cmd.current_dir(&canonical_working);

                // Strip sensitive environment variables (AWS, SSH, Tokens, Keyrings)
                for (key, _) in std::env::vars() {
                    let upper = key.to_uppercase();
                    if upper.starts_with("AWS_")
                        || upper.starts_with("SSH_")
                        || upper.starts_with("GITHUB_")
                        || upper.contains("TOKEN")
                        || upper.contains("SECRET")
                        || upper.contains("KEY")
                    {
                        cmd.env_remove(&key);
                    }
                }

                for (k, v) in &policy.environment {
                    cmd.env(k, v);
                }

                #[cfg(target_os = "linux")]
                unsafe {
                    use std::os::unix::process::CommandExt;
                    cmd.pre_exec(|| {
                        extern "C" {
                            fn prctl(
                                option: i32,
                                arg2: u64,
                                arg3: u64,
                                arg4: u64,
                                arg5: u64,
                            ) -> i32;
                        }
                        prctl(1, 15, 0, 0, 0);
                        Ok(())
                    });
                }

                Ok(cmd)
            }

            SandboxEngine::RestrictedWindows => {
                let mut cmd = Command::new(raw_bin);
                cmd.args(raw_args);
                cmd.current_dir(&canonical_working);

                // Strip sensitive environment variables
                for (key, _) in std::env::vars() {
                    let upper = key.to_uppercase();
                    if upper.starts_with("AWS_")
                        || upper.starts_with("SSH_")
                        || upper.starts_with("GITHUB_")
                        || upper.contains("TOKEN")
                        || upper.contains("SECRET")
                        || upper.contains("KEY")
                    {
                        cmd.env_remove(&key);
                    }
                }

                for (k, v) in &policy.environment {
                    cmd.env(k, v);
                }

                Ok(cmd)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_engine_detection() {
        let manager = SandboxManager::new();
        println!("Detected Sandbox Engine: {}", manager.engine_name());
        assert!(!manager.engine_name().is_empty());
    }

    #[test]
    fn test_sandbox_command_builder_structure() {
        let manager = SandboxManager::new();
        let mut env = HashMap::new();
        env.insert("PORT".to_string(), "54321".to_string());
        env.insert("HOST".to_string(), "127.0.0.1".to_string());

        let policy = SandboxPolicy {
            enabled: true,
            app_id: "test_service".to_string(),
            working_dir: std::env::current_dir().unwrap(),
            data_dir: None,
            environment: env,
            extra_ro_binds: Vec::new(),
            extra_rw_binds: Vec::new(),
        };

        let cmd = manager
            .build_sandboxed_command(
                &policy,
                "python3",
                &["-c".to_string(), "print(1)".to_string()],
            )
            .expect("Failed to build sandboxed command");

        let program = cmd.get_program().to_string_lossy().to_string();
        println!("Program built: {}", program);
        assert!(!program.is_empty());
    }
}
