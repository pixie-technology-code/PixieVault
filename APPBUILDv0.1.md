# APPBUILDv0.1.md — PixieVault Multi-Platform Native Host Specification

Welcome to the **PixieVault Multi-Platform Native Host Specification (v0.1)**. This document establishes the architecture, native packaging standards, IPC bridge contracts, OS biometric authentication, and manifest specifications for building or integrating applications with the **PixieVault Native Host Environment**.

---

## 1. Overview & Architecture

**PixieVault** is a 100% native desktop application environment built in Rust (Tauri v2). The native shell owns the window, native menus, hardware-backed biometric security, zero-trust encrypted storage, and multi-app lifecycle.

### Multi-Platform Packaging Matrix:

| Platform | Primary Executable / Package | Installer Packages | Biometric Provider | Web Runtime |
| :--- | :--- | :--- | :--- | :--- |
| **Windows** | Standalone `.exe` (`pixievault.exe`) | NSIS Setup (`.exe`) & WiX (`.msi`) | **Windows Hello** (Fingerprint, Face IR, PIN) | Microsoft Edge WebView2 |
| **macOS** | Native Application Bundle (`PixieVault.app`) | Apple Disk Image (`.dmg`) | **Touch ID / Face ID** (`LocalAuthentication`) | Apple WebKit (WKWebView) |
| **Linux** | Universal **AppImage** (`PixieVault.AppImage`) | **Flatpak**, `.deb` (Debian/Ubuntu), `.rpm` (Fedora/RHEL) | **PAM / FPrint / SecretService** | WebKit2GTK / WebKitGTK-6.0 |

---

## 2. Core Native Host Services

1. **Hardware-Backed OS Biometric Authentication**:
   - Native prompt on launch (Windows Hello, Touch ID, or Linux PAM).
   - On successful unlock, derives the master key in RAM (protected by `ZeroizeOnDrop`) and auto-restores the active guest workspace.
2. **Native OS Menuing System**:
   - Native top menu bar (`File`, `Security & Auth`, `Apps & Catalog`, `Storage & Data`, `View`, `Help`) with OS-standard keyboard shortcuts (`Ctrl/Cmd+O`, `Ctrl/Cmd+W`, `Ctrl/Cmd+L`, `Ctrl/Cmd+Shift+A`).
3. **Zero-Trust Encrypted Storage**:
   - Atomic AES-256-GCM encrypted `.pvlt` container with isolated per-app data namespaces.
4. **Multi-App Lifecycle & Inter-App Telemetry Bus**:
   - Pluggable guest application workspaces (*Dummy App A*, *Dummy App B*, *Dummy App C*) communicate strictly via direct Rust IPC commands (`window.PixieVaultNative`).

---

## 3. App Manifest Specification (`manifest.json`)

```json
{
  "app_id": "dummy_simulation_app",
  "name": "Dummy Engineering & Performance Analyzer",
  "version": "1.0.0",
  "min_pixievault_version": "0.1.0",
  "description": "High-fidelity domain modeling, dynamics, sweeps, and inter-app telemetry simulation",
  "entrypoint": "index.html",
  "update_url": "https://updates.example.local/apps/dummy_simulation/manifest.json",
  "author": "Pixie Technology",
  "permissions": {
    "requested_read": [
      "metricOutputPeak",
      "effectiveWeight",
      "operationalRatio",
      "totalBuildCostUsd",
      "bestCycleTimeSec"
    ],
    "requested_write": []
  },
  "theme_compatibility": {
    "supports_dark_mode": true,
    "supports_light_mode": true,
    "custom_accent_override": null
  }
}
```

---

## 4. Native IPC Bridge Contract (`wrapper-bridge.js`)

All communication between guest app modules and the Rust host occurs through `window.PixieVaultNative`:

```javascript
// 1. Load encrypted app state
const state = await PixieVaultNative.loadAppData("dummy_simulation_app");

// 2. Persist updated app state
await PixieVaultNative.saveAppData({ metricA: 500, metricB: 450 }, "dummy_simulation_app");

// 3. Register public metrics for adjacent apps
PixieVaultNative.registerDataExporter(() => ({
  metricOutputPeak: 500,
  operationalRatio: 3.42
}));

// 4. Query telemetry from another app
const bestCycle = await PixieVaultNative.requestCrossAppData("dummy_telemetry_app", "bestCycleTimeSec", "dummy_simulation_app");
```

---

## 5. Multi-Platform Build Commands

- **Windows**: `pwsh scripts/build-windows.ps1` (generates `pixievault.exe`, NSIS installer, and MSI)
- **Linux**: `bash scripts/build-linux.sh` (generates AppImage, `.deb`, `.rpm`, and Flatpak bundle)
- **macOS**: `bash scripts/build-macos.sh` (generates `PixieVault.app` and `.dmg`)
