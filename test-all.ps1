# PixieVault Multi-Tier Automated Cross-Platform Test Suite for Windows
# Tier 1: Pure Unit Tests (Zero OS/network dependencies)
# Tier 2: Filesystem & Persistence Tests (Atomic storage, package exclusions)
# Tier 3: Network & Process Integration Tests (Ephemeral ports, Composer rollback, sandbox)
# Tier 4: Packaged Bundle Smoke & Core Task Acceptance Gate

$WorkspaceRoot = $PSScriptRoot
Set-Location $WorkspaceRoot

$env:CARGO_TARGET_DIR = "C:\temp\pv-target"
if (-not (Test-Path "C:\temp")) {
    New-Item -ItemType Directory -Path "C:\temp" | Out-Null
}

Write-Host "=====================================================================" -ForegroundColor Cyan
Write-Host "   PixieVault Multi-Tier Reliability & Production Test Suite         " -ForegroundColor Cyan
Write-Host "=====================================================================" -ForegroundColor Cyan

# Tier 1: Pure Unit Tests
Write-Host "`n>>> [TIER 1] Pure Unit Tests (No OS/Network Dependencies)" -ForegroundColor Blue
Write-Host "[1.1] Validating App Manifests..." -ForegroundColor Yellow
node tests/validate-manifests.js
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "`n[1.2] Validating Bundled frontendDist Assets..." -ForegroundColor Yellow
node tests/validate-dist-assets.js
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "`n[1.3] Running Frontend Mock & IPC Bridge Unit Tests..." -ForegroundColor Yellow
node tests/run-all-tests.js
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "`n[1.4] Running Native Menu Dispatch Tests..." -ForegroundColor Yellow
node tests/test-menus.js
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "`n[1.5] Running Rust Pure Unit Tests..." -ForegroundColor Yellow
Set-Location "$WorkspaceRoot\src-tauri"
cargo test --test unit_tests -- --nocapture
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Set-Location $WorkspaceRoot

# Tier 2: Filesystem & Persistence Tests
Write-Host "`n>>> [TIER 2] Filesystem & Persistence Integration Tests" -ForegroundColor Blue
Write-Host "[2.1] Running Atomic Vault Persistence & Recovery Tests..." -ForegroundColor Yellow
Set-Location "$WorkspaceRoot\src-tauri"
cargo test --test persistence_tests -- --nocapture
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "`n[2.2] Running Windows Hello & Envelope Protector Tests..." -ForegroundColor Yellow
cargo test --test protector_envelope_tests -- --nocapture
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "`n[2.3] Running Portable Package & Sourcing Tests..." -ForegroundColor Yellow
cargo test --test source_and_package_tests -- --nocapture
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "`n[2.4] Running Native Menu Integration Tests..." -ForegroundColor Yellow
cargo test --test menu_tests -- --nocapture
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Set-Location $WorkspaceRoot

# Tier 3: Network & Process Integration Tests
Write-Host "`n>>> [TIER 3] Network & Process Integration Tests" -ForegroundColor Blue
Write-Host "[3.1] Running Live MikroTik Native Host Test..." -ForegroundColor Yellow
node tests/test-mikrotik-live-host.js
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "`n[3.2] Running Vault Composer & Rollback Tests..." -ForegroundColor Yellow
Set-Location "$WorkspaceRoot\src-tauri"
cargo test --test composer_tests -- --nocapture
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Set-Location $WorkspaceRoot

# Tier 4: Packaged Bundle Smoke & Core Task Acceptance Gate
Write-Host "`n>>> [TIER 4] Packaged Bundle Smoke & Core Task Acceptance Gate" -ForegroundColor Blue
Write-Host "[4.1] Running Packaged Bundle Smoke Test..." -ForegroundColor Yellow
node tests/smoke-test-packaged-bundle.js
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "`n[4.2] Running Packaged Distribution Artifact Staging Test..." -ForegroundColor Yellow
node tests/test-packaged-distribution-artifact.js
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "`n[4.3] Running Node.js Core Task Acceptance Workflow..." -ForegroundColor Yellow
node tests/test-core-task-acceptance.js
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "`n[4.4] Running Rust End-to-End Core Task Acceptance Gate..." -ForegroundColor Yellow
Set-Location "$WorkspaceRoot\src-tauri"
cargo test --test acceptance_test -- --nocapture
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Set-Location $WorkspaceRoot

Write-Host "`n=====================================================================" -ForegroundColor Green
Write-Host "   ✓ ALL 4 TEST TIERS PASSED SUCCESSFULLY!                           " -ForegroundColor Green
Write-Host "=====================================================================" -ForegroundColor Green
