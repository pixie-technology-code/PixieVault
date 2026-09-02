use super::manifest::{AppManifest, HealthcheckConfig, ServiceConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ================= Legacy Sidecar Compatibility Types =================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarStatus {
    pub app_id: String,
    pub is_running: bool,
    pub pid: Option<u32>,
    pub port: u16,
    pub url: Option<String>,
    pub error: Option<String>,
}

// ================= Native Composer Types =================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceRuntimeStatus {
    pub name: String,
    pub is_running: bool,
    pub pid: Option<u32>,
    pub port: u16,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposerAppStatus {
    pub app_id: String,
    pub is_running: bool,
    pub services: HashMap<String, ServiceRuntimeStatus>,
    pub entrypoint_url: String,
    pub error: Option<String>,
}

/// Dynamic Ephemeral Loopback Port Allocator
pub fn allocate_ephemeral_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to bind ephemeral loopback port: {}", e))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Failed to retrieve ephemeral port: {}", e))?
        .port();
    drop(listener);
    Ok(port)
}

/// Helper to classify address conflicts vs configuration / binary errors
pub fn is_address_conflict_error(err_str: &str) -> bool {
    let lower = err_str.to_lowercase();
    lower.contains("address already in use")
        || lower.contains("eaddrinuse")
        || lower.contains("only one usage of each socket address")
        || lower.contains("os { code: 98")
        || lower.contains("os { code: 10048")
        || lower.contains("connection refused")
        || lower.contains("failed to bind")
        || lower.contains("port collision")
        || lower.contains("address conflict")
}

/// Asynchronous Healthcheck & TCP/HTTP Readiness Probe with fast child failure detection
pub fn poll_service_health(
    port: u16,
    healthcheck: Option<&HealthcheckConfig>,
) -> Result<(), String> {
    poll_child_health(None, port, healthcheck, None)
}

