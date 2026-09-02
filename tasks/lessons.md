# Lessons Learned

<!-- Append-only log. Never delete entries. Each entry: date + lesson + rule to prevent recurrence. -->

## Format

- **YYYY-MM-DD**: [Verbatim correction or lesson]
  - **Rule**: [What to do differently next time]

- **2026-09-01**: "This wrapper is everything. Its the entire interface, there is no opening a web browser."
  - **Rule**: PixieVault is strictly a 100% native desktop shell application where the Rust wrapper IS the entire user experience. Never present or design it as a web browser app or suggest browser fallbacks. The native Rust shell manages all windows, native menus, OS biometric dialogs, app module mounting, and encrypted persistence directly on the operating system.

- **2026-09-01**: PowerShell ExecutionPolicy and CMD.EXE UNC path restrictions (`\\wsl.localhost\...`).
  - **Rule**: In `.cmd` / `.bat` scripts on Windows accessing WSL UNC shares, always wrap with `pushd "%~dp0"` and `popd` to map a temporary drive letter so Windows CMD and CLI tools find the project directory.

- **2026-09-01**: `frontendDist` directory boundaries in Tauri webview serve as the web root `/`.
  - **Rule**: Relative script paths like `../shared/...` outside `frontendDist` will 404 in the webview runtime. Always keep bundled bridge scripts and styles directly within `frontendDist` (or configure `frontendDist` to the parent root), and automate checking via `tests/validate-dist-assets.js`.

- **2026-09-01**: Native Composer Service Healthcheck and Cross-Platform Python Execution ("Healthcheck failed for service 'backend': Timed out").
  - **Rule**: 
    1. **Fast Failure Detection**: In process supervision (`sidecar.rs`), `poll_child_health` must check `child.try_wait()` inside the probe loop to detect premature exits immediately with exit codes instead of waiting the full timeout.
    2. **Cross-Platform Executable Resolution**: On Windows, `python3` command does not exist by default or invokes the Windows Store stub; dynamically resolve `python` -> `py` -> `python3` across platforms.
    3. **HTTP Healthchecks with Redirects**: Endpoints protected by `@login_required` return `302 Found` to `/login`. Treat both `2xx` and `3xx` as healthy web server responses.
    4. **Automated Live Testing**: Maintain an automated live service orchestration test (`test-live-host.js` and `test_vault_composer_launches_live`) in `./test-all.sh`, `./test-all.ps1`, and `./test-all.bat`.
    5. **OS Pipe Buffer Drainage**: When spawning child processes with `Stdio::piped()`, always spawn non-blocking background reader threads in Rust to continuously drain `stdout` and `stderr`. Otherwise, once the OS pipe buffer (4KB–64KB) fills with daemon logs, child processes hang indefinitely in write syscalls.
    6. **SQLite Concurrency & WAL Mode**: Ensure SQLite connections in guest microservices use `timeout=30.0, check_same_thread=False` with `PRAGMA journal_mode=WAL;` and `PRAGMA busy_timeout=30000;` to prevent `database is locked` contention across background threads and processes.
    7. **Canonical Script Arguments**: Resolve relative script paths (e.g. `app.py`) to canonical absolute paths before spawning child processes to avoid UNC working directory limitations on Windows.
    8. **Bubblewrap `--die-with-parent` and `PYTHONUNBUFFERED`**: Under Linux Bubblewrap namespaces, always pass `--die-with-parent` to ensure child daemon trees terminate immediately when the host stops. Always pass `PYTHONUNBUFFERED=1` so startup errors and log streams flush across pipes in real time.
    9. **Windows Python Standard I/O Codec & build.rs File Locking**:
       - **Symptom**: `UnicodeEncodeError` on Windows during backend startup and `Access is denied` during `cargo build` in `build.rs`.
       - **Rule**: Always inject `PYTHONIOENCODING="utf-8"` and `PYTHONUTF8="1"` into Python subprocess environments. Replace destructive directory manipulation in `build.rs` with native Tauri v2 `"resources"` declarations.

- **2026-09-01**: Windows Host Composer Service Launch & UNC Network Share SQLite Locking ("Healthcheck failed on port ...: Timed out waiting for service").
  - **Rule**:
    1. **Verbatim Path Normalization**: Rust's `Path::canonicalize()` on Windows prepends extended-length prefixes (`\\?\` and `\\?\UNC\`). Passing `\\?\` paths to child process arguments (`python.exe`, `node.exe`) or setting working directories breaks C-runtime `_chdir` and Python module imports. Always sanitize paths with `canonicalize_clean` / `clean_path` to strip `\\?\` and convert `\\?\UNC\server\share` to `\\server\share`.
    2. **SQLite Network Share Locking**: SQLite WAL mode (`PRAGMA journal_mode=WAL;`) requires shared memory (`.shm`) files that hang or fail over SMB/UNC network shares (e.g. `\\wsl.localhost\...`). In guest microservices, always route database storage to `VAULT_STORAGE_DIR` (local SSD), and dynamically detect UNC paths (`\\...` / `//...`) to fall back to `PRAGMA journal_mode=DELETE;`.
    3. **Windows Python Execution Discovery**: On Windows, prioritize the standard Python Launcher (`py`) over `python` to prevent invoking the 0-byte Microsoft Store execution alias stub, and validate all candidates by executing `--version`.

