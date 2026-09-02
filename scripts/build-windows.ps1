# Windows Multi-Target Packaging Script
# Generates pixievault.exe (standalone portable) and PixieVault-Setup.exe (NSIS Installer)

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  PixieVault Windows Packaging Builder  " -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

$WorkspaceRoot = Split-Path -Parent $PSScriptRoot
Set-Location $WorkspaceRoot

$env:CARGO_TARGET_DIR = "C:\temp\pv-target"
if (-not (Test-Path "C:\temp")) {
    New-Item -ItemType Directory -Path "C:\temp" | Out-Null
}

Write-Host "`n[1/3] Validating Rust Environment..." -ForegroundColor Yellow
cargo --version
rustc --version

Write-Host "`n[2/3] Building Standalone Portable Executable (pixievault.exe)..." -ForegroundColor Yellow
Set-Location "$WorkspaceRoot\src-tauri"
cargo build --release

$ReleaseExe = "C:\temp\pv-target\release\pixievault.exe"
if (Test-Path $ReleaseExe) {
    Copy-Item $ReleaseExe "$WorkspaceRoot\pixievault.exe" -Force
    Write-Host "✓ Standalone Portable Executable created: pixievault.exe" -ForegroundColor Green
}

Write-Host "`n[3/3] Building Windows Installer Packages (NSIS & MSI)..." -ForegroundColor Yellow
npx @tauri-apps/cli build --bundles nsis,msi

Write-Host "`n========================================" -ForegroundColor Green
Write-Host "  Windows Packaging Complete!           " -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host "Outputs:"
Write-Host "  • Portable Binary:  $WorkspaceRoot\pixievault.exe"
Write-Host "  • NSIS Setup Wizard: src-tauri\target\release\bundle\nsis\"
Write-Host "  • MSI Package:       src-tauri\target\release\bundle\msi\"
