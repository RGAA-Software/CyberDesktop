@echo off
rem Shell context menu diagnostic - run on machines where cyber_files right-click hangs.
rem Place this .bat next to cyber_files.exe (or run from the repo with target\debug built).
rem Produces full.log (Layer A + Layer B) and layer_a.log (aggregate only) in this directory.

setlocal
cd /d "%~dp0"

set EXE=cyber_files.exe
if not exist "%EXE%" set EXE=..\target\debug\cyber_files.exe
if not exist "%EXE%" (
    echo [ERROR] cyber_files.exe not found next to this script or in ..\target\debug\
    pause
    exit /b 1
)

rem Kill any leftover instances first; a wedged shell extension DLL can keep
rem zombie processes alive after exit, which deadlocks the next test run.
taskkill /f /im cyber_files.exe >nul 2>&1

echo === [1/2] Full pipeline: Layer A aggregate + Layer B per-handler probes ===
echo     writing full.log ...
"%EXE%" --shell-menu-test --repeat 3 > full.log 2>&1
echo     exit code: %errorlevel%  (0=OK, 2=empty/timeout, 1=error)

taskkill /f /im cyber_files.exe >nul 2>&1

echo === [2/2] Layer A only (Files.app-equivalent aggregate query) ===
echo     writing layer_a.log ...
"%EXE%" --shell-menu-test --no-layer-b --repeat 3 > layer_a.log 2>&1
echo     exit code: %errorlevel%  (0=OK, 2=empty/timeout, 1=error)

taskkill /f /im cyber_files.exe >nul 2>&1

echo.
echo Done. Please send back full.log and layer_a.log.
pause
