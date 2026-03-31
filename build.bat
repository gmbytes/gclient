@echo off
setlocal

set "ROOT=%~dp0"
set "RUST_DIR=%ROOT%rust"
set "BIN_DIR=%ROOT%addons\gdbridge\bin"

echo [1/3] Building Rust workspace...
cd /d "%RUST_DIR%"
cargo build
if errorlevel 1 (
    echo BUILD FAILED
    exit /b 1
)

echo [2/3] Copying library to Godot addons...
if not exist "%BIN_DIR%" mkdir "%BIN_DIR%"
copy /Y "%RUST_DIR%\target\debug\gdbridge.dll" "%BIN_DIR%\gdbridge.dll"

echo [3/3] Done.
echo.
echo   Library: %BIN_DIR%\gdbridge.dll
echo   Open client\project.godot in Godot to run.

endlocal
