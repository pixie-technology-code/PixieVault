use pixievault_lib::app_manager::{allocate_ephemeral_port, AppManifest, VaultComposer};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_ephemeral_port_allocation() {
    let port1 = allocate_ephemeral_port().expect("Failed to allocate port 1");
    let port2 = allocate_ephemeral_port().expect("Failed to allocate port 2");

    assert!(
        port1 > 1024,
        "Port should be in unprivileged user range (>1024)"
    );
    assert!(
        port2 > 1024,
        "Port should be in unprivileged user range (>1024)"
    );
}

#[test]
fn test_composer_manifest_parsing_and_template_resolution() {
    let manifest_json = r#"{
      "app_id": "mikrotik_fleet_test",
      "name": "MikroTik Fleet Test",
      "version": "1.0.0",
      "entrypoint": "http://127.0.0.1:{{services.backend.port}}",
      "composer": {
        "version": "1",
        "services": {
          "backend": {
            "command": ["python3", "app.py"],
            "working_dir": "automation",
            "port": "auto",
            "environment": {
              "PORT": "{{port}}",
              "FLASK_ENV": "production"
            },
            "healthcheck": {
              "endpoint": "/health",
              "interval_ms": 100,
              "timeout_ms": 3000
            }
          }
        }
      }
    }"#;

    let manifest: AppManifest =
        serde_json::from_str(manifest_json).expect("Failed to parse composer manifest");

    assert_eq!(manifest.app_id, "mikrotik_fleet_test");
    assert!(manifest.has_composer());

    let composer = manifest.composer.as_ref().unwrap();
    assert_eq!(composer.version, "1");
    let backend_svc = composer.services.get("backend").unwrap();
    assert_eq!(backend_svc.command, vec!["python3", "app.py"]);
    assert_eq!(backend_svc.port, "auto");
    assert_eq!(
        backend_svc.healthcheck.as_ref().unwrap().endpoint,
        "/health"
    );

    // Test URL template substitution
    let mut port_map = HashMap::new();
    port_map.insert("backend".to_string(), 54321);

    let resolved_url = manifest.resolve_entrypoint(&port_map);
    assert_eq!(resolved_url, "http://127.0.0.1:54321");
}

#[test]
fn test_vault_composer_lifecycle_and_shutdown() {
    let composer = VaultComposer::new();

    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let app_dir = temp_dir.path().join("dummy_app");
    fs::create_dir_all(&app_dir).unwrap();

    let manifest_json = r#"{
      "app_id": "dummy_static_app",
      "name": "Dummy Static App",
      "version": "1.0.0",
      "entrypoint": "index.html"
    }"#;
    fs::write(app_dir.join("manifest.json"), manifest_json).unwrap();
    fs::write(app_dir.join("index.html"), "<h1>Static App</h1>").unwrap();

    let manifest = AppManifest::load_from_file(app_dir.join("manifest.json")).unwrap();

    // 1. Launch static composer app (zero services)
    let status = composer
        .start_composer_app(&manifest, &app_dir, None)
        .expect("Failed to start app");
    assert!(status.is_running);
    assert!(status.entrypoint_url.starts_with("http://127.0.0.1:"));
    assert!(status.entrypoint_url.ends_with("/index.html"));

    // Verify HTTP response from loopback server
    let resp = reqwest::blocking::get(&status.entrypoint_url).expect("HTTP GET failed");
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().unwrap(), "<h1>Static App</h1>");

    // 2. Query status
    let query_status = composer.get_app_status("dummy_static_app", Some(&manifest));
    assert!(query_status.is_running);
    assert_eq!(query_status.entrypoint_url, status.entrypoint_url);

    // 3. Stop app
    let stopped = composer.stop_composer_app("dummy_static_app");
    assert!(stopped);
    let after_status = composer.get_app_status("dummy_static_app", Some(&manifest));
    assert!(!after_status.is_running);

    // 4. Global teardown (zero-trust lock)
    composer.stop_all();
}

