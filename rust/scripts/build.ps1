param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "debug"
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$rustRoot = Split-Path -Parent $scriptDir
$workspaceRoot = Split-Path -Parent $rustRoot
$binDir = Join-Path $workspaceRoot "addons\gdbridge\bin"

Push-Location $rustRoot
try {
    if ($Profile -eq "release") {
        cargo build -p gdbridge --release
        $libDir = Join-Path $rustRoot "target\release"
    } else {
        cargo build -p gdbridge
        $libDir = Join-Path $rustRoot "target\debug"
    }

    New-Item -ItemType Directory -Force -Path $binDir | Out-Null

    $isWindowsPlatform = $false
    $isLinuxPlatform = $false
    $isMacPlatform = $false

    if ($null -ne (Get-Variable -Name IsWindows -ErrorAction SilentlyContinue) -and $IsWindows) {
        $isWindowsPlatform = $true
    } elseif ($null -ne (Get-Variable -Name IsLinux -ErrorAction SilentlyContinue) -and $IsLinux) {
        $isLinuxPlatform = $true
    } elseif ($null -ne (Get-Variable -Name IsMacOS -ErrorAction SilentlyContinue) -and $IsMacOS) {
        $isMacPlatform = $true
    } elseif ($env:OS -eq "Windows_NT") {
        $isWindowsPlatform = $true
    }

    if ($isWindowsPlatform) {
        Copy-Item -Path (Join-Path $libDir "gdbridge.dll") -Destination (Join-Path $binDir "gdbridge.dll") -Force
        Write-Host "Copied gdbridge.dll -> $binDir"
    } elseif ($isLinuxPlatform) {
        Copy-Item -Path (Join-Path $libDir "libgdbridge.so") -Destination (Join-Path $binDir "libgdbridge.so") -Force
        Write-Host "Copied libgdbridge.so -> $binDir"
    } elseif ($isMacPlatform) {
        Copy-Item -Path (Join-Path $libDir "libgdbridge.dylib") -Destination (Join-Path $binDir "libgdbridge.dylib") -Force
        Write-Host "Copied libgdbridge.dylib -> $binDir"
    } else {
        throw "Unsupported platform for gdbridge copy step."
    }
}
finally {
    Pop-Location
}
