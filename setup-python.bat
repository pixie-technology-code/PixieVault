@echo off
rem PixieVault Python Dependencies Setup for Windows CMD
pushd "%~dp0"
echo ==================================================
echo    PixieVault Python Dependencies Installer
echo ==================================================

echo Installing Flask, Cryptography, and RouterOS-API for MikroTik Fleet Manager...
python -m pip install -r MikrotikFleetMgr\automation\mac-finder\requirements.txt
if %errorlevel% neq 0 (
    echo Retrying with py -m pip...
    py -m pip install -r MikrotikFleetMgr\automation\mac-finder\requirements.txt
)

if %errorlevel% equ 0 (
    echo.
    echo ==================================================
    echo    Python dependencies installed successfully!
    echo ==================================================
) else (
    echo.
    echo ==================================================
    echo    Failed to install Python dependencies.
    echo ==================================================
)
popd
pause
