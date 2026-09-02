use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    ".venv",
    "venv",
    "ENV",
    "__pycache__",
    ".pytest_cache",
    ".secrets",
    "node_modules",
];

fn excluded(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    EXCLUDED_DIRS.contains(&name)
        || name.ends_with(".pyc")
        || name.ends_with(".pyo")
        || name.ends_with(".db")
        || name.ends_with(".sqlite")
        || name.ends_with(".sqlite3")
        || name.ends_with(".tmp")
        || name.to_ascii_lowercase().contains(":zone.identifier")
}

fn copy_clean_tree(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        if excluded(&source_path) {
            continue;
        }
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_clean_tree(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            let should_copy = if !destination_path.exists() {
                true
            } else if let (Ok(m1), Ok(m2)) = (fs::metadata(&source_path), fs::metadata(&destination_path)) {
                m1.len() != m2.len() || m1.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH) > m2.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            } else {
                true
            };
            if should_copy {
                let _ = fs::copy(&source_path, &destination_path);
            }
        }
    }
    Ok(())
}

fn clean_destination_tree(destination: &Path) -> io::Result<()> {
    if !destination.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(destination)? {
        let entry = entry?;
        let p = entry.path();
        if excluded(&p) {
            if p.is_dir() {
                let _ = fs::remove_dir_all(&p);
            } else {
                let _ = fs::remove_file(&p);
            }
        } else if p.is_dir() {
            let _ = clean_destination_tree(&p);
        }
    }
    Ok(())
}

fn stage_app_resources() -> io::Result<()> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let source = manifest_dir.parent().unwrap().join("apps");
    let destination = manifest_dir.join("apps");

    println!("cargo:rerun-if-changed={}", source.display());
    let _ = clean_destination_tree(&destination);
    copy_clean_tree(&source, &destination)
}


fn main() {
    let _ = stage_app_resources();
    tauri_build::build();
}


