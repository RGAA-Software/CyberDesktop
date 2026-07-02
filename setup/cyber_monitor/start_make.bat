@echo off
setlocal

set "SCRIPT_DIR=%~dp0"
pushd "%SCRIPT_DIR%"
if errorlevel 1 (
    echo [setup/cyber_monitor] failed to enter setup directory
    exit /b 1
)

python make_setup.py %*
set "EXIT_CODE=%ERRORLEVEL%"

popd
exit /b %EXIT_CODE%