pub fn poll_child_health(
    mut child: Option<&mut Child>,
    port: u16,
    healthcheck: Option<&HealthcheckConfig>,
    stderr_logs: Option<Arc<Mutex<Vec<String>>>>,
) -> Result<(), String> {
    let endpoint = healthcheck.map(|h| h.endpoint.as_str()).unwrap_or("/");
    let interval = Duration::from_millis(healthcheck.map(|h| h.interval_ms).unwrap_or(200));
    let timeout = Duration::from_millis(healthcheck.map(|h| h.timeout_ms).unwrap_or(30000));
    let expected_status = healthcheck.and_then(|h| h.expected_status);
    let expected_body = healthcheck.and_then(|h| h.expected_body.as_deref());
    let start = Instant::now();

    let addr: SocketAddr = format!("127.0.0.1:{}", port)
        .parse()
        .map_err(|e| format!("Invalid socket address: {}", e))?;

    while start.elapsed() < timeout {
        // Fast failure detection: check if child process exited prematurely
        if let Some(ref mut c) = child {
            if let Ok(Some(status)) = c.try_wait() {
                let captured = if let Some(ref logs_arc) = stderr_logs {
                    logs_arc.lock().unwrap().join("\n")
                } else {
                    String::new()
                };
                let clean_err = captured.trim();
                if clean_err.is_empty() {
                    return Err(format!(
                        "Process terminated prematurely with exit status: {}",
                        status
                    ));
                } else {
                    return Err(format!(
                        "Process terminated with status {}:\n{}",
                        status, clean_err
                    ));
                }
            }
        }

        if let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(300)) {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
            let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));

            let http_req = format!(
                "GET {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
                endpoint, port
            );

            if stream.write_all(http_req.as_bytes()).is_ok() {
                let mut buffer = [0u8; 1024];
                if let Ok(bytes_read) = stream.read(&mut buffer) {
                    if bytes_read > 0 {
                        let response = String::from_utf8_lossy(&buffer[..bytes_read]);
                        if response.starts_with("HTTP/") {
                            // Extract HTTP status code
                            let status_part = response.split_whitespace().nth(1);
                            let status_code =
                                status_part.and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);

                            let is_status_ok = if let Some(req_status) = expected_status {
                                status_code == req_status
                            } else {
                                status_code >= 200 && status_code < 400
                            };

                            let is_body_ok = if let Some(exp_body) = expected_body {
                                response.contains(exp_body)
                            } else {
                                true
                            };

                            if is_status_ok && is_body_ok {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
        std::thread::sleep(interval);
    }

    let captured = if let Some(ref logs_arc) = stderr_logs {
        logs_arc.lock().unwrap().join("\n")
    } else {
        String::new()
    };
    let clean_err = captured.trim();
    if clean_err.is_empty() {
        Err(format!(
            "Timed out waiting for service on 127.0.0.1:{} after {}ms",
            port,
            timeout.as_millis()
        ))
    } else {
        Err(format!(
            "Timed out waiting for service on 127.0.0.1:{} after {}ms. Captured output:\n{}",
            port,
            timeout.as_millis(),
            clean_err
        ))
    }
}

// ================= Transactional Rollback Guard =================

struct ComposerStartupTx {
    processes: HashMap<String, Child>,
    ports: HashMap<String, u16>,
    statuses: HashMap<String, ServiceRuntimeStatus>,
    committed: bool,
}

impl ComposerStartupTx {
    fn new() -> Self {
        Self {
            processes: HashMap::new(),
            ports: HashMap::new(),
            statuses: HashMap::new(),
            committed: false,
        }
    }

    fn add_service(&mut self, name: String, child: Child, port: u16, pid: u32) {
        self.processes.insert(name.clone(), child);
        self.ports.insert(name.clone(), port);
        self.statuses.insert(
            name.clone(),
            ServiceRuntimeStatus {
                name,
                is_running: true,
                pid: Some(pid),
                port,
                error: None,
            },
        );
    }

    fn rollback(&mut self) {
        if !self.committed {
            for (svc_name, mut child) in self.processes.drain() {
                eprintln!(
                    "[VaultComposer] Rollback: Terminating spawned service '{}' (PID: {:?})",
                    svc_name,
                    child.id()
                );
                terminate_child_process(&mut child);
            }
            self.ports.clear();
            self.statuses.clear();
        }
    }

    fn commit(
        mut self,
    ) -> (
        HashMap<String, Child>,
        HashMap<String, u16>,
        HashMap<String, ServiceRuntimeStatus>,
    ) {
        self.committed = true;
        (
            std::mem::take(&mut self.processes),
            std::mem::take(&mut self.ports),
            std::mem::take(&mut self.statuses),
        )
    }
}

impl Drop for ComposerStartupTx {
    fn drop(&mut self) {
        self.rollback();
    }
}

// ================= Static App Embedded Server =================

fn get_mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("wasm") => "application/wasm",
        Some("pdf") => "application/pdf",
        Some("txt") | Some("md") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn urlencoding_decode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut bytes = input.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let h1 = bytes.next();
            let h2 = bytes.next();
            if let (Some(h1), Some(h2)) = (h1, h2) {
                let hex_str = [h1, h2];
                if let Ok(s) = std::str::from_utf8(&hex_str) {
                    if let Ok(val) = u8::from_str_radix(s, 16) {
                        result.push(val as char);
                        continue;
                    }
                }
            }
            result.push('%');
        } else if b == b'+' {
            result.push(' ');
        } else {
            result.push(b as char);
        }
    }
    result
}