- **2026-09-01**: Application Migration & Sandboxed Dependency Isolation (`paramiko` vs `sshpass` CLI fallbacks).
  - **Rule**:
    1. **Self-Contained Dependencies**: When migrating guest applications, audit all `try/import` blocks in the upstream codebase that fall back to shell CLI subprocesses (e.g., `import paramiko` falling back to `sshpass`/`scp`/`ssh`). In a sandboxed desktop environment and on Windows, host CLI binaries do not exist. Always declare all network transport, protocol, and cryptographic libraries in `requirements.txt` or `package.json` so the sandboxed virtualenv is 100% self-contained.
    2. **Immutable Upstream Source**: Never mutate the upstream `<SOURCE_DIRECTORY>`. Copy cleanly into `apps/<APP_ID>/` and stage with dedicated vector SVG assets, hardened `manifest.json`, and isolated mutable storage routing.
    3. **Portable `.pvpkg` Packaging Gate**: Produce and verify a clean air-gapped `.pvpkg` package bundle using `PackageBundler::export_package` ensuring zero `.venv`, `.secrets`, `.db`, or `__pycache__` artifacts are leaked into distribution packages.

- **2026-09-01**: Static Guest Applications & Webview Sibling Directory Boundaries (`frontendDist` isolation).
  - **Rule**:
    1. **Dynamic Runtime Webview Serving**: In Tauri, `frontendDist` compiles only the host shell into the build artifact. Static web applications (`apps/<APP_ID>/index.html`) mounted at runtime from disk, `.pvpkg` packages, or USB drives cannot be accessed via relative URI navigation from `frontendDist`.
    2. **Universal Loopback Orchestration**: `VaultComposer` must manage all guest applications uniformly. For pure static web apps (no backend microservices), `VaultComposer` allocates an ephemeral loopback port and binds a lightweight embedded HTTP server (`serve_static_app`) to serve the app's directory over `http://127.0.0.1:<port>/index.html` with correct MIME types, CORS headers, and path traversal protection. This guarantees zero 404s across all environments.
- **2026-09-01**: Native OS Folder Picker vs Package File Picker (`rfd::AsyncFileDialog` filtering out `.pvpkg` files in folder selection mode).
  - **Rule**:
    1. **Folder Picker OS Filter Invariant**: When `pick_folder()` is invoked on Windows or Linux, the operating system file dialog strictly enforces directory navigation and hides all regular files, including `.pvpkg` archives. If a user navigates to `dist/`, the directory appears blank ("No items match your search").
    2. **Dedicated UI & IPC Picker Separation**: Always provide distinct, explicit options and native IPC handlers for **Import `.pvpkg` Package Bundle** (`pick_file()` filtered to `*.pvpkg, *.zip`) and **Mount Unpacked App Folder** (`pick_folder()`). Ensure universal file pickers accept both package archives and `manifest.json` entries.

- **2026-09-01**: Testing Password & Credential Convention ("For testing, please record I'm using the password: MasterPassword").
  - **Rule**: In all interactive tests, automated test fixtures requiring a default passphrase, and manual test sessions where a configured master password is required, standard test operations use `MasterPassword` (or initial unconfigured blank for first-run tests).

- **2026-09-01**: Windows Hello, CNG Key Storage Providers, and Biometric Consent Verification.
  - **Rule**:
    1. **WinRT UserConsentVerifier for Biometrics**: For native Windows 10/11 desktop applications, use Microsoft's official `UserConsentVerifier::RequestVerificationAsync` API in `windows::Security::Credentials::UI` to trigger the interactive Windows Hello biometric/PIN prompt modal.
    2. **Hardware/TPM-Backed DPAPI Envelope Encryption**: In envelope encryption on Windows, wrap the random 256-bit Vault Master Key using Windows DPAPI (`CryptProtectData` / `CryptUnprotectData` with `CRYPTPROTECT_UI_FORBIDDEN` and vault-scoped entropy `PixieVault::<vault_id>::<device_id>`). This provides hardware/TPM-backed, user-bound key protection that integrates seamlessly with Windows Hello user consent verification.
    3. **CNG Key Naming Invariant**: In Windows CNG, key names containing forward slashes `/` or backslashes `\` cause Key Storage Providers to interpret the prefix as a Windows Security Identifier (SID) path, returning `0x80070539` (`ERROR_INVALID_SID`). Always sanitize key names to use alphanumeric characters with underscores or hyphens (e.g. `PixieVault_<vault_id>_<device_id>`).
    4. **`windows-rs` Handle Passing & Imports**: In `windows-rs`, handle types (`NCRYPT_KEY_HANDLE`, `NCRYPT_PROV_HANDLE`) implement `windows_core::Param<NCRYPT_HANDLE>` directly. Never pass `.0` (raw `usize`) into API functions expecting a handle. `LocalFree` and `HLOCAL` reside in `windows::Win32::Foundation`.

