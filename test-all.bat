@echo off
rem Automated Multi-Tier test runner for Windows CMD
pushd "%~dp0"
set CARGO_TARGET_DIR=C:\temp\pv-target
echo =====================================================================
echo    PixieVault Multi-Tier Reliability & Production Test Suite
echo =====================================================================

echo [TIER 1] Pure Unit Tests...
node tests\validate-manifests.js
if %errorlevel% neq 0 goto error
node tests\validate-dist-assets.js
if %errorlevel% neq 0 goto error
node tests\run-all-tests.js
if %errorlevel% neq 0 goto error
node tests\test-menus.js
if %errorlevel% neq 0 goto error
cd src-tauri
cargo test --test unit_tests -- --nocapture
if %errorlevel% neq 0 goto error
cd ..

echo [TIER 2] Filesystem & Persistence Tests...
cd src-tauri
cargo test --test persistence_tests -- --nocapture
if %errorlevel% neq 0 goto error
cargo test --test protector_envelope_tests -- --nocapture
if %errorlevel% neq 0 goto error
cargo test --test source_and_package_tests -- --nocapture
if %errorlevel% neq 0 goto error
cargo test --test menu_tests -- --nocapture
if %errorlevel% neq 0 goto error
cd ..

echo [TIER 3] Network & Process Integration Tests...
node tests\test-mikrotik-live-host.js
if %errorlevel% neq 0 goto error
cd src-tauri
cargo test --test composer_tests -- --nocapture
if %errorlevel% neq 0 goto error
cargo test --test sandbox_tests -- --nocapture
if %errorlevel% neq 0 goto error
cd ..

echo [TIER 4] Packaged Bundle Smoke & Core Task Acceptance Gate...
node tests\smoke-test-packaged-bundle.js
if %errorlevel% neq 0 goto error
node tests\test-packaged-distribution-artifact.js
if %errorlevel% neq 0 goto error
node tests\test-core-task-acceptance.js
if %errorlevel% neq 0 goto error
cd src-tauri
cargo test --test acceptance_test -- --nocapture
if %errorlevel% neq 0 goto error
cd ..

echo =====================================================================
echo    ✓ ALL 4 TEST TIERS PASSED SUCCESSFULLY!
echo =====================================================================
popd
pause
exit /b 0

:error
echo =====================================================================
echo    ❌ TEST TIER FAILED!
echo =====================================================================
popd
pause
exit /b 1
