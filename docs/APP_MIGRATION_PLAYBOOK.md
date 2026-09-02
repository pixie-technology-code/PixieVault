# PixieVault Application Migration & Packaging Playbook

This playbook defines the architecture, storage virtualization, and packaging workflow to bring any web application, dashboard, or developer script into **PixieVault** with **zero modifications to original application source code**.

Before migrating an unfamiliar application, run the read-only compatibility gate in `PRE_MIGRATION_PROMPT.md`. Proceed to `MIGRATION_PROMPT.md` only after the gate returns `GO` or every condition of a `CONDITIONAL_GO` has been met. A `NO_GO_HOST_GAP` should become a generic host capability proposal, never an application-specific branch in trusted host code.

---

## 1. PixieVault Application Architecture

A PixieVault guest application is a self-contained, encapsulated single-page application (SPA) running within the native Rust host wrapper.

### Key Guarantees:
- **Zero-Trust Encryption**: Guest apps never manage raw cryptographic keys or disk file handles. All persistence is transparently encrypted via **AES-256-GCM** with **Argon2id** key derivation into the host `.pvlt` database.
- **Inter-App Telemetry Bus**: Apps can expose telemetry metrics (`registerDataExporter`) and query adjacent applications (`requestCrossAppData`) safely through the Rust memory broker.
- **Multi-Platform Native UI**: Native window controls, OS biometric prompts (Windows Hello, Touch ID, Linux PAM), and top-level OS menu bars are provided by the host wrapper.

---

## 2. Directory Structure of a PixieVault App

Every application in PixieVault requires only a `manifest.json` and static web assets:

```
my_app_name/
├── manifest.json              <-- Required: App identification, entrypoint & permissions
├── index.html                 <-- Application entrypoint
├── app.js                     <-- Application logic (or original web code)
├── styles.css                 <-- Application styles
└── assets/                    <-- Icons, images, fonts, data files
```

---

## 3. The `manifest.json` Specification

```json
{
  "app_id": "sample_dashboard_app",
  "name": "Sample Dashboard Application",
  "version": "1.0.0",
  "min_pixievault_version": "0.1.0",
  "description": "Interactive local telemetry, configuration generator, and metrics viewer",
  "entrypoint": "index.html",
  "author": "Pixie Technology",
  "permissions": {
    "requested_read": ["*:*"],
    "requested_write": []
  },
  "theme_compatibility": {
    "supports_dark_mode": true,
    "supports_light_mode": true
  }
}
```

`min_pixievault_version` is required and uses SemVer. It names the oldest PixieVault release that provides all host capabilities used by the app—not the release on which the app happened to be built. See `MIGRATION_PROMPT.md` for the append-only capability history and selection policy. Installers must stop before making changes when the running host is older, then offer an explicit Vault update or cancellation.

---

## 4. Packaging Categories & Patterns

### Pattern A: Static HTML / JavaScript Single Page Apps
*Best for: Dashboards, calculators, simulators, financial models, React/Vue/Svelte SPAs.*

- **Zero-Touch Integration**:
  1. Drop the built web assets (`index.html`, `bundle.js`, `style.css`) into a directory inside `apps/` (or package into `.pvpkg`).
  2. Include `<script src="../shared/wrapper-bridge.js"></script>` or use standard `localStorage`.
  3. Run `node scripts/package-app.js ./my_app` to generate an all-in-one `.pvpkg` package.

---

### Pattern B: Python / Backend Web Tools (e.g. Flask / FastAPI)
*Best for: Python scripts with embedded HTML templates or local microservices (like `my_tool/automation/web_ui.py`).*

There are two non-invasive approaches to package Python-driven tools:

#### Approach 1: Client-Side Adapter (Recommended for 100% Portability)
- **Concept**: Extract the embedded HTML template (`HTML_TEMPLATE`) from the Python script into a clean `index.html`.
- **Advantages**:
  - 100% client-side execution with **zero Python dependency** on target machines.
  - Automatically routes state into PixieVault's AES-256-GCM encrypted database.
  - Instant zero-latency workspace switching.
  - Exports standard YAML / JSON configurations on demand.

#### Approach 2: Native Sidecar Process
- **Concept**: The Rust host wrapper executes `python3 automation/web_ui.py` as a managed background child process and mounts the webview to the local loopback socket.
- **Advantages**: Runs existing server-side Python libraries without refactoring.

---

## 5. Storage Virtualization & Persistence

Guest applications persist state through either:

### Method 1: The Native IPC Bridge (`PixieVaultNative`)
```javascript
// Load decrypted app state from vault
const state = await window.PixieVaultNative.loadAppData("my_app_id");

// Save state into encrypted vault
await window.PixieVaultNative.saveAppData({
  items: myItemList,
  lastUpdated: Date.now()
}, "my_app_id");
```

### Method 2: Standard Web `localStorage` Virtualization
If the original application uses standard `localStorage.setItem("key", value)` and `localStorage.getItem("key")`, PixieVault automatically synchronizes `localStorage` with the encrypted `.pvlt` container when the vault unlocks or locks.

---

## 6. Inter-App Telemetry Bus Integration

Apps can share real-time metrics with other installed apps:

### 1. Exporting Metrics to the Bus:
```javascript
window.PixieVaultNative.registerDataExporter(() => ({
  totalItems: itemList.length,
  activeCount: itemList.filter(d => d.status === "active").length,
  hostId: "node-01"
}));
```

### 2. Querying Adjacent Installed Apps:
```javascript
// Query metric from another installed workspace
const val = await window.PixieVaultNative.requestCrossAppData(
  "sample_analytics_v1",
  "activeItemCount",
  "my_app_id"
);
```

---

## 7. 1-Click Automated Packaging CLI

You can package any application folder into a `.pvpkg` archive using the bundled CLI:

```bash
# Basic packaging (auto-detects manifest)
node scripts/package-app.js ./path/to/my_app

# Custom metadata packaging
node scripts/package-app.js ./my_app \
  --name "Sample Application" \
  --id "sample_app" \
  --version "1.0.0" \
  --entrypoint "index.html" \
  --output "./sample_app.pvpkg"
```

The resulting `.pvpkg` file can be:
- Copied to any air-gapped machine or flash drive.
- Imported into PixieVault via **File > Open App Bundle / Dashboard** (<kbd>Ctrl+O</kbd>).
- Distributed via GitHub Releases with Ed25519 digital signatures.
