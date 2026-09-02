use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// WorkspaceManager oversees the ephemeral decrypted workspace lifecycle
#[derive(Clone, Debug)]
pub struct WorkspaceManager {
    workspace_root: PathBuf,
}

impl WorkspaceManager {
    pub fn new(data_root: &Path) -> Self {
        Self {
            workspace_root: data_root.join("secure_workspace"),
        }
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn app_dir(&self, app_id: &str) -> PathBuf {
        let safe_id: String = app_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.workspace_root.join("apps").join(safe_id)
    }

    pub fn app_secrets_dir(&self, app_id: &str) -> PathBuf {
        self.app_dir(app_id).join("secrets")
    }

    pub fn app_db_path(&self, app_id: &str, db_name: &str) -> PathBuf {
        self.app_dir(app_id).join(db_name)
    }

    /// Create secure workspace directory on disk with restricted 0700 permissions
    pub fn materialize_workspace(&self) -> Result<(), std::io::Error> {
        fs::create_dir_all(&self.workspace_root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(
                &self.workspace_root,
                fs::Permissions::from_mode(0o700),
            );
        }
        Ok(())
    }

    /// Unpack stored binary files into an application's private decrypted workspace
    pub fn unpack_app_files(
        &self,
        app_id: &str,
        files: &HashMap<String, Vec<u8>>,
    ) -> Result<(), std::io::Error> {
        self.materialize_workspace()?;
        let app_dir = self.app_dir(app_id);
        fs::create_dir_all(&app_dir)?;

        for (rel_path, content) in files {
            let target_path = app_dir.join(rel_path);
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = fs::File::create(&target_path)?;
            file.write_all(content)?;
            file.sync_all()?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(
                    &target_path,
                    fs::Permissions::from_mode(0o600),
                );
            }
        }
        Ok(())
    }

    /// Collect all files from an application's decrypted workspace for packaging into VaultData
    pub fn pack_app_files(&self, app_id: &str) -> Result<HashMap<String, Vec<u8>>, std::io::Error> {
        let mut files = HashMap::new();
        let app_dir = self.app_dir(app_id);
        if !app_dir.exists() {
            return Ok(files);
        }

        Self::collect_files_recursive(&app_dir, &app_dir, &mut files)?;
        Ok(files)
    }

    fn collect_files_recursive(
        base_dir: &Path,
        current_dir: &Path,
        out_map: &mut HashMap<String, Vec<u8>>,
    ) -> Result<(), std::io::Error> {
        if let Ok(entries) = fs::read_dir(current_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();

                // Skip socket files, locks, or cache directories
                if file_name.starts_with(".tmp") || file_name.ends_with(".sock") {
                    continue;
                }

                if path.is_dir() {
                    Self::collect_files_recursive(base_dir, &path, out_map)?;
                } else if path.is_file() {
                    if let Ok(rel) = path.strip_prefix(base_dir) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        if let Ok(content) = fs::read(&path) {
                            out_map.insert(rel_str, content);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Securely wipe/shred all plaintext files and remove the workspace directory tree
    pub fn shred_and_remove_all(&self) -> Result<(), std::io::Error> {
        if !self.workspace_root.exists() {
            return Ok(());
        }

        Self::secure_wipe_directory(&self.workspace_root)?;
        let _ = fs::remove_dir_all(&self.workspace_root);
        Ok(())
    }

    fn secure_wipe_directory(dir: &Path) -> Result<(), std::io::Error> {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let _ = Self::secure_wipe_directory(&path);
                    let _ = fs::remove_dir(&path);
                } else if path.is_file() {
                    Self::secure_wipe_file(&path);
                }
            }
        }
        Ok(())
    }

    fn secure_wipe_file(path: &Path) {
        if let Ok(meta) = fs::metadata(path) {
            let len = meta.len() as usize;
            if len > 0 {
                if let Ok(mut file) = fs::OpenOptions::new().write(true).open(path) {
                    let zeroes = vec![0u8; len.min(65536)];
                    let mut written = 0;
                    while written < len {
                        let to_write = (len - written).min(zeroes.len());
                        if file.write_all(&zeroes[..to_write]).is_err() {
                            break;
                        }
                        written += to_write;
                    }
                    let _ = file.sync_all();
                }
            }
        }
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_lifecycle_and_shredding() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let ws = WorkspaceManager::new(temp_dir.path());

        let app_id = "test_app_42";
        let mut sample_files = HashMap::new();
        sample_files.insert(
            "mac_finder.db".to_string(),
            b"SQLITE_DB_BINARY_CANARY_TEST".to_vec(),
        );
        sample_files.insert(
            "secrets/db_key.txt".to_string(),
            b"SECRET_KEY_CANARY_TEST".to_vec(),
        );

        // 1. Unpack files into workspace
        ws.unpack_app_files(app_id, &sample_files)
            .expect("Unpack must succeed");
        assert!(ws.app_db_path(app_id, "mac_finder.db").exists());
        assert!(ws.app_secrets_dir(app_id).join("db_key.txt").exists());

        // 2. Pack files back from workspace
        let packed = ws.pack_app_files(app_id).expect("Pack must succeed");
        assert_eq!(
            packed.get("mac_finder.db"),
            Some(&b"SQLITE_DB_BINARY_CANARY_TEST".to_vec())
        );
        assert_eq!(
            packed.get("secrets/db_key.txt"),
            Some(&b"SECRET_KEY_CANARY_TEST".to_vec())
        );

        // 3. Shred and wipe workspace
        ws.shred_and_remove_all().expect("Wipe must succeed");
        assert!(!ws.workspace_root().exists());
    }
}
