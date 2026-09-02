# PixieVault Python Dependencies Setup for Windows
$WorkspaceRoot = $PSScriptRoot
Set-Location $WorkspaceRoot

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "   PixieVault Python Dependencies Installer       " -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

$ReqFile = Join-Path $WorkspaceRoot "MikrotikFleetMgr\automation\mac-finder\requirements.txt"

Write-Host "Installing Flask, Cryptography, and RouterOS-API for MikroTik Fleet Manager..." -ForegroundColor Yellow
python -m pip install -r $ReqFile

if ($LASTEXITCODE -ne 0) {
    Write-Host "Trying with 'py -m pip'..." -ForegroundColor Yellow
    py -m pip install -r $ReqFile
}

if ($LASTEXITCODE -eq 0) {
    Write-Host "`n✓ Python dependencies installed successfully!" -ForegroundColor Green
} else {
    Write-Host "`n❌ Failed to install Python dependencies. Please ensure Python and Pip are in PATH." -ForegroundColor Red
}
