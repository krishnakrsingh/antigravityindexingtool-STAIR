# scripts/provision_workspaces.ps1
# Provision isolated dual-workspaces for makeplane/plane benchmark
# Milestone M4 / Requirement R3

param(
    [string]$SourceRepo = "",
    [string]$WorkspaceRoot = "",
    [string]$VectorSourceDir = ""
)

$ErrorActionPreference = "Stop"

if (-not $SourceRepo) {
    $SourceRepo = Join-Path (Split-Path -Parent $PSScriptRoot) "target_repo\plane"
}
if (-not $WorkspaceRoot) {
    $WorkspaceRoot = Join-Path (Split-Path -Parent $PSScriptRoot) "benchmark_workspaces"
}

# Resolve full absolute paths
$SourceRepo = (Resolve-Path $SourceRepo).Path
$WorkspaceRoot = [System.IO.Path]::GetFullPath($WorkspaceRoot)

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "Provisioning Dual Workspaces for Makeplane/Plane Benchmark" -ForegroundColor Cyan
Write-Host "Source Repo:    $SourceRepo" -ForegroundColor Gray
Write-Host "Workspace Root: $WorkspaceRoot" -ForegroundColor Gray
Write-Host "==========================================================" -ForegroundColor Cyan

$WorkspaceA = Join-Path $WorkspaceRoot "workspace_a_stock\plane"
$WorkspaceB = Join-Path $WorkspaceRoot "workspace_b_enhanced\plane"
$WorkspaceBVectors = Join-Path $WorkspaceB ".antigravity\vectors"

New-Item -ItemType Directory -Force -Path $WorkspaceA | Out-Null
New-Item -ItemType Directory -Force -Path $WorkspaceB | Out-Null

# --- Provision Workspace A (Stock: Pristine, NO .antigravity) ---
Write-Host "`n[1/3] Provisioning Workspace A (Pristine Stock)..." -ForegroundColor Cyan
$swA = [System.Diagnostics.Stopwatch]::StartNew()
robocopy "$SourceRepo" "$WorkspaceA" /MIR /MT:16 /R:1 /W:1 /NFL /NDL /NP /XD .antigravity
$exitA = $LASTEXITCODE
$swA.Stop()

# Robocopy exit codes 0-7 represent success; 8 or higher is failure
if ($exitA -ge 8) {
    throw "Failed provisioning Workspace A! Robocopy exit code: $exitA"
}
Write-Host "Workspace A provisioned successfully in $($swA.ElapsedMilliseconds) ms (Robocopy Exit code: $exitA)" -ForegroundColor Green

# Extra guarantee: Ensure .antigravity does not exist in Workspace A
$antigravityA = Join-Path $WorkspaceA ".antigravity"
if (Test-Path $antigravityA) {
    Remove-Item -Recurse -Force -Path $antigravityA
}

# --- Provision Workspace B (Enhanced: Full code + Crepe) ---
Write-Host "`n[2/3] Provisioning Workspace B (Crepe + Fast-Core)..." -ForegroundColor Cyan
$swB = [System.Diagnostics.Stopwatch]::StartNew()

# If source repo lacks .antigravity/vectors but Workspace B already has vectors, exclude them from /MIR purge
$sourceVectors = Join-Path $SourceRepo ".antigravity\vectors"
if (-not (Test-Path $sourceVectors) -and (Test-Path $WorkspaceBVectors) -and (Get-ChildItem -Path $WorkspaceBVectors -File -ErrorAction SilentlyContinue)) {
    robocopy "$SourceRepo" "$WorkspaceB" /MIR /MT:16 /R:1 /W:1 /NFL /NDL /NP /XD "$WorkspaceBVectors"
} else {
    robocopy "$SourceRepo" "$WorkspaceB" /MIR /MT:16 /R:1 /W:1 /NFL /NDL /NP
}
$exitB = $LASTEXITCODE
$swB.Stop()