fn serve_static_app(
    app_dir: PathBuf,
    listener: TcpListener,
    stop_signal: Arc<AtomicBool>,
) {
    let _ = listener.set_nonblocking(true);

    while !stop_signal.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));

                let mut buffer = [0u8; 4096];
                if let Ok(n) = stream.read(&mut buffer) {
                    if n > 0 {
                        let request = String::from_utf8_lossy(&buffer[..n]);
                        let first_line = request.lines().next().unwrap_or("");
                        let mut parts = first_line.split_whitespace();
                        let method = parts.next().unwrap_or("GET");
                        let raw_path = parts.next().unwrap_or("/");

                        if method == "GET" || method == "HEAD" {
                            let clean_req_path = raw_path.split('?').next().unwrap_or(raw_path);
                            let clean_req_path = clean_req_path.split('#').next().unwrap_or(clean_req_path);
                            let decoded_path = urlencoding_decode(clean_req_path);

                            let relative_path = decoded_path.trim_start_matches('/');
                            let relative_path = if relative_path.is_empty() {
                                "index.html"
                            } else {
                                relative_path
                            };

                            let target_path = app_dir.join(relative_path);
                            let is_safe = !relative_path.contains("..")
                                && target_path.starts_with(&app_dir)
                                && target_path.exists()
                                && target_path.is_file();

                            if is_safe {
                                if let Ok(contents) = fs::read(&target_path) {
                                    let mime = get_mime_type(&target_path);
                                    let header = format!(
                                        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
                                        mime,
                                        contents.len()
                                    );
                                    let _ = stream.write_all(header.as_bytes());
                                    if method == "GET" {
                                        let _ = stream.write_all(&contents);
                                    }
                                } else {
                                    let _ = stream.write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 21\r\nConnection: close\r\n\r\nInternal Server Error");
                                }
                            } else {
                                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nNot Found");
                            }
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(15));
            }
            Err(_) => {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

// ================= Vault Composer Micro-Orchestrator =================

pub struct VaultComposer {
    processes: Arc<Mutex<HashMap<String, HashMap<String, Child>>>>,
    ports: Arc<Mutex<HashMap<String, HashMap<String, u16>>>>,
    entrypoints: Arc<Mutex<HashMap<String, String>>>,
    static_servers: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    sandbox: super::sandbox::SandboxManager,
}

impl Default for VaultComposer {
    fn default() -> Self {
        Self::new()
    }
}

/// Terminate child process and its process tree
pub fn terminate_child_process(child: &mut Child) {
    let _ = child.kill();
    #[cfg(unix)]
    {
        let pid = child.id() as i32;
        extern "C" {
            fn kill(pid: i32, sig: i32) -> i32;
        }
        unsafe {
            kill(-pid, 9);
            kill(pid, 9);
        }
    }
    let _ = child.wait();
}

impl VaultComposer {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
            ports: Arc::new(Mutex::new(HashMap::new())),
            entrypoints: Arc::new(Mutex::new(HashMap::new())),
            static_servers: Arc::new(Mutex::new(HashMap::new())),
            sandbox: super::sandbox::SandboxManager::new(),
        }
    }

    /// Launch all composer services or static file servers defined in an application manifest
    /// Launch all composer services or static file servers defined in an application manifest
    pub fn start_composer_app(
        &self,
        manifest: &AppManifest,
        app_dir: &Path,
        vault_data_dir: Option<&Path>,
    ) -> Result<ComposerAppStatus, String> {
        self.start_composer_app_with_env(manifest, app_dir, vault_data_dir, None)
    }

    /// Launch composer services with additional in-memory environment variables (e.g. APP_ENCRYPTION_KEY)
    pub fn start_composer_app_with_env(
        &self,
        manifest: &AppManifest,
        app_dir: &Path,
        vault_data_dir: Option<&Path>,
        extra_env: Option<&HashMap<String, String>>,
    ) -> Result<ComposerAppStatus, String> {
        let app_id = &manifest.app_id;

        // Check if already running
        let current_status = self.get_app_status(app_id, Some(manifest));
        if current_status.is_running {
            return Ok(current_status);
        }

        // 1. Pure static guest applications (no composer backend microservices)
        if !manifest.has_composer() {
            let listener = TcpListener::bind("127.0.0.1:0")
                .map_err(|e| format!("Failed to bind loopback static server: {}", e))?;
            let port = listener
                .local_addr()
                .map_err(|e| format!("Failed to get loopback port: {}", e))?
                .port();

            let stop_signal = Arc::new(AtomicBool::new(false));
            let stop_clone = Arc::clone(&stop_signal);
            let dir_clone = app_dir.to_path_buf();

            std::thread::spawn(move || {
                serve_static_app(dir_clone, listener, stop_clone);
            });

            let entrypoint = if manifest.entrypoint.is_empty() {
                "index.html"
            } else {
                manifest.entrypoint.trim_start_matches('/')
            };
            let entrypoint_url = format!("http://127.0.0.1:{}/{}", port, entrypoint);

            self.static_servers
                .lock()
                .unwrap()
                .insert(app_id.to_string(), stop_signal);
            self.entrypoints
                .lock()
                .unwrap()
                .insert(app_id.to_string(), entrypoint_url.clone());

            return Ok(ComposerAppStatus {
                app_id: app_id.to_string(),
                is_running: true,
                services: HashMap::new(),
                entrypoint_url,
                error: None,
            });
        }

        // 2. Declarative Composer microservices
        let mut tx = ComposerStartupTx::new();

        if let Some(composer_cfg) = &manifest.composer {
            for (svc_name, svc_cfg) in &composer_cfg.services {
                let working_dir = super::canonicalize_clean(
                    &manifest
                        .resolve_service_working_dir(app_dir, svc_name)
                        .map_err(|e| {
                            format!(
                                "Failed to resolve working directory for service '{}': {}",
                                svc_name, e
                            )
                        })?,
                );
                let runtime_dir = super::clean_path(
                    &vault_data_dir
                        .map(|p| p.join("runtimes").join(svc_name))
                        .unwrap_or_else(|| working_dir.join(".venv")),
                );

                let (bin_path, resolved_cmd_args) =
                    super::provisioning::RuntimeProvisioner::prepare_service_execution_in(
                        &working_dir,
                        &runtime_dir,
                        svc_name,
                        svc_cfg,
                    )?;
                let bin = super::clean_path(&bin_path).to_string_lossy().to_string();

                let max_retries = 3;
                let mut last_err = String::new();
                let mut service_spawned = false;

                for attempt in 1..=max_retries {
                    let port = if svc_cfg.port.eq_ignore_ascii_case("auto") {
                        allocate_ephemeral_port()?
                    } else {
                        svc_cfg
                            .port
                            .parse::<u16>()
                            .unwrap_or_else(|_| allocate_ephemeral_port().unwrap_or(5000))
                    };

                    let mut raw_args = Vec::new();
                    for arg in &resolved_cmd_args {
                        let resolved_arg = arg
                            .replace("{{port}}", &port.to_string())
                            .replace("{{PORT}}", &port.to_string());
                        let direct_file = working_dir.join(&resolved_arg);
                        if direct_file.exists() {
                            raw_args.push(
                                super::canonicalize_clean(&direct_file)
                                    .to_string_lossy()
                                    .to_string(),
                            );
                        } else {
                            raw_args.push(resolved_arg);
                        }
                    }

                    let mut env_map = HashMap::new();
                    env_map.insert("PORT".to_string(), port.to_string());
                    env_map.insert("FLASK_RUN_PORT".to_string(), port.to_string());
                    env_map.insert("HOST".to_string(), "127.0.0.1".to_string());

                    if let Some(vd) = vault_data_dir {
                        let clean_vd = super::clean_path(vd);
                        let vd_str = clean_vd.to_string_lossy().to_string();
                        let db_path = clean_vd.join("mac_finder.db");
                        let secrets_dir = clean_vd.join("secrets");
                        env_map.insert("VAULT_STORAGE_DIR".to_string(), vd_str.clone());
                        env_map.insert("VAULT_APP_DATA".to_string(), vd_str.clone());
                        env_map.insert(
                            "APP_DB_PATH".to_string(),
                            db_path.to_string_lossy().to_string(),
                        );
                        env_map.insert(
                            "APP_SECRETS_DIR".to_string(),
                            secrets_dir.to_string_lossy().to_string(),
                        );
                    }

                    for (k, v) in &svc_cfg.environment {
                        let resolved_v = v
                            .replace("{{port}}", &port.to_string())
                            .replace("{{PORT}}", &port.to_string());
                        env_map.insert(k.clone(), resolved_v);
                    }

                    if let Some(extra) = extra_env {
                        for (k, v) in extra {
                            env_map.insert(k.clone(), v.clone());
                        }
                    }


                    let runtime = svc_cfg.get_runtime();
                    if runtime.runtime_type == "python" {
                        env_map.insert(
                            "PYTHONPATH".to_string(),
                            working_dir.to_string_lossy().to_string(),
                        );
                        env_map.insert("FLASK_ENV".to_string(), "production".to_string());
                        env_map.insert("FLASK_DEBUG".to_string(), "0".to_string());
                        env_map.insert("PYTHONUNBUFFERED".to_string(), "1".to_string());
                        env_map.insert("PYTHONIOENCODING".to_string(), "utf-8".to_string());
                        env_map.insert("PYTHONUTF8".to_string(), "1".to_string());
                        env_map.insert("PYTHONDONTWRITEBYTECODE".to_string(), "1".to_string());
                    } else if runtime.runtime_type == "node" {


                        if !env_map.contains_key("NODE_ENV") {
                            env_map.insert("NODE_ENV".to_string(), "production".to_string());
                        }
                        env_map.insert(
                            "NODE_PATH".to_string(),
                            runtime_dir
                                .join("node_modules")
                                .to_string_lossy()
                                .to_string(),
                        );
                    }
                    env_map.insert(
                        "PIXIEVAULT_RUNTIME_DIR".to_string(),
                        runtime_dir.to_string_lossy().to_string(),
                    );

                    let sandbox_enabled =
                        svc_cfg.sandbox.as_ref().map(|s| s.enabled).unwrap_or(true);
                    let policy = super::sandbox::SandboxPolicy {
                        enabled: sandbox_enabled,
                        app_id: app_id.to_string(),
                        working_dir: working_dir.clone(),
                        data_dir: vault_data_dir.map(|p| p.to_path_buf()),
                        environment: env_map,
                        extra_ro_binds: Vec::new(),
                        extra_rw_binds: Vec::new(),
                    };

                    let mut cmd = match self
                        .sandbox
                        .build_sandboxed_command(&policy, &bin, &raw_args)
                    {
                        Ok(c) => c,
                        Err(e) => {
                            return Err(format!(
                                "Sandbox policy configuration error for '{}': {}",
                                svc_name, e
                            ));
                        }
                    };

                    cmd.stdout(Stdio::piped());
                    cmd.stderr(Stdio::piped());

                    let mut child = match cmd.spawn() {
                        Ok(c) => c,
                        Err(e) => {
                            let e_str = e.to_string();
                            if is_address_conflict_error(&e_str) && attempt < max_retries {
                                last_err = format!(
                                    "Address conflict on spawn (attempt {}): {}",
                                    attempt, e_str
                                );
                                continue;
                            }
                            return Err(format!("Failed to spawn service '{}': {}", svc_name, e));
                        }
                    };

                    let pid = child.id();
                    let output_logs = Arc::new(Mutex::new(Vec::<String>::new()));
                    if let Some(stderr) = child.stderr.take() {
                        let logs_clone = Arc::clone(&output_logs);
                        let svc_label = svc_name.clone();
                        std::thread::spawn(move || {
                            use std::io::BufRead;
                            let reader = std::io::BufReader::new(stderr);
                            for line in reader.lines().flatten() {
                                eprintln!("[{}] {}", svc_label, line);
                                let mut logs = logs_clone.lock().unwrap();
                                logs.push(format!("[stderr] {}", line));
                                if logs.len() > 50 {
                                    logs.remove(0);
                                }
                            }
                        });
                    }

                    if let Some(stdout) = child.stdout.take() {
                        let logs_clone = Arc::clone(&output_logs);
                        let svc_label = svc_name.clone();
                        std::thread::spawn(move || {
                            use std::io::BufRead;
                            let reader = std::io::BufReader::new(stdout);
                            for line in reader.lines().flatten() {
                                println!("[{}] {}", svc_label, line);
                                let mut logs = logs_clone.lock().unwrap();
                                logs.push(format!("[stdout] {}", line));
                                if logs.len() > 50 {
                                    logs.remove(0);
                                }
                            }
                        });
                    }

                    match poll_child_health(
                        Some(&mut child),
                        port,
                        svc_cfg.healthcheck.as_ref(),
                        Some(output_logs),
                    ) {

                        Ok(()) => {
                            tx.add_service(svc_name.clone(), child, port, pid);
                            service_spawned = true;
                            break;
                        }
                        Err(health_err) => {
                            terminate_child_process(&mut child);
                            let h_str = health_err.to_string();
                            if is_address_conflict_error(&h_str) && attempt < max_retries {
                                last_err = format!(
                                    "Port collision on port {} (attempt {}): {}",
                                    port, attempt, h_str
                                );
                                continue;
                            } else {
                                return Err(format!(
                                    "Failed to start service '{}': Healthcheck failed on port {}: {}",
                                    svc_name, port, h_str
                                ));
                            }
                        }
                    }
                }

                if !service_spawned {
                    return Err(format!(
                        "Failed to start service '{}': {}",
                        svc_name, last_err
                    ));
                }
            }
        }

        let (procs, app_ports, service_statuses) = tx.commit();
        let resolved_entrypoint = manifest.resolve_entrypoint(&app_ports);

        self.processes
            .lock()
            .unwrap()
            .insert(app_id.to_string(), procs);
        self.ports
            .lock()
            .unwrap()
            .insert(app_id.to_string(), app_ports);
        self.entrypoints
            .lock()
            .unwrap()
            .insert(app_id.to_string(), resolved_entrypoint.clone());

        Ok(ComposerAppStatus {
            app_id: app_id.to_string(),
            is_running: true,
            services: service_statuses,
            entrypoint_url: resolved_entrypoint,
            error: None,
        })
    }

    /// Stop all services associated with an app
    pub fn stop_composer_app(&self, app_id: &str) -> bool {
        let mut stopped_any = false;

        let mut static_map = self.static_servers.lock().unwrap();
        if let Some(stop_signal) = static_map.remove(app_id) {
            stop_signal.store(true, Ordering::Relaxed);
            stopped_any = true;
        }

        let mut all_procs = self.processes.lock().unwrap();
        if let Some(mut app_procs) = all_procs.remove(app_id) {
            for (_, mut child) in app_procs.drain() {
                terminate_child_process(&mut child);
            }
            stopped_any = true;
        }

        self.ports.lock().unwrap().remove(app_id);
        self.entrypoints.lock().unwrap().remove(app_id);
        stopped_any
    }

    /// Check runtime status of all composer services for an app
    pub fn get_app_status(
        &self,
        app_id: &str,
        manifest: Option<&AppManifest>,
    ) -> ComposerAppStatus {
        let is_static_running = {
            let static_map = self.static_servers.lock().unwrap();
            static_map.contains_key(app_id)
        };

        let mut all_procs = self.processes.lock().unwrap();
        let all_ports = self.ports.lock().unwrap();
        let all_entrypoints = self.entrypoints.lock().unwrap();

        let app_ports = all_ports.get(app_id).cloned().unwrap_or_default();
        let entrypoint_url = all_entrypoints.get(app_id).cloned().unwrap_or_else(|| {
            manifest
                .map(|m| m.resolve_entrypoint(&app_ports))
                .unwrap_or_else(|| "index.html".to_string())
        });

        let mut service_statuses: HashMap<String, ServiceRuntimeStatus> = HashMap::new();
        let mut is_running = is_static_running;

        if let Some(app_procs) = all_procs.get_mut(app_id) {
            let mut dead_svcs = Vec::new();
            for (svc_name, child) in app_procs.iter_mut() {
                let port = *app_ports.get(svc_name).unwrap_or(&0);
                match child.try_wait() {
                    Ok(None) => {
                        is_running = true;
                        service_statuses.insert(
                            svc_name.clone(),
                            ServiceRuntimeStatus {
                                name: svc_name.clone(),
                                is_running: true,
                                pid: Some(child.id()),
                                port,
                                error: None,
                            },
                        );
                    }
                    _ => {
                        dead_svcs.push(svc_name.clone());
                        service_statuses.insert(
                            svc_name.clone(),
                            ServiceRuntimeStatus {
                                name: svc_name.clone(),
                                is_running: false,
                                pid: None,
                                port,
                                error: Some("Process terminated".to_string()),
                            },
                        );
                    }
                }
            }
            for dead in dead_svcs {
                app_procs.remove(&dead);
            }
        }

        ComposerAppStatus {
            app_id: app_id.to_string(),
            is_running,
            services: service_statuses,
            entrypoint_url,
            error: None,
        }
    }

    // ================= Legacy Sidecar Support Methods =================

    pub fn start_python_app(
        &self,
        app_id: &str,
        script_path: &PathBuf,
        port: u16,
    ) -> Result<SidecarStatus, String> {
        let manifest = AppManifest {
            app_id: app_id.to_string(),
            name: app_id.to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            entrypoint: format!("http://127.0.0.1:{}", port),
            author: None,
            presentation: None,
            permissions: Default::default(),
            theme_compatibility: None,
            composer: Some(crate::app_manager::manifest::ComposerConfig {
                version: "1".to_string(),
                services: {
                    let mut s = HashMap::new();
                    s.insert(
                        "backend".to_string(),
                        ServiceConfig {
                            command: vec![
                                "python3".to_string(),
                                script_path.to_string_lossy().to_string(),
                            ],
                            working_dir: script_path
                                .parent()
                                .map(|p| p.to_string_lossy().to_string()),
                            port: port.to_string(),
                            environment: HashMap::new(),
                            healthcheck: None,
                            runtime: None,
                            requirements: None,
                            auto_install: None,
                            sandbox: None,
                        },
                    );
                    s
                },
            }),
            source: None,
            signature: None,
            public_key: None,
        };

        let app_dir = script_path.parent().unwrap_or(script_path);
        let res = self.start_composer_app(&manifest, app_dir, None)?;
        let svc_status = res.services.get("backend");

        Ok(SidecarStatus {
            app_id: app_id.to_string(),
            is_running: res.is_running,
            pid: svc_status.and_then(|s| s.pid),
            port,
            url: Some(format!("http://127.0.0.1:{}", port)),
            error: res.error,
        })
    }

    pub fn stop_app(&self, app_id: &str) -> bool {
        self.stop_composer_app(app_id)
    }

    pub fn get_status(&self, app_id: &str) -> SidecarStatus {
        let res = self.get_app_status(app_id, None);
        let svc_status = res.services.get("backend");
        let port = svc_status.map(|s| s.port).unwrap_or(5000);

        SidecarStatus {
            app_id: app_id.to_string(),
            is_running: res.is_running,
            pid: svc_status.and_then(|s| s.pid),
            port,
            url: if res.is_running {
                Some(res.entrypoint_url)
            } else {
                None
            },
            error: res.error,
        }
    }

    /// Stop all services across all apps (used on vault lock or shutdown)
    pub fn stop_all(&self) {
        let mut static_map = self.static_servers.lock().unwrap();
        for (_, stop_signal) in static_map.drain() {
            stop_signal.store(true, Ordering::Relaxed);
        }

        let mut all_procs = self.processes.lock().unwrap();
        for (_, mut app_procs) in all_procs.drain() {
            for (_, mut child) in app_procs.drain() {
                terminate_child_process(&mut child);
            }
        }
        self.ports.lock().unwrap().clear();
        self.entrypoints.lock().unwrap().clear();
    }
}

impl Drop for VaultComposer {
    fn drop(&mut self) {
        self.stop_all();
    }
}

// Legacy alias
pub type SidecarManager = VaultComposer;
