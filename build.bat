@echo off
setlocal

set "ROOT=%~dp0"
set "RUST_DIR=%ROOT%rust"
set "BIN_DIR=%ROOT%addons\gdbridge\bin"
set "GENXLS=%ROOT%..\comm\tools\genxls\genxls.exe"
set "EXCEL_DIR=%ROOT%..\comm\excel"
set "CONFIG_OUT=%ROOT%data\config"
set "DATA_CLASSES=%ROOT%src\game\config\data_classes"
set "TOOLS_DIR=%ROOT%tools"

echo [1/5] Generating config from Excel...
if exist "%GENXLS%" (
    if exist "%EXCEL_DIR%" (
        if not exist "%CONFIG_OUT%" mkdir "%CONFIG_OUT%"
        call "%GENXLS%" --in "%EXCEL_DIR%" --out "%CONFIG_OUT%" --json=true --flag client --lang gd -v
        if errorlevel 1 (
            echo CONFIG GENERATION FAILED
            exit /b 1
        )
        echo   -^> %CONFIG_OUT%\gd\
        echo   -^> %CONFIG_OUT%\all.json
    ) else (
        echo   Excel dir not found: %EXCEL_DIR% (skipping config generation)
    )
) else (
    echo   genxls.exe not found: %GENXLS% (skipping config generation)
    echo   Run comm\build.bat first to build genxls.exe
)

echo [2/5] Copying GD data classes...
if exist "%CONFIG_OUT%\gd" (
    if not exist "%DATA_CLASSES%" mkdir "%DATA_CLASSES%"
    for %%f in ("%CONFIG_OUT%\gd\c_*.gd") do (
        copy /Y "%%f" "%DATA_CLASSES%\" >nul
    )
    if not exist "%TOOLS_DIR%" mkdir "%TOOLS_DIR%"
    copy /Y "%CONFIG_OUT%\gd\res_importer.gd" "%TOOLS_DIR%\res_importer.gd" >nul
    echo   -^> %DATA_CLASSES%\
    echo   -^> %TOOLS_DIR%\res_importer.gd
)

echo [3/5] Building Rust workspace...
cd /d "%RUST_DIR%"
cargo build
if errorlevel 1 (
    echo BUILD FAILED
    exit /b 1
)

echo [4/5] Copying library to Godot addons...
if not exist "%BIN_DIR%" mkdir "%BIN_DIR%"
copy /Y "%RUST_DIR%\target\debug\gdbridge.dll" "%BIN_DIR%\gdbridge.dll"

echo [5/5] Done.
echo.
echo   Library: %BIN_DIR%\gdbridge.dll
echo   Config:  %CONFIG_OUT%\
echo   GD classes: %DATA_CLASSES%\
echo.
echo   To generate .res files, run in Godot:
echo     godot --headless --script res://tools/res_importer.gd
echo.
echo   Open gclient\project.godot in Godot to run.

endlocal
