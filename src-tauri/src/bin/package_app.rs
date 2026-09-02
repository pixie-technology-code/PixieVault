use pixievault_lib::app_manager::PackageBundler;
use std::env;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();
    let app_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("apps/mikrotik_fleet")
    };

    let output_path = if args.len() > 2 {
        PathBuf::from(&args[2])
    } else {
        let manifest_path = app_path.join("manifest.json");
        let manifest_content = std::fs::read_to_string(&manifest_path)
            .expect("manifest.json must exist in app directory");
        let manifest_json: serde_json::Value = serde_json::from_str(&manifest_content)
            .expect("manifest.json must be valid JSON");
        let app_id = manifest_json["app_id"]
            .as_str()
            .unwrap_or("app_bundle");
        PathBuf::from(format!("dist/{}.pvpkg", app_id))
    };

    println!("[PixieVault Package Bundler] Packaging {} -> {}", app_path.display(), output_path.display());
    PackageBundler::export_package(&app_path, None, &output_path)
        .expect("Failed to export .pvpkg bundle");
    println!("✓ Package bundle created successfully at {}", output_path.display());
}