if ($exitB -ge 8) {
    throw "Failed provisioning Workspace B! Robocopy exit code: $exitB"
}
Write-Host "Workspace B provisioned successfully in $($swB.ElapsedMilliseconds) ms (Robocopy Exit code: $exitB)" -ForegroundColor Green

# --- Provision Workspace B Vector RAG Index (.antigravity/vectors/) ---
Write-Host "`n[3/3] Checking & Syncing Vector RAG Index in Workspace B..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path $WorkspaceBVectors | Out-Null

# Check potential vector sources
$candidateVectorDirs = @()
if ($VectorSourceDir) { $candidateVectorDirs += $VectorSourceDir }
$candidateVectorDirs += (Join-Path $SourceRepo ".antigravity\vectors")
$candidateVectorDirs += (Join-Path (Split-Path -Parent $PSScriptRoot) ".antigravity\vectors")

$vectorCopied = $false
foreach ($cand in $candidateVectorDirs) {
    if ((Test-Path $cand) -and (Get-ChildItem -Path $cand -Filter "*.faiss" -ErrorAction SilentlyContinue)) {
        Write-Host "Found Vector index in: $cand. Mirroring to Workspace B..." -ForegroundColor Cyan
        robocopy "$cand" "$WorkspaceBVectors" /MIR /MT:8 /R:1 /W:1 /NFL /NDL /NP
        $exitVec = $LASTEXITCODE
        if ($exitVec -ge 8) {
            throw "Failed copying Vector RAG index into Workspace B! Robocopy exit code: $exitVec"
        }
        Write-Host "Vector RAG index mirrored from $cand to Workspace B (Exit code: $exitVec)" -ForegroundColor Green
        $vectorCopied = $true
        break
    }
}

if (-not $vectorCopied) {
    if (Test-Path (Join-Path $WorkspaceBVectors "ast.faiss")) {
        Write-Host "Vector RAG index already present in Workspace B." -ForegroundColor Green
    } else {
        Write-Host "[INFO] Vector RAG index files not yet detected in source locations." -ForegroundColor Yellow
        Write-Host "[INFO] Workspace B directory ready at: $WorkspaceBVectors" -ForegroundColor Yellow
    }
}

# --- Summary & Integrity Verification ---
Write-Host "`n================ Provisioning Verification ================" -ForegroundColor Cyan
$countA = (Get-ChildItem -Recurse -File -Path $WorkspaceA | Measure-Object).Count
$hasAntigravityA = Test-Path (Join-Path $WorkspaceA ".antigravity")

$countB = (Get-ChildItem -Recurse -File -Path $WorkspaceB | Measure-Object).Count
$hasCrepeB = Test-Path (Join-Path $WorkspaceB ".antigravity\crepe\gen_2\metadata.json")
$hasVectorsB = Test-Path (Join-Path $WorkspaceB ".antigravity\vectors")

Write-Host "Workspace A (Stock):" -ForegroundColor White
Write-Host "  - File count: $countA" -ForegroundColor White
Write-Host "  - Contains .antigravity: $hasAntigravityA (Must be False)" -ForegroundColor $(if ($hasAntigravityA) { "Red" } else { "Green" })

Write-Host "Workspace B (Enhanced):" -ForegroundColor White
Write-Host "  - File count: $countB" -ForegroundColor White
Write-Host "  - Contains Crepe: $hasCrepeB (Must be True)" -ForegroundColor $(if ($hasCrepeB) { "Green" } else { "Red" })
Write-Host "  - Contains Vectors Dir: $hasVectorsB (Must be True)" -ForegroundColor $(if ($hasVectorsB) { "Green" } else { "Red" })

if ($hasAntigravityA) {
    throw "Verification Failure: Workspace A must NOT have .antigravity!"
}
if (-not $hasCrepeB) {
    throw "Verification Failure: Workspace B must have Crepe index!"
}

Write-Host "`nDual-Workspace Provisioning Completed Successfully!" -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Cyan
