# APPBUILD.md — PixieVault Ecosystem & Native Packaging Specification

Welcome to the **PixieVault Application Ecosystem Specification**. This document outlines the multi-platform packaging standards, native menuing conventions, OS biometric security protocols, and manifest requirements for building or integrating applications to run inside the **PixieVault Native Host Environment**.

---

## 1. Overview & Multi-Platform Packaging

**PixieVault** is distributed as a first-class native application across Windows, macOS, and Linux:

- **Windows**: Portable `.exe` and NSIS setup installer with Windows Hello integration.
- **macOS**: Native `.app` bundle and `.dmg` installer with Touch ID / Face ID.
- **Linux**: Universal **AppImage** (standalone portable binary for any distro), **Flatpak** sandboxed package, and native `.deb` / `.rpm` packages.

---

## 2. Pluggable Guest App Modules

Applications inside PixieVault (*Dummy Finance App*, *Dummy IoT Monitor*, *Dummy Analytics Studio*) are pure domain modules running inside the native host shell:
- No web browser dependencies.
- No faux navigation menus or fake login screens.
- Zero-trust encrypted persistence strictly through `window.PixieVaultNative`.
- Brokered inter-app telemetry bus for cross-application communication.

---

## 3. App Manifest (`manifest.json`)

Every app package imported into PixieVault must contain a `manifest.json` file in its root directory:

```json
{
  "app_id": "dummy_planner_app",
  "name": "Dummy Planner Application",
  "version": "1.0.0",
  "min_pixievault_version": "0.1.0",
  "description": "High-fidelity domain modeling and local offline analysis",
  "entrypoint": "index.html",
  "update_url": "https://updates.example.local/apps/dummy_planner/manifest.json",
  "author": "Pixie Technology",
  "permissions": {
    "requested_read": ["metricA", "metricB", "metricC"],
    "requested_write": []
  },
  "theme_compatibility": {
    "supports_dark_mode": true,
    "supports_light_mode": true,
    "custom_accent_override": null
  }
}
```

`min_pixievault_version` is required SemVer and must be the oldest host release providing the capabilities the app actually uses. It is not automatically the current PixieVault version. The migration prompt maintains the append-only capability history used to select this floor.

---

## 4. Packaging Directory Layout

```
PixieVault/
├── src-tauri/                        # Native Rust Host Shell
│   ├── Cargo.toml                    # Native dependencies
│   ├── tauri.conf.json               # Multi-platform bundle configuration
│   ├── FlatpakManifest.json          # Linux Flatpak specification
│   └── icons/                        # Cross-platform icons (.ico, .icns, .png)
├── apps/                             # Installed Guest Application Workspaces
│   ├── app_alpha/                    # Domain App: Alpha Studio
│   ├── app_beta/                     # Domain App: Beta Planner
│   └── app_gamma/                    # Domain App: Gamma Monitor
├── shared/                           # Shared IPC Bridge & Tokens
│   ├── wrapper-bridge.js             # Native IPC bridge
│   └── tokens.css                    # Unified design tokens
├── host/                             # Host Shell & Dashboard Interface
└── scripts/                          # Packaging Scripts
    ├── build-windows.ps1             # Windows .exe & NSIS builder
    ├── build-linux.sh                # Linux AppImage, Flatpak, and .deb builder
    └── build-macos.sh                # macOS .app and .dmg builder
```