#[test]
fn test_vault_composer_transactional_rollback_on_healthcheck_failure() {
    let composer = VaultComposer::new();
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let app_dir = temp_dir.path().join("failing_service_app");
    fs::create_dir_all(&app_dir).unwrap();

    // Write a Python server that will fail or exit immediately
    let server_py = r#"
import sys
import time
time.sleep(0.1)
sys.exit(42)
"#;
    fs::write(app_dir.join("fail_server.py"), server_py).unwrap();

    let manifest_json = r#"{
      "app_id": "failing_test_app",
      "name": "Failing Test App",
      "version": "1.0.0",
      "entrypoint": "http://127.0.0.1:{{services.web.port}}/",
      "composer": {
        "version": "1",
        "services": {
          "web": {
            "command": ["python3", "fail_server.py"],
            "port": "auto",
            "healthcheck": {
              "endpoint": "/",
              "interval_ms": 100,
              "timeout_ms": 2000
            }
          }
        }
      }
    }"#;
    fs::write(app_dir.join("manifest.json"), manifest_json).unwrap();
    let manifest = AppManifest::load_from_file(app_dir.join("manifest.json")).unwrap();

    let result = composer.start_composer_app(&manifest, &app_dir, None);
    assert!(
        result.is_err(),
        "Service that exits immediately must fail startup"
    );

    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("terminated") || err_msg.contains("failed") || err_msg.contains("status"),
        "Error message should contain diagnostic failure: {}",
        err_msg
    );

    // Verify rollback left zero running services
    let query_status = composer.get_app_status("failing_test_app", Some(&manifest));
    assert!(
        !query_status.is_running,
        "No running state should exist after rollback"
    );
}

#[test]
fn test_vault_composer_spawns_real_python_service() {
    let composer = VaultComposer::new();
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let app_dir = temp_dir.path().join("py_service_app");
    fs::create_dir_all(&app_dir).unwrap();

    // Write a tiny Python HTTP server
    let server_py = r#"
import http.server
import socketserver
import os
import sys

port = int(os.environ.get('PORT', 8080))
class Handler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-type', 'text/html')
        self.end_headers()
        self.wfile.write(b'<h1>OK</h1>')

with socketserver.TCPServer(('127.0.0.1', port), Handler) as httpd:
    httpd.serve_forever()
"#;
    fs::write(app_dir.join("server.py"), server_py).unwrap();

    let manifest_json = r#"{
      "app_id": "python_test_app",
      "name": "Python Test App",
      "version": "1.0.0",
      "entrypoint": "http://127.0.0.1:{{services.web.port}}/",
      "composer": {
        "version": "1",
        "services": {
          "web": {
            "command": ["python3", "server.py"],
            "port": "auto",
            "environment": {
              "PORT": "{{port}}"
            },
            "healthcheck": {
              "endpoint": "/",
              "interval_ms": 100,
              "timeout_ms": 5000
            }
          }
        }
      }
    }"#;
    fs::write(app_dir.join("manifest.json"), manifest_json).unwrap();

    let manifest = AppManifest::load_from_file(app_dir.join("manifest.json")).unwrap();

    // Launch service
    let status = composer
        .start_composer_app(&manifest, &app_dir, None)
        .expect("Failed to start python service");
    assert!(status.is_running);
    assert!(status.entrypoint_url.starts_with("http://127.0.0.1:"));
    assert!(status.services.contains_key("web"));
    assert!(status.services.get("web").unwrap().is_running);

    composer.stop_all();
}

