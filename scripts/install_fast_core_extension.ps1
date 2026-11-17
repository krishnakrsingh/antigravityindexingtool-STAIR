# PowerShell script to install and register Antigravity Fast-Core Engine
param(
    [string]$TargetWorkspace = (Get-Location).Path
)

$ErrorActionPreference = "Stop"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host " Antigravity Fast-Core: High-Speed Engine Installer " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

$cargoPath = "$env:USERPROFILE\.cargo\bin\cargo.exe"
if (-not (Test-Path $cargoPath)) {
    $cargoCmd = Get-Command "cargo" -ErrorAction SilentlyContinue
    if ($cargoCmd) {
        $cargoPath = $cargoCmd.Source
    } else {
        Write-Error "Cargo was not found in ~/.cargo/bin or PATH. Please install Rust toolchain."
        exit 1
    }
}

Write-Host "`n[1/3] Building optimized release binary..." -ForegroundColor Yellow
& $cargoPath build --release
if ($LASTEXITCODE -ne 0) {
    Write-Error "Cargo release build failed."
    exit $LASTEXITCODE
}

$binPath = (Resolve-Path "target\release\antigravity-fast-core.exe").Path
Write-Host "✅ Release binary ready at: $binPath" -ForegroundColor Green

Write-Host "`n[2/3] Verifying Fast-Core Engine capabilities..." -ForegroundColor Yellow
& $binPath -w $TargetWorkspace --grep "package" --max-results 1
& $binPath -w $TargetWorkspace --verify

Write-Host "`n[3/3] MCP Configuration Snippet:" -ForegroundColor Yellow
$mcpConfig = @"
{
  "mcpServers": {
    "fast-core": {
      "command": "$($binPath.Replace('\', '\\'))",
      "args": ["-w", "$($TargetWorkspace.Replace('\', '\\'))", "--mcp"]
    }
  }
}
"@
Write-Host $mcpConfig -ForegroundColor Gray

Write-Host "`n==========================================================" -ForegroundColor Cyan
Write-Host " ✅ Antigravity Fast-Core Engine successfully built! " -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Cyan
