@echo off
setlocal

set "ROOT=%~dp0"
set "RUST_DIR=%ROOT%rust"
set "BIN_DIR=%ROOT%addons\gdbridge\bin"
set "GENXLS=%ROOT%..\comm\tools\genxls\genxls.exe"
set "EXCEL_DIR=%ROOT%..\comm\excel"
set "CONFIG_OUT=%ROOT%data\config"

echo [1/4] Generating config from Excel...
if exist "%GENXLS%" (
    if exist "%EXCEL_DIR%" (
        if not exist "%CONFIG_OUT%" mkdir "%CONFIG_OUT%"
        call "%GENXLS%" --in "%EXCEL_DIR%" --out "%CONFIG_OUT%" --split-json --json=true --flag client --lang rust -v
        if errorlevel 1 (
            echo CONFIG GENERATION FAILED
            exit /b 1
        )
        if not exist "%RUST_DIR%\configcore\src" mkdir "%RUST_DIR%\configcore\src"
        copy /Y "%CONFIG_OUT%\config.gen.rs" "%RUST_DIR%\configcore\src\config.gen.rs" >nul
        echo   -^> %CONFIG_OUT%\manifest.json + tables\
        echo   -^> %RUST_DIR%\configcore\src\config.gen.rs
    ) else (
        echo   Excel dir not found: %EXCEL_DIR% (skipping config generation)
    )
) else (
    echo   genxls.exe not found: %GENXLS% (skipping config generation)
    echo   Run comm\build.bat first to build genxls.exe
)

echo [2/4] Building Rust workspace...
cd /d "%RUST_DIR%"
cargo build
if errorlevel 1 (
    echo BUILD FAILED
    exit /b 1
)

echo [3/4] Copying library to Godot addons...
if not exist "%BIN_DIR%" mkdir "%BIN_DIR%"
copy /Y "%RUST_DIR%\target\debug\gdbridge.dll" "%BIN_DIR%\gdbridge.dll"

echo [4/4] Done.
echo.
echo   Library: %BIN_DIR%\gdbridge.dll
echo   Config:  %CONFIG_OUT%\
echo   Open gclient\project.godot in Godot to run.

endlocal
