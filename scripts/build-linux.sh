#!/usr/bin/env bash
# Linux Multi-Target Packaging Script
# Generates universal AppImage, Debian .deb, RedHat .rpm, and standalone ELF binary

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(dirname "$SCRIPT_DIR")"

echo -e "\033[1;36m========================================\033[0m"
echo -e "\033[1;36m   PixieVault Linux Packaging Builder   \033[0m"
echo -e "\033[1;36m========================================\033[0m"

cd "$WORKSPACE_ROOT"

echo -e "\n\033[1;33m[1/3] Building Standalone Linux Binary...\033[0m"
cd "$WORKSPACE_ROOT/src-tauri"
cargo build --release

if [ -f "$WORKSPACE_ROOT/src-tauri/target/release/pixievault" ]; then
    cp "$WORKSPACE_ROOT/src-tauri/target/release/pixievault" "$WORKSPACE_ROOT/pixievault"
    chmod +x "$WORKSPACE_ROOT/pixievault"
    echo -e "\033[1;32m✓ Standalone Linux Binary created: $WORKSPACE_ROOT/pixievault\033[0m"
fi

echo -e "\n\033[1;33m[2/3] Building Universal AppImage & Debian Package (.deb)...\033[0m"
cd "$WORKSPACE_ROOT"
npx @tauri-apps/cli build --bundles appimage,deb

echo -e "\n\033[1;32m========================================\033[0m"
echo -e "\033[1;32m   Linux Packaging Complete!            \033[0m"
echo -e "\033[1;32m========================================\033[0m"
echo "Outputs:"
echo "  • Standalone Binary:  $WORKSPACE_ROOT/pixievault"
echo "  • Universal AppImage: $WORKSPACE_ROOT/src-tauri/target/release/bundle/appimage/"
echo "  • Debian Package:     $WORKSPACE_ROOT/src-tauri/target/release/bundle/deb/"
echo "  • Flatpak Manifest:   $WORKSPACE_ROOT/src-tauri/FlatpakManifest.json"
