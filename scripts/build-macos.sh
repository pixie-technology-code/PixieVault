#!/usr/bin/env bash
# macOS Multi-Target Packaging Script
# Generates native macOS Application Bundle (.app) and Apple Disk Image (.dmg)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(dirname "$SCRIPT_DIR")"

echo -e "\033[1;36m========================================\033[0m"
echo -e "\033[1;36m   PixieVault macOS Packaging Builder   \033[0m"
echo -e "\033[1;36m========================================\033[0m"

cd "$WORKSPACE_ROOT"

echo -e "\n\033[1;33m[1/2] Building macOS .app & .dmg Bundles...\033[0m"
npx @tauri-apps/cli build --bundles app,dmg

echo -e "\n\033[1;32m========================================\033[0m"
echo -e "\033[1;32m   macOS Packaging Complete!            \033[0m"
echo -e "\033[1;32m========================================\033[0m"
echo "Outputs:"
echo "  • Application Bundle: $WORKSPACE_ROOT/src-tauri/target/release/bundle/macos/PixieVault.app"
echo "  • Apple Disk Image:   $WORKSPACE_ROOT/src-tauri/target/release/bundle/dmg/PixieVault.dmg"
