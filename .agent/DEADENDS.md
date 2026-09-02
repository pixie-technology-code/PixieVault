# Dead Ends

<!-- Register of approaches that didn't work. Prefix each entry with DEADEND. -->
<!-- Future agents: read this at session start to avoid wasting time. -->

## Format

- **DEADEND**: [What was tried]
  - **Why it failed**: [Root cause]
  - **Alternative**: [What works instead]

- **DEADEND**: Treating PixieVault as a web app with browser-mode dev fallbacks (`localStorage`, mock browser servers, web-embedded menus).
  - **Why it failed**: PixieVault is a 100% native desktop application environment. Web-based menus and browser storage violate zero-trust architecture and create confusing, inconsistent multi-app interfaces.
  - **Alternative**: Pure native Rust shell (Tauri v2) that owns the window, native menuing system (File/Security/Apps/Storage/View), OS biometric integration (Windows Hello / Touch ID), and AES-256-GCM encrypted vault container.
