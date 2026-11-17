<#
.SYNOPSIS
    AgaateApp Multi-Permutation Benchmark: Complete 4-Tier E2E Test Suite Runner.
.DESCRIPTION
    Executes opaque-box validation across all 4 benchmark tiers:
    - Tier 1: Feature Coverage (R1 - R5 >= 25 tests)
    - Tier 2: Boundary & Corner Cases (>= 25 tests)
    - Tier 3: Cross-Feature Combinations (>= 5 tests)
    - Tier 4: Real-World Application Scenarios (>= 5 tests)
    Total: 62 test cases.

    Validates the 5 benchmark core requirements:
    R1: 40 isolated workspaces (junction integrity, prisma client independence, no shared lock contention)
    R2: End-to-end task execution (schema.prisma, domain rbac, auth accessControl, navigation UI)
    R3: Tool permutation matrix & timing (40 distinct configurations, high-precision wall-clock timing)
    R4: Programmatic verification (Prisma, Vitest, TSC, AST assert, Build)
    R5: Consolidated benchmark reporting (Markdown, JSON database, CSV matrix)
.PARAMETER Tier
    Which tier to execute: 'all', '1', '2', '3', '4', 'tier1', 'tier2', 'tier3', 'tier4', 'feature', 'boundary', 'cross', 'realworld'. Defaults to 'all'.
.PARAMETER WorkspaceRoot
    Root folder containing isolated workspaces ws_01..ws_40. Defaults to 'c:\Users\krish\Downloads\antigravitytoolbuild\workspaces'.
.PARAMETER ResultsDir
    Root folder containing consolidated benchmark results. Defaults to 'c:\Users\krish\Downloads\antigravitytoolbuild\results'.
.PARAMETER UseFixtures
    If specified, runs tests against built-in fixtures rather than live workspace copies.
.PARAMETER Detailed
    If specified, prints granular pass/fail assertions for every individual test case.
.PARAMETER JsonOutput
    Optional path to export structured test execution results JSON.
.EXAMPLE
    .\scripts\test_e2e_benchmark_suite.ps1 -Tier all -Detailed
.EXAMPLE
    .\scripts\test_e2e_benchmark_suite.ps1 -Tier 1
.EXAMPLE
    .\scripts\test_e2e_benchmark_suite.ps1 -Tier 2 -UseFixtures -JsonOutput "results\e2e_test_report.json"
#>

param(
    [ValidateSet('all', '1', '2', '3', '4', 'tier1', 'tier2', 'tier3', 'tier4', 'feature', 'boundary', 'cross', 'realworld')]
    [string]$Tier = "all",

    [string]$WorkspaceRoot = "c:\Users\krish\Downloads\antigravitytoolbuild\workspaces",
    [string]$ResultsDir = "c:\Users\krish\Downloads\antigravitytoolbuild\results",
    [switch]$UseFixtures = $false,
    [switch]$Detailed = $false,
    [string]$JsonOutput = ""
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $scriptDir
$runnerScript = Join-Path $projectRoot "tests\e2e\runner.ts"

if (-not (Test-Path $runnerScript)) {
    Write-Error "Runner script not found at: $runnerScript"
    exit 1
}

# Construct command line arguments
$argsList = @(
    "--experimental-strip-types",
    $runnerScript,
    "--tier", $Tier,
    "--workspace-root", $WorkspaceRoot,
    "--results-dir", $ResultsDir
)

if ($UseFixtures) {
    $argsList += "--use-fixtures"
}
if ($Detailed) {
    $argsList += "--detailed"
}
if ($JsonOutput) {
    $argsList += @("--json-output", $JsonOutput)
}

Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host " Launching AgaateApp Benchmark E2E Test Suite via Node Runner" -ForegroundColor Cyan
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host " Tier Selection:   $Tier" -ForegroundColor Gray
Write-Host " Workspace Root:   $WorkspaceRoot" -ForegroundColor Gray
Write-Host " Results Dir:      $ResultsDir" -ForegroundColor Gray
Write-Host " Force Fixtures:   $UseFixtures" -ForegroundColor Gray
Write-Host " Detailed Output:  $Detailed" -ForegroundColor Gray
Write-Host " Runner Script:    $runnerScript" -ForegroundColor Gray
Write-Host "--------------------------------------------------------------------------------`n" -ForegroundColor DarkGray

$process = Start-Process -FilePath "node" -ArgumentList $argsList -NoNewWindow -PassThru -Wait
$exitCode = $process.ExitCode

if ($exitCode -eq 0) {
    Write-Host "`n>>> [SUCCESS] All E2E Benchmark Test Suites PASSED cleanly with exit code 0." -ForegroundColor Green
} else {
    Write-Host "`n>>> [FAILURE] E2E Benchmark Test Suites encountered failures (Exit code: $exitCode)." -ForegroundColor Red
}

exit $exitCode
