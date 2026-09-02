@echo off
rem pushd automatically mounts UNC path to a temporary drive letter so CMD works on \\wsl.localhost
pushd "%~dp0"
set CARGO_TARGET_DIR=C:\temp\pv-target

echo Launching PixieVault Native Windows Desktop Host...
npx @tauri-apps/cli dev
popd
