@echo off
setlocal

set "ROOT=%~dp0"
set "RUST_DIR=%ROOT%rust"
set "BIN_DIR=%ROOT%addons\gdbridge\bin"
set "GENXLS=%ROOT%..\comm\tools\genxls.exe"
set "EXCEL_DIR=%ROOT%..\comm\excel"
set "CONFIG_OUT=%ROOT%data\config"
set "GD_OUT=%ROOT%data\generated\gd"

echo [1/4] Generating config ^(genxls + Godot .res via comm\tools\protoc-gen-gd.exe^)...
if exist "%GENXLS%" (
    if exist "%EXCEL_DIR%" (
        if not exist "%CONFIG_OUT%" mkdir "%CONFIG_OUT%"
        call "%GENXLS%" --in "%EXCEL_DIR%" --out "%CONFIG_OUT%" --gd-out "%GD_OUT%" --gclient "%ROOT%." --json=true --flag client --lang gd -v
        if errorlevel 1 (
            echo CONFIG GENERATION FAILED
            exit /b 1
        )
        echo   -^> %GD_OUT%\
        echo   -^> %CONFIG_OUT%\all.json
        echo   -^> %ROOT%data\generated\*.res
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
echo   GD classes: %GD_OUT%\
echo   Binary .res:  %ROOT%data\generated\ ^(requires comm\tools\protoc-gen-gd.exe or GODOT / --godot^)
echo.
echo   Open gclient\project.godot in Godot to run.

endlocal
