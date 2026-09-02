#!/usr/bin/env bash
# 1-Click Linux Native Desktop Launcher

set -euo pipefail
WORKSPACE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$WORKSPACE_ROOT"

echo -e "\033[1;36mLaunching PixieVault Native Linux Desktop Host...\033[0m"
npx @tauri-apps/cli dev
