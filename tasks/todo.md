# PixieVault Tasks & Progress Checklist

## 12. Application Migration & Runtime Architecture
- [x] Phase 1: Clean legacy demo fixtures and remove root static files <!-- id: 1201 -->
- [x] Phase 2: Copy guest application into isolated workspace <!-- id: 1202 -->
- [x] Phase 3: Create PixieVault `manifest.json`, vector SVG icon, and integrate `wrapper-bridge.js` / storage hooks <!-- id: 1203 -->
- [x] Phase 4: Stage package bundle and generate `.pvpkg` distribution artifact <!-- id: 1204 -->
- [x] Phase 5: Update catalogs and automated test suites <!-- id: 1205 -->
- [x] Phase 6: Run full verification across all 4 tiers (`./test-all.sh`) <!-- id: 1206 -->

## 13. Public Repository Preparation & GitHub Push
- [x] Phase 1: Update `.gitignore` to strictly exclude all guest apps (`apps/`, `dist/`), temporary databases, and caches <!-- id: 1301 -->
- [x] Phase 2: Create high-impact, professional `README.md` showcasing architecture, features, security model, and quickstart <!-- id: 1302 -->
- [x] Phase 3: Stage all core wrapper files and verify `git status` contains zero app packages or leaked artifacts <!-- id: 1303 -->
- [x] Phase 4: Configure remote `https://github.com/pixie-technology-code/PixieVault` and push <!-- id: 1304 -->

## 14. Cryptographic Security & Encrypted Workspace Architecture
- [x] Phase 1: Cryptographic Engine & Per-Vault Random Salt Derivation <!-- id: 1401 -->
- [x] Phase 2: Host Encrypted Workspace Manager & Lifecycle (`WorkspaceManager`) <!-- id: 1402 -->
- [x] Phase 3: Microservice Key Injection & Permission Hardening (`db.py` & sidecar env) <!-- id: 1403 -->
- [x] Phase 4: WebView Storage Containment & Bridge Hardening <!-- id: 1404 -->
- [x] Phase 5: Auto-Lock Supervision & Zero-Plaintext Teardown Enforcement <!-- id: 1405 -->
- [x] Phase 6: Disk-Inventory Acceptance Tests & 4-Tier Verification (`./test-all.sh`) <!-- id: 1406 -->

## 15. Windows Hello & Envelope Encryption Architecture
- [x] Phase 1: Dependencies & Platform Crate Configuration (`Cargo.toml`) <!-- id: 1501 -->
- [x] Phase 2: Platform Protector Abstraction & Mock Provider (`protector.rs`, `windows_hello.rs`) <!-- id: 1502 -->
- [x] Phase 3: Versioned Envelope Encryption Format (v3) (`vault_crypto.rs`, `vault.rs`) <!-- id: 1503 -->
- [x] Phase 4: Explicit Enrollment Lifecycle & Host Commands (`commands/mod.rs`, `lib.rs`) <!-- id: 1504 -->
- [x] Phase 5: Legacy Credential Migration & Safe File Shredding (`commands/mod.rs`) <!-- id: 1505 -->
- [x] Phase 6: Host UX & Lock Screen Polish (`host/`, `wrapper-bridge.js`) <!-- id: 1506 -->
- [x] Phase 7: Security Isolation, Adversarial Tests & 4-Tier Verification (`./test-all.sh`) <!-- id: 1507 -->

---

## 16. Native Menu System & Automated Menu Test Coverage
- [x] Phase 1: Audit all 26 native menu items across 6 categories in `src-tauri/src/menu.rs` <!-- id: 1601 -->
- [x] Phase 2: Add interactive modals & overlays for Help (About, Docs, Ed25519 Signatures), Security (Change Passphrase), and Toast notifications <!-- id: 1602 -->
- [x] Phase 3: Implement 100% handler coverage in `host/shell.js` for File, Security, Apps, Storage, View, and Help <!-- id: 1603 -->
- [x] Phase 4: Create automated JS menu dispatch test (`tests/test-menus.js`) extracting and verifying all declared menu IDs <!-- id: 1604 -->
- [x] Phase 5: Create Rust menu structure and event dispatch test (`src-tauri/tests/menu_tests.rs`) <!-- id: 1605 -->
- [x] Phase 6: Integrate menu tests into 4-tier runners (`./test-all.sh`, `./test-all.ps1`, `./test-all.bat`) and pass 100% <!-- id: 1606 -->

---

## Testing Reference & Credentials
- **Testing Master Password**: `MasterPassword`
- **Initial Setup Mode**: Unsecured / blank default during first run (auto-prompts user to set master passphrase)
