# Antigravity Fast-Core Production One-Command Installer for Windows PowerShell
$ErrorActionPreference = "Stop"

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "  Antigravity Fast-Core: Production Installer (Windows)     " -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

# 1. Locate Cargo/Rust
$cargoPath = "$env:USERPROFILE\.cargo\bin\cargo.exe"
if (-not (Test-Path $cargoPath)) {
    $cargoCmd = Get-Command "cargo" -ErrorAction SilentlyContinue
    if ($cargoCmd) {
        $cargoPath = $cargoCmd.Source
    } else {
        Write-Error "Cargo/Rust toolchain not found. Please install Rust from https://rustup.rs"
        exit 1
    }
} else {
    $cargoBinDir = "$env:USERPROFILE\.cargo\bin"
    if ($env:Path -notlike "*$cargoBinDir*") {
        $env:Path = "$cargoBinDir;$env:Path"
    }
}

Write-Host "`n[1/3] Compiling high-speed release binary..." -ForegroundColor Yellow
& $cargoPath build --release
if ($LASTEXITCODE -ne 0) {
    Write-Error "Cargo build failed."
    exit $LASTEXITCODE
}

$releaseExe = (Resolve-Path "target\release\antigravity-fast-core.exe").Path
Write-Host "✅ Release binary built: $releaseExe" -ForegroundColor Green

# 2. Run self-setup and auto-configuration
Write-Host "`n[2/3] Registering global binary and syncing IDE MCP schemas..." -ForegroundColor Yellow
& $releaseExe --setup
if ($LASTEXITCODE -ne 0) {
    Write-Error "Setup registration failed."
    exit $LASTEXITCODE
}

# 3. Add to user PATH and current session
Write-Host "`n[3/3] Checking environment PATH..." -ForegroundColor Yellow
$binDir = "$env:USERPROFILE\.antigravity\bin"
$userPath = [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::User)
if ([string]::IsNullOrWhiteSpace($userPath)) {
    [Environment]::SetEnvironmentVariable("Path", $binDir, [EnvironmentVariableTarget]::User)
    Write-Host "✅ Set $binDir in User PATH environment variable." -ForegroundColor Green
} elseif ($userPath -notlike "*$binDir*") {
    $newPath = "$userPath;$binDir"
    [Environment]::SetEnvironmentVariable("Path", $newPath, [EnvironmentVariableTarget]::User)
    Write-Host "✅ Added $binDir to User PATH environment variable." -ForegroundColor Green
} else {
    Write-Host "✅ $binDir is already present in User PATH." -ForegroundColor Green
}

if ($env:Path -notlike "*$binDir*") {
    $env:Path = "$binDir;$env:Path"
}

Write-Host "`n============================================================" -ForegroundColor Cyan
Write-Host "  ✅ Installation Complete! Fast-Core is ready for PROD!   " -ForegroundColor Green
Write-Host "  Commands available:                                      " -ForegroundColor Gray
Write-Host "    antigravity-fast-core --mcp                            " -ForegroundColor Gray
Write-Host "    antigravity-fast-core --cache-list                     " -ForegroundColor Gray
Write-Host "    antigravity-fast-core --setup                          " -ForegroundColor Gray
Write-Host "============================================================" -ForegroundColor Cyan
