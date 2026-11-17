<#
.SYNOPSIS
    Provisions 30 to 40 isolated benchmark workspace copies of the agaateapp codebase.
.DESCRIPTION
    Uses a high-performance Hybrid Junction + Isolated Prisma/Cache architecture:
    - Essential source trees (src, public, prisma, scripts, docs) and configs are copied.
    - node_modules/.prisma and node_modules/@prisma are deeply copied (~214MB per workspace)
      to allow fully isolated `prisma generate` execution without Windows DLL/handle locking collisions.
    - node_modules/.cache and .vite are isolated local directories to prevent build cache lock contention.
    - All other ~400 NPM packages in node_modules are linked via zero-copy NTFS Directory Junctions.
    - .git, .antigravity, graphify-out, .agents, and .next build artifacts are excluded to keep workspaces lightweight.
    - Each workspace occupies ~228 MB, totaling ~9.1 GB for 40 workspaces (taking ~3-4 minutes total).
.PARAMETER SourceDir
    Path to the source agaateapp repository. Defaults to C:\Users\krish\Downloads\agaateapp.
.PARAMETER WorkspaceRoot
    Root directory where workspaces will be created. Defaults to c:\Users\krish\Downloads\antigravitytoolbuild\workspaces.
.PARAMETER Count
    Number of workspaces to provision (30 to 40). Defaults to 40.
.PARAMETER CleanExisting
    If specified, deletes existing workspace directories in WorkspaceRoot before provisioning.
#>

param(
    [string]$SourceDir = "C:\Users\krish\Downloads\agaateapp",
    [string]$WorkspaceRoot = "c:\Users\krish\Downloads\antigravitytoolbuild\workspaces",
    [ValidateRange(1, 50)]
    [int]$Count = 40,
    [switch]$CleanExisting = $false
)

$ErrorActionPreference = "Stop"

$swTotal = [System.Diagnostics.Stopwatch]::StartNew()

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "Provisioning $Count Isolated Workspaces for agaateapp Benchmark" -ForegroundColor Cyan
Write-Host "Source Dir:     $SourceDir" -ForegroundColor Gray
Write-Host "Workspace Root: $WorkspaceRoot" -ForegroundColor Gray
Write-Host "Workspace Count:$Count" -ForegroundColor Gray
Write-Host "==========================================================" -ForegroundColor Cyan

# 1. Validate Source
if (-not (Test-Path $SourceDir)) {
    throw "Source directory not found: $SourceDir"
}
$sourceNm = Join-Path $SourceDir "node_modules"
if (-not (Test-Path $sourceNm)) {
    throw "Source node_modules not found at: $sourceNm"
}

# 2. Check Disk Space
$destDrive = (Get-Item $WorkspaceRoot -ErrorAction SilentlyContinue).PSDrive
if (-not $destDrive) {
    $driveLetter = [System.IO.Path]::GetPathRoot([System.IO.Path]::GetFullPath($WorkspaceRoot)).Substring(0, 1)
    $destDrive = Get-PSDrive -Name $driveLetter
}
$freeSpaceGB = [math]::Round($destDrive.Free / 1GB, 2)
$estimatedRequiredGB = [math]::Round(($Count * 0.25), 2)
Write-Host "[Disk Check] Drive $($destDrive.Name): has $freeSpaceGB GB free. Estimated requirement: $estimatedRequiredGB GB." -ForegroundColor Green

if ($destDrive.Free -lt ($estimatedRequiredGB * 1GB * 2)) {
    throw "Insufficient disk space on drive $($destDrive.Name):. Required: $estimatedRequiredGB GB, Free: $freeSpaceGB GB."
}

# 3. Prepare Workspace Root
if (-not (Test-Path $WorkspaceRoot)) {
    New-Item -ItemType Directory -Path $WorkspaceRoot -Force | Out-Null
}

$dirsToCopy = @("src", "public", "prisma", "scripts", "docs")
$filesToCopy = @(
    "package.json", "package-lock.json", "tsconfig.json", "tsconfig.tsbuildinfo",
    "next.config.ts", "next-env.d.ts", "eslint.config.mjs", "components.json",
    "vitest.config.ts", "vitest.integration.config.ts", "vitest.all.config.ts",
    "middleware.ts", "server.ts", ".env", ".gitignore"
)

# Pre-catalog node_modules items to link/copy
$nmItems = Get-ChildItem -Force $sourceNm

Write-Host "`nCataloging node_modules items: found $($nmItems.Count) top-level items." -ForegroundColor Gray

# 4. Provision Workspaces
for ($i = 1; $i -le $Count; $i++) {
    $wsName = "ws_{0:D2}" -f $i
    $wsPath = Join-Path $WorkspaceRoot $wsName
    $swWs = [System.Diagnostics.Stopwatch]::StartNew()

    if (Test-Path $wsPath) {
        if ($CleanExisting) {
            Write-Host "Cleaning existing workspace $wsName..." -ForegroundColor Yellow
            cmd.exe /c "rmdir /s /q `"$wsPath`""
        } else {
            Write-Host "Workspace $wsName already exists. Skipping." -ForegroundColor DarkGray
            continue
        }
    }

    New-Item -ItemType Directory -Path $wsPath -Force | Out-Null

    # Copy source directories
    foreach ($d in $dirsToCopy) {
        $srcDirPath = Join-Path $SourceDir $d
        if (Test-Path $srcDirPath) {
            Copy-Item -Recurse $srcDirPath (Join-Path $wsPath $d)
        }
    }

    # Copy configuration and root files
    foreach ($f in $filesToCopy) {
        $srcFilePath = Join-Path $SourceDir $f
        if (Test-Path $srcFilePath) {
            Copy-Item $srcFilePath (Join-Path $wsPath $f)
        }
    }

    # Setup hybrid node_modules
    $wsNm = Join-Path $wsPath "node_modules"
    New-Item -ItemType Directory -Path $wsNm -Force | Out-Null

    foreach ($item in $nmItems) {
        $targetPath = Join-Path $wsNm $item.Name
        if ($item.PSIsContainer) {
            if ($item.Name -in @(".prisma", "@prisma")) {
                # Isolated deep copy of Prisma runtime and engines
                Copy-Item -Recurse $item.FullName $targetPath
            } elseif ($item.Name -in @(".cache", ".vite")) {
                # Isolated empty build/test caches
                New-Item -ItemType Directory -Path $targetPath -Force | Out-Null
            } else {
                # Zero-copy directory junction for all shared dependencies
                New-Item -ItemType Junction -Path $targetPath -Target $item.FullName | Out-Null
            }
        } else {
            Copy-Item $item.FullName $targetPath
        }
    }

    $swWs.Stop()
    Write-Host "  [$i/$Count] Created $wsName in $($swWs.ElapsedMilliseconds) ms" -ForegroundColor Green
}

$swTotal.Stop()
Write-Host "`n==========================================================" -ForegroundColor Cyan
Write-Host "Successfully provisioned $Count workspaces in $($swTotal.Elapsed.ToString('mm\:ss'))" -ForegroundColor Green
Write-Host "Workspaces location: $WorkspaceRoot" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan
