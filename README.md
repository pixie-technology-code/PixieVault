<div align="center">

# 💎 PixieVault

### **Zero-Trust, Air-Gapped Desktop Application Host & Micro-Orchestrator**

[![Rust](https://img.shields.io/badge/Rust-2021_Edition-orange.svg?logo=rust)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-v2.0-blue.svg?logo=tauri)](https://tauri.app/)
[![Security](https://img.shields.io/badge/Encryption-AES--256--GCM-success.svg)](#-zero-trust-security-architecture)
[![Key Derivation](https://img.shields.io/badge/KDF-Argon2id-blueviolet.svg)](#-zero-trust-security-architecture)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Platforms](https://img.shields.io/badge/Platforms-Windows%20|%20Linux%20|%20macOS-informational.svg)](#-platform-support)

<p align="center">
  <b>PixieVault is a zero-trust, air-gapped desktop application runtime that orchestrates offline web apps and microservices inside isolated sandboxes with biometric authentication, Inter-App bus telemetry, and hardware-backed AES-256 encrypted persistence.</b>
</p>

</div>

---

## 🧭 System Architecture

```mermaid
flowchart TB
    subgraph Host["💻 PixieVault Native Desktop Shell (Rust + Tauri v2)"]
        direction TB
        UI["🖥️ Unified Host Shell (Biometric Auth & Dynamic Router)"]
        Auth["🔑 Cryptographic Auth Engine (Argon2id + AES-256-GCM)"]
        Vault[("🔒 Atomic .pvlt Container (Write-Ahead State)")]
        Bus["📡 Zero-Trust Inter-App Telemetry Bus"]
        Composer["⚡ VaultComposer Micro-Orchestrator"]
        Registry["📦 AppRegistry & Package Bundler (.pvpkg)"]
        
        UI --> Auth --> Vault
        UI --> Composer
        UI --> Bus
        UI --> Registry
    end

    subgraph GuestLayer["🛡️ Isolated Guest Application Layer"]
        direction TB
        subgraph StaticApp["🌐 Pure Static Guest App"]
            StaticServer["Embedded Loopback HTTP Server (127.0.0.1:<auto>)"]
            StaticFrame["Webview Viewport (HTML5/CSS3/ES6)"]
            StaticServer --> StaticFrame
        end

        subgraph ServiceApp["⚙️ Declarative Microservice App"]
            Sidecar["Sandboxed Native Daemon (Python/Node/Binary)"]
            Healthcheck["TCP/HTTP Readiness Probe Loop"]
            ServiceFrame["Webview Viewport (Dynamic Web UI)"]
            Sidecar --> Healthcheck --> ServiceFrame
        end
    end

    Composer --> StaticServer
    Composer --> Sidecar
    StaticFrame <-->|"PixieVaultNative Bridge (IPC)"| Bus
    ServiceFrame <-->|"PixieVaultNative Bridge (IPC)"| Bus
```

---

## ✨ Key Features

### 🛡️ Zero-Trust Encrypted Persistence
- **Military-Grade Cryptography**: All application state envelopes are encrypted using authenticated **AES-256-GCM** with deterministic **Argon2id** key derivation (64MB memory cost, 3 iterations, 4 parallel lanes).
- **Atomic Write-Ahead Storage**: Writes are executed atomically via temporary staging files and automatic write-ahead `.bak` snapshots to prevent data loss or file corruption during unexpected power cuts.
- **Corrupted Vault Auto-Recovery**: Built-in self-healing engine automatically detects tampering or serialization errors and seamlessly restores from validated backup stores.

### ⚡ Universal Guest Micro-Orchestrator (`VaultComposer`)
- **Declarative Microservice Supervision**: Define multi-process backends (Flask, FastAPI, Express, Go, Rust binaries) directly in `manifest.json`.
- **Dynamic Ephemeral Ports**: Backend daemons bind to randomly allocated loopback ports (`127.0.0.1:0`), completely eliminating port collisions (`EADDRINUSE`).
- **Transactional Rollback**: Fast-failure healthcheck probe loops verify process readiness before mounting viewports; if a service fails during startup, all child processes are immediately reaped and rolled back.
- **Embedded Loopback Static Web Server**: Pure static web applications are served via a high-performance, embedded loopback HTTP server with complete MIME type resolution, CORS headers, and path traversal security.

### 📦 Dual-Source Air-Gapped Distribution
- **Standalone `.pvpkg` Packages**: Export and import complete self-contained application bundles as single-file portable archives.
- **Zero Artifact Leaks**: Built-in packager strips virtualenvs (`.venv`), bytecode caches (`__pycache__`), local databases (`*.db`, `*.sqlite`), and secrets before building distribution packages.
- **Direct USB / Folder Mounting**: Mount raw offline application folders directly from removable media or external drives.

### 📡 Zero-Trust Inter-App Telemetry Bus
- **Secure Cross-App Messaging**: Guest applications publish metrics and state without direct socket connections or shared memory vulnerabilities.
- **Least-Privilege RBAC**: Guest applications explicitly request read/write permissions in `manifest.json`. Unauthorized queries are rejected by the Rust host kernel.

### 🔒 Hardware OS Biometrics
- **Native OS Integration**: Authenticate instantly via **Windows Hello** (facial recognition / fingerprint / PIN), **Apple Touch ID**, or **Linux PAM Keyring**.

---

## 🤖 Ingesting GitHub Applications with AI (LLM Migration Playbook)

PixieVault is designed from the ground up for **zero-source-modification ingestion** of open source web applications, dashboards, dev tools, and multi-tier microservices.

By pairing an LLM coding assistant (such as **Antigravity**, **Claude**, **ChatGPT**, **Cursor**, or **GitHub Copilot**) with our standardized migration templates in [`docs/`](docs/), you can assess, adapt, and package any GitHub repository into an air-gapped, zero-trust PixieVault `.pvpkg` application in minutes.

```mermaid
flowchart LR
    GH["🐙 Target GitHub Repo<br/>(React / Vue / Flask / FastAPI)"] --> Step1["1️⃣ Pre-Migration Audit<br/>(docs/PRE_MIGRATION_PROMPT.md)"]
    Step1 --> Verdict{"Verdict?"}
    Verdict -->|"GO / CONDITIONAL_GO"| Step2["2️⃣ Generate Manifest & Sidecars<br/>(manifest.json + VaultComposer)"]
    Verdict -->|"NO_GO_HOST_GAP"| HostGap["Identify Generic Host Feature"]
    Step2 --> Step3["3️⃣ Virtualize Storage & Bus<br/>(wrapper-bridge.js)"]
    Step3 --> Step4["4️⃣ Build & Verify .pvpkg<br/>(./test-all.sh)"]
    Step4 --> PV["💎 Air-Gapped Secure App<br/>(PixieVault Host)"]
```

### 🛠️ Step-by-Step AI Ingestion Workflow

#### Step 1: Run the Pre-Migration Assessment Gate
Before modifying any code or copying assets, point your LLM at the target repository using our read-only compatibility gate prompt in [`docs/PRE_MIGRATION_PROMPT.md`](docs/PRE_MIGRATION_PROMPT.md).

**Copy & Paste this prompt into your LLM:**

> ```markdown
> You are assessing an existing application for faithful migration into PixieVault. 
> Do not edit the source application or PixieVault during this assessment.
> 
> ## Inputs
> - Source repository: https://github.com/<owner>/<target-repo>
> - Source revision / commit: <FULL_COMMIT_SHA_OR_TAG>
> - PixieVault repository: https://github.com/pixie-technology-code/PixieVault
> - Required targets: Windows, Linux, macOS
> - Operating mode: 100% Offline / Air-Gapped
> 
> Evaluate the process topology, build/supply-chain requirements, persistence lifecycle, 
> external boundaries, and rendering assumptions against PixieVault's implemented contract 
> as specified in docs/PRE_MIGRATION_PROMPT.md.
> 
> Return the standardized Capability Matrix, Storage Map, and Verdict (GO / CONDITIONAL_GO / NO_GO).
> ```

*The assessment will verify whether the app is pure static, single-sidecar, or multi-process, and produce a pass/fail capability matrix.* (See [`docs/PRE_MIGRATION_SAMPLE_ASSESSMENT.md`](docs/PRE_MIGRATION_SAMPLE_ASSESSMENT.md) for a complete example).

---

#### Step 2: Generate the PixieVault Manifest (`manifest.json`)
Once the assessment returns `GO`, create the guest application directory under `apps/<app_id>/` and generate the declarative `manifest.json`:

1. **For Pure Static SPAs** (React, Vue, Svelte, static HTML/JS):
   - Place built assets (`index.html`, `dist/`, `assets/`) in `apps/<app_id>/`.
   - Set `"entrypoint": "index.html"`. PixieVault's embedded loopback server will serve it with strict local CORS and MIME type isolation.

2. **For Python / Node / Binary Microservices**:
   - Declare the backend process under `composer.services`:
     ```json
     "composer": {
       "version": "1",
       "services": {
         "backend": {
           "command": ["python3", "server.py"],
           "working_dir": "backend",
           "port": "auto",
           "healthcheck": {
             "endpoint": "/healthz",
             "interval_ms": 100,
             "timeout_ms": 10000,
             "expected_status": 200
           }
         }
       }
     }
     ```
   - Set `"entrypoint": "http://127.0.0.1:{{services.backend.port}}/"`. PixieVault automatically allocates an ephemeral loopback port, verifies the healthcheck, and mounts the viewport.

---

#### Step 3: Wire Storage Virtualization & Telemetry (`wrapper-bridge.js`)
To enable zero-trust authenticated encryption without altering core application business logic:
- PixieVault provides a transparent storage virtualization shim (`wrapper-bridge.js`).
- Include `<script src="wrapper-bridge.js"></script>` in your entrypoint `index.html`.
- Standard browser `localStorage` and `sessionStorage` calls are automatically intercepted and redirected to the host's **Argon2id + AES-256-GCM** encrypted `.pvlt` database.
- Use `PixieVaultNative.bus.publish(topic, payload)` to publish metrics to the Inter-App Telemetry Bus.

---

#### Step 4: Package, Test, and Verify (`.pvpkg`)
Package the application bundle into a standalone, air-gapped `.pvpkg` archive:

```bash
# Package the application
cargo run --bin package_app -- apps/<app_id> dist/<app_id>.pvpkg

# Run the 4-tier verification suite across all test gates
./test-all.sh       # Linux / macOS
.\test-all.ps1      # Windows
```

For advanced multi-tier microservice patterns, refer to the full [Application Migration Playbook](docs/APP_MIGRATION_PLAYBOOK.md).

---

## 📋 Application Manifest Specification

Applications declare their identity, entrypoint, permissions, and optional microservices via a root `manifest.json`:

```json
{
  "app_id": "sample_service_app",
  "name": "Sample Microservice Application",
  "version": "1.0.0",
  "description": "Autonomous local backend and modern web frontend orchestration",
  "entrypoint": "http://127.0.0.1:{{services.backend.port}}/",
  "presentation": {
    "icon": "assets/app-icon.svg",
    "accent": "#6366f1",
    "category": "Utilities"
  },
  "permissions": {
    "network": true,
    "sidecar": true,
    "bus_publish": true,
    "bus_subscribe": true
  },
  "composer": {
    "version": "1",
    "services": {
      "backend": {
        "command": ["python3", "app.py"],
        "working_dir": "backend",
        "port": "auto",
        "requirements": "requirements.txt",
        "healthcheck": {
          "endpoint": "/healthz",
          "interval_ms": 100,
          "timeout_ms": 10000,
          "expected_status": 200
        },
        "sandbox": {
          "enabled": true,
          "isolate_network_loopback": false
        }
      }
    }
  }
}
```

---

## 🚀 Quickstart Guide

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (1.78+)
- [Node.js](https://nodejs.org/) (v18+)
- [Python](https://www.python.org/) (3.10+)

### Running Locally

#### 🐧 Linux
```bash
# Clone the repository
git clone https://github.com/pixie-technology-code/PixieVault.git
cd PixieVault

# Install system dependencies (Ubuntu/Debian)
sudo apt-get update && sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev bubblewrap

# Launch PixieVault Host
./run-linux.sh
```

#### 🪟 Windows
```powershell
# In PowerShell or Command Prompt:
.\run-windows.bat
# Or using PowerShell script directly:
.\run-windows.ps1
```

---

## 🧪 Comprehensive Multi-Tier Test Suite

PixieVault features an automated 4-tier verification suite covering unit tests, persistence integrity, live process orchestration, and end-to-end acceptance gates:

```bash
# Run all 4 test tiers (Linux)
./test-all.sh

# Run all 4 test tiers (Windows)
.\test-all.ps1
```

### Test Hierarchy:
1. **Tier 1: Pure Unit Tests** (`validate-manifests.js`, `validate-dist-assets.js`, `unit_tests.rs`)
2. **Tier 2: Filesystem & Persistence Tests** (`persistence_tests.rs`, `source_and_package_tests.rs`)
3. **Tier 3: Network & Process Orchestration** (`composer_tests.rs`, `sandbox_tests.rs`)
4. **Tier 4: End-to-End Acceptance Gate** (`acceptance_test.rs`, `smoke-test-packaged-bundle.js`)

---

## 💻 Platform Support

| Operating System | Webview Engine | Biometric Provider | Sandbox Isolation |
| :--- | :--- | :--- | :--- |
| **Windows 10 / 11** | Microsoft WebView2 | Windows Hello (`Win32 Security Credentials`) | AppContainer / Process Isolation |
| **Linux (Ubuntu/Fedora/Arch)** | WebKit2GTK 4.1 | Linux PAM / Secret Service Keyring | Linux Namespaces (`bubblewrap`) |
| **macOS (11.0+)** | Apple WebKit | Touch ID / LocalAuthentication API | macOS Seatbelt Sandbox |

---

## 📄 License

PixieVault is licensed under the [MIT License](LICENSE).
Distributed by **Pixie Technology**.
