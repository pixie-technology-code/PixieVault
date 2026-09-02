#!/usr/bin/env bash
# PixieVault Multi-Tier Automated Cross-Platform Test Suite
# Tier 1: Pure Unit Tests (Zero OS/network dependencies)
# Tier 2: Filesystem & Persistence Tests (Atomic storage, package exclusions)
# Tier 3: Network & Process Integration Tests (Ephemeral ports, Composer rollback, sandbox)
# Tier 4: Packaged Bundle Smoke & Core Task Acceptance Gate

set -euo pipefail

WORKSPACE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$WORKSPACE_ROOT"

echo -e "\033[1;36m=====================================================================\033[0m"
echo -e "\033[1;36m   PixieVault Multi-Tier Reliability & Production Test Suite         \033[0m"
echo -e "\033[1;36m=====================================================================\033[0m"

# ================= TIER 1: Pure Unit Tests =================
echo -e "\n\033[1;34m>>> [TIER 1] Pure Unit Tests (No OS/Network Dependencies)\033[0m"
echo -e "\033[1;33m[1.1] Validating App Manifests...\033[0m"
node tests/validate-manifests.js

echo -e "\n\033[1;33m[1.2] Validating Bundled frontendDist Assets...\033[0m"
node tests/validate-dist-assets.js

echo -e "\n\033[1;33m[1.3] Running Frontend Mock & IPC Bridge Unit Tests...\033[0m"
node tests/run-all-tests.js

echo -e "\n\033[1;33m[1.4] Running Native Menu Dispatch Tests...\033[0m"
node tests/test-menus.js

echo -e "\n\033[1;33m[1.5] Running Rust Pure Unit Tests...\033[0m"
cd "$WORKSPACE_ROOT/src-tauri"
cargo test --test unit_tests -- --nocapture
cd "$WORKSPACE_ROOT"

# ================= TIER 2: Filesystem & Persistence Tests =================
echo -e "\n\033[1;34m>>> [TIER 2] Filesystem & Persistence Integration Tests\033[0m"
echo -e "\033[1;33m[2.1] Running Atomic Vault Persistence & Exclusion Tests...\033[0m"
cd "$WORKSPACE_ROOT/src-tauri"
cargo test --test persistence_tests -- --nocapture

echo -e "\n\033[1;33m[2.2] Running Windows Hello & Envelope Protector Tests...\033[0m"
cargo test --test protector_envelope_tests -- --nocapture

echo -e "\n\033[1;33m[2.3] Running Portable Package & Sourcing Tests...\033[0m"
cargo test --test source_and_package_tests -- --nocapture

echo -e "\n\033[1;33m[2.4] Running Native Menu Integration Tests...\033[0m"
cargo test --test menu_tests -- --nocapture
cd "$WORKSPACE_ROOT"

# ================= TIER 3: Network & Process Integration Tests =================
echo -e "\n\033[1;34m>>> [TIER 3] Network & Process Integration Tests\033[0m"
echo -e "\033[1;33m[3.1] Running Live MikroTik Native Host Test...\033[0m"
node tests/test-mikrotik-live-host.js

echo -e "\n\033[1;33m[3.2] Running Vault Composer & Rollback Tests...\033[0m"
cd "$WORKSPACE_ROOT/src-tauri"
cargo test --test composer_tests -- --nocapture

echo -e "\n\033[1;33m[3.3] Running OS Sandbox & Namespace Isolation Tests...\033[0m"
cargo test --test sandbox_tests -- --nocapture
cd "$WORKSPACE_ROOT"

# ================= TIER 4: Packaged Smoke & Core Acceptance Gate =================
echo -e "\n\033[1;34m>>> [TIER 4] Packaged Bundle Smoke & Core Task Acceptance Gate\033[0m"
echo -e "\033[1;33m[4.1] Running Packaged Bundle Smoke Test...\033[0m"
node tests/smoke-test-packaged-bundle.js

echo -e "\n\033[1;33m[4.2] Running Packaged Distribution Artifact Staging Test...\033[0m"
node tests/test-packaged-distribution-artifact.js

echo -e "\n\033[1;33m[4.3] Running Node.js Core Task Acceptance Workflow...\033[0m"
node tests/test-core-task-acceptance.js

echo -e "\n\033[1;33m[4.4] Running Rust End-to-End Core Task Acceptance Gate...\033[0m"
cd "$WORKSPACE_ROOT/src-tauri"
cargo test --test acceptance_test -- --nocapture
cd "$WORKSPACE_ROOT"

echo -e "\n\033[1;32m=====================================================================\033[0m"
echo -e "\033[1;32m   ✓ ALL 4 TEST TIERS PASSED SUCCESSFULLY!                           \033[0m"
echo -e "\033[1;32m=====================================================================\033[0m"