#[test]
fn test_vault_composer_launches_mikrotik_fleet_live() {
    let composer = VaultComposer::new();
    let runtime_root = tempfile::tempdir().expect("Failed to create isolated runtime directory");

    // Find mikrotik manifest across possible cwd paths
    let candidates = [
        PathBuf::from("apps/mikrotik_fleet/manifest.json"),
        PathBuf::from("../apps/mikrotik_fleet/manifest.json"),
    ];

    let manifest_path = candidates.into_iter().find(|p| p.exists());
    if manifest_path.is_none() {
        eprintln!("[test_vault_composer_launches_mikrotik_fleet_live] Skipping: MikrotikFleet manifest not in candidate paths");
        return;
    }
    let manifest_path = manifest_path.unwrap();
    let app_dir = manifest_path.parent().unwrap();

    let manifest =
        AppManifest::load_from_file(&manifest_path).expect("Failed to load mikrotik manifest");

    // Provision runtime environment (dependency fingerprinting)
    if let Some(ref composer_cfg) = manifest.composer {
        if let Some(backend_cfg) = composer_cfg.services.get("backend") {
            let working_dir = manifest
                .resolve_service_working_dir(app_dir, "backend")
                .expect("Failed to resolve backend working directory");

            let runtime_dir = runtime_root.path().join("runtimes").join("backend");
            pixievault_lib::app_manager::PythonProvisioningManager::provision_environment_in(
                &working_dir,
                &runtime_dir,
                backend_cfg.requirements.as_deref(),
                false,
            )
            .expect("Provisioning mikrotik environment must succeed");
        }
    }

    let status = composer
        .start_composer_app(&manifest, app_dir, Some(runtime_root.path()))
        .expect("Failed to start Mikrotik Composer service");

    assert!(status.is_running);
    assert!(status.entrypoint_url.starts_with("http://127.0.0.1:"));
    assert!(status.services.contains_key("backend"));
    assert!(status.services.get("backend").unwrap().is_running);

    // Clean teardown
    composer.stop_all();
    let after_status = composer.get_app_status("mikrotik_fleet_mgr", Some(&manifest));
    assert!(!after_status.is_running);
}

#[test]
fn test_vault_composer_serves_cairn_dead_reckoning_live() {
    let composer = VaultComposer::new();

    let candidates = [
        PathBuf::from("apps/cairn_dead_reckoning/manifest.json"),
        PathBuf::from("../apps/cairn_dead_reckoning/manifest.json"),
    ];

    let manifest_path = candidates.into_iter().find(|p| p.exists());
    if manifest_path.is_none() {
        eprintln!("[test_vault_composer_serves_cairn_dead_reckoning_live] Skipping: Cairn manifest not in candidate paths");
        return;
    }
    let manifest_path = manifest_path.unwrap();
    let app_dir = manifest_path.parent().unwrap();

    let manifest =
        AppManifest::load_from_file(&manifest_path).expect("Failed to load Cairn manifest");

    let status = composer
        .start_composer_app(&manifest, app_dir, None)
        .expect("Failed to start Cairn static app server");

    assert!(status.is_running);
    assert!(status.entrypoint_url.starts_with("http://127.0.0.1:"));
    assert!(status.entrypoint_url.ends_with("/index.html"));

    // Verify index.html content
    let index_resp = reqwest::blocking::get(&status.entrypoint_url).expect("HTTP GET index.html failed");
    assert_eq!(index_resp.status(), 200);
    assert_eq!(index_resp.headers().get("content-type").unwrap(), "text/html; charset=utf-8");
    let index_body = index_resp.text().unwrap();
    assert!(index_body.contains("Cairn: Dead Reckoning"));

    // Verify cairn_app.js content
    let js_url = status.entrypoint_url.replace("index.html", "cairn_app.js");
    let js_resp = reqwest::blocking::get(&js_url).expect("HTTP GET cairn_app.js failed");
    assert_eq!(js_resp.status(), 200);
    assert_eq!(js_resp.headers().get("content-type").unwrap(), "application/javascript; charset=utf-8");

    // Clean teardown
    composer.stop_all();
    let after_status = composer.get_app_status("cairn_dead_reckoning", Some(&manifest));
    assert!(!after_status.is_running);
}
