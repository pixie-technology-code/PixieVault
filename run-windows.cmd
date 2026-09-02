@echo off
rem pushd automatically mounts UNC path to a temporary drive letter so CMD works on \\wsl.localhost
pushd "%~dp0"
set CARGO_TARGET_DIR=C:\temp\pv-target

rem Clean any cross-platform symlinked venvs or temporary databases from apps folder before bundling
if exist "apps\mikrotik_fleet\backend\.venv" (
    echo Cleaning cross-platform virtualenv cache...
    rmdir /s /q "apps\mikrotik_fleet\backend\.venv" 2>nul
)
if exist ".secrets" (
    rmdir /s /q ".secrets" 2>nul
)
if exist "apps\.secrets" (
    rmdir /s /q "apps\.secrets" 2>nul
)
if exist "apps\mikrotik_fleet\backend\.secrets" (
    rmdir /s /q "apps\mikrotik_fleet\backend\.secrets" 2>nul
)

echo Launching PixieVault Native Windows Desktop Application...
npx @tauri-apps/cli dev
popd

