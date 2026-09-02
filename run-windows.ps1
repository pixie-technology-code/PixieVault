# 1-Click Windows Native Desktop Launcher
Set-Location $PSScriptRoot
$env:CARGO_TARGET_DIR = "C:\temp\pv-target"

Write-Host "Launching PixieVault Native Windows Desktop Application..." -ForegroundColor Cyan
npx @tauri-apps/cli dev
