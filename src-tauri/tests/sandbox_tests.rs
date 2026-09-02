use pixievault_lib::app_manager::{
    AppManifest, AppPermissions, ComposerConfig, HealthcheckConfig, PackageBundler, SandboxConfig,
    SandboxManager, SandboxPolicy, ServiceConfig, VaultComposer,
};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

#[test]
fn test_zipslip_traversal_rejection() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let zip_path = temp_dir.path().join("malicious.pvpkg");
    let extract_dir = temp_dir.path().join("extracted");

    // Construct malicious zip with directory traversal entry
    {
        let file = File::create(&zip_path).expect("Failed to create zip file");
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        // Valid manifest
        zip.start_file("manifest.json", options).unwrap();
        zip.write_all(br#"{"app_id":"malicious_test","name":"Malicious","version":"1.0.0","min_pixievault_version":"0.2.0","entrypoint":"index.html"}"#).unwrap();

        // Malicious traversal path
        zip.start_file("../outside.txt", options).unwrap();
        zip.write_all(b"PWNED").unwrap();

        zip.finish().unwrap();
    }

    // Attempt extract
    let result = PackageBundler::extract_package(&zip_path, &extract_dir);
    assert!(
        result.is_err(),
        "ZipSlip package should be rejected with an error"
    );
    let err_msg = result.err().unwrap();
    println!("ZipSlip error correctly caught: {}", err_msg);
    assert!(err_msg.contains("Security violation"));

    // Verify file was NOT written outside
    let outside_path = temp_dir.path().join("outside.txt");
    assert!(!outside_path.exists(), "Traversal file must not be created");
}

#[test]
fn test_manifest_strict_permission_checks() {
    let manifest_restricted = AppManifest {
        app_id: "restricted_app".to_string(),
        name: "Restricted App".to_string(),
        version: "1.0.0".to_string(),
        min_pixievault_version: "0.2.0".to_string(),
        description: String::new(),
        entrypoint: "index.html".to_string(),
        author: None,
        presentation: None,
        permissions: AppPermissions {
            requested_read: vec![
                "target_app:allowed_metric".to_string(),
                "engineBhpPeak".to_string(),
            ],
            requested_write: vec![],
        },
        theme_compatibility: None,
        required_capabilities: vec![],
        composer: None,
        source: None,
        signature: None,
        public_key: None,
    };

    // 1. Explicit pattern match: allowed
    assert!(manifest_restricted.can_read_metric("target_app", "allowed_metric"));

    // 2. Direct metric name match: allowed
    assert!(manifest_restricted.can_read_metric("any_app", "engineBhpPeak"));

    // 3. Unauthorized metric: denied
    assert!(!manifest_restricted.can_read_metric("target_app", "secret_admin_token"));
    assert!(!manifest_restricted.can_read_metric("other_app", "confidential_metric"));
}

#[test]
fn test_sandbox_manager_engine_and_policy() {
    let sandbox = SandboxManager::new();
    println!("Detected Sandbox Engine in test: {}", sandbox.engine_name());
    assert!(!sandbox.engine_name().is_empty());

    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let working_dir = temp_dir.path().to_path_buf();
    let data_dir = temp_dir.path().join("data");

    let mut env = HashMap::new();
    env.insert("PORT".to_string(), "8080".to_string());
    env.insert("VAULT_SECRET".to_string(), "test_secret".to_string());

    let policy = SandboxPolicy {
        enabled: true,
        app_id: "sandbox_test_app".to_string(),
        working_dir,
        data_dir: Some(data_dir),
        environment: env,
        extra_ro_binds: vec![],
        extra_rw_binds: vec![],
    };

    let cmd = sandbox
        .build_sandboxed_command(&policy, "python3", &["--version".to_string()])
        .expect("Failed to build sandboxed command");

    let prog = cmd.get_program().to_string_lossy().to_string();
    assert!(!prog.is_empty());
}

#[test]
fn test_live_sandboxed_python_service_execution() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let app_dir = temp_dir.path().to_path_buf();

    // Create a tiny Flask/HTTP server script
    let script_content = r#"
import sys
import os
from http.server import HTTPServer, BaseHTTPRequestHandler

port = int(os.environ.get('PORT', '5999'))

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-type', 'text/html')
        self.end_headers()
        # Verify that home sensitive files are not visible if under bwrap
        ssh_exists = os.path.exists(os.path.expanduser('~/.ssh'))
        self.wfile.write(f"OK:SANDBOXED:ssh_visible={ssh_exists}".encode('utf-8'))

    def log_message(self, format, *args):
        pass

server = HTTPServer(('127.0.0.1', port), Handler)
print(f"Server started on port {port}", flush=True)
server.serve_forever()
"#;

    let script_path = app_dir.join("sandboxed_app.py");
    fs::write(&script_path, script_content).expect("Failed to write test script");

    let manifest = AppManifest {
        app_id: "sandboxed_test_service".to_string(),
        name: "Sandboxed Test Service".to_string(),
        version: "1.0.0".to_string(),
        min_pixievault_version: "0.2.0".to_string(),
        description: "Sandbox integration test".to_string(),
        entrypoint: "http://127.0.0.1:{{services.web.port}}/".to_string(),
        author: None,
        presentation: None,
        permissions: AppPermissions::default(),
        theme_compatibility: None,
        required_capabilities: vec![],
        composer: Some(ComposerConfig {
            version: "1".to_string(),
            services: {
                let mut s = HashMap::new();
                s.insert(
                    "web".to_string(),
                    ServiceConfig {
                        command: vec!["python3".to_string(), "sandboxed_app.py".to_string()],
                        working_dir: None,
                        port: "auto".to_string(),
                        environment: HashMap::new(),
                        healthcheck: Some(HealthcheckConfig {
                            endpoint: "/".to_string(),
                            interval_ms: 100,
                            timeout_ms: 10000,
                            expected_status: None,
                            expected_body: None,
                        }),
                        runtime: None,
                        requirements: None,
                        auto_install: Some(false),
                        sandbox: Some(SandboxConfig {
                            enabled: true,
                            writable_dirs: vec![".".to_string()],
                            isolate_network_loopback: true,
                        }),
                    },
                );
                s
            },
        }),
        source: None,
        signature: None,
        public_key: None,
    };

    let composer = VaultComposer::new();
    let status = composer
        .start_composer_app(&manifest, &app_dir, None)
        .expect("Failed to launch sandboxed service");

    assert!(status.is_running, "Service should be marked as running");
    let web_status = status
        .services
        .get("web")
        .expect("Web service should be present");
    assert!(web_status.is_running);
    let port = web_status.port;
    assert!(port > 0);

    // Probe HTTP endpoint via TCP stream
    let addr = format!("127.0.0.1:{}", port);
    let mut stream =
        TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(3000))
            .expect("Failed to connect to sandboxed HTTP service");

    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut buffer = String::new();
    stream.read_to_string(&mut buffer).unwrap();

    println!("Sandboxed response received:\n{}", buffer);
    assert!(buffer.contains("200 OK"));
    assert!(buffer.contains("OK:SANDBOXED"));

    // Verify teardown
    let stopped = composer.stop_composer_app("sandboxed_test_service");
    assert!(stopped, "Composer service stop should succeed");

    let final_status = composer.get_app_status("sandboxed_test_service", Some(&manifest));
    assert!(!final_status.is_running, "Service should be stopped");
}
