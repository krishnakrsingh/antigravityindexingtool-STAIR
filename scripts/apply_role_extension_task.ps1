<#
.SYNOPSIS
    Applies the Role Extension task (AUDITOR & FIELD_INSPECTOR) across all 4 layers of a workspace.
.PARAMETER WorkspacePath
    Path to isolated workspace directory.
#>
param(
    [Parameter(Mandatory=$true)]
    [string]$WorkspacePath
)

$ErrorActionPreference = "Stop"
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)

if (-not (Test-Path $WorkspacePath)) {
    throw "Workspace path does not exist: $WorkspacePath"
}

# 1. prisma/schema.prisma
$prismaPath = Join-Path $WorkspacePath "prisma\schema.prisma"
if (Test-Path $prismaPath) {
    $pText = [System.IO.File]::ReadAllText($prismaPath)
    if ($pText -notmatch "\bAUDITOR\b") {
        $pText = $pText -replace '(?s)(\benum Role\s*\{.*?)(\s*\})', "`$1`r`n  AUDITOR`r`n  FIELD_INSPECTOR`r`n}"
        [System.IO.File]::WriteAllText($prismaPath, $pText, $utf8NoBom)
    }
}

# 2. src/modules/auth/domain/rbac.ts
$rbacPath = Join-Path $WorkspacePath "src\modules\auth\domain\rbac.ts"
if (Test-Path $rbacPath) {
    $rText = [System.IO.File]::ReadAllText($rbacPath)
    if ($rText -notmatch "\bAUDITOR\b") {
        # ALL_ROLES
        $rText = $rText -replace '("DIRECT_FARMER",?\s*\];)', "`"DIRECT_FARMER`",`r`n  `"AUDITOR`",`r`n  `"FIELD_INSPECTOR`",`r`n];"

        # ROLE_PERMISSIONS
        $permAdd = "`"chat:use`",`r`n    `"issues:read`",`r`n    `"issues:write`",`r`n  ],`r`n  AUDITOR: [`r`n    `"analytics:read`",`r`n    `"farms:read_all`",`r`n    `"plans:read`",`r`n    `"tasks:read`",`r`n    `"issues:read`",`r`n  ],`r`n  FIELD_INSPECTOR: [`r`n    `"farms:read_all`",`r`n    `"tasks:read`",`r`n    `"issues:read`",`r`n    `"issues:write`",`r`n    `"field_ops:execute`",`r`n  ],`r`n};"
        $rText = $rText -replace '("chat:use",\s*"issues:read",\s*"issues:write",?\s*\]\s*,?\s*\};)', $permAdd

        # ROLE_CATALOG
        $catAdd = "`"Request upgrade to Enterprise Farm Admin`",`r`n    ],`r`n  },`r`n  AUDITOR: {`r`n    value: `"AUDITOR`",`r`n    label: `"Auditor`",`r`n    shortLabel: `"Auditor`",`r`n    description: `"Compliance and auditing specialist for platform and farm records.`",`r`n    tier: `"hq`",`r`n    scope: `"platform`",`r`n    capabilities: [`r`n      `"Review farm activity logs`",`r`n      `"Audit harvest records`",`r`n      `"Read compliance and analytics reports`",`r`n    ],`r`n  },`r`n  FIELD_INSPECTOR: {`r`n    value: `"FIELD_INSPECTOR`",`r`n    label: `"Field Inspector`",`r`n    shortLabel: `"Inspector`",`r`n    description: `"Field compliance and physical quality inspection agent.`",`r`n    tier: `"field`",`r`n    scope: `"assigned`",`r`n    capabilities: [`r`n      `"Inspect farm plots and crops`",`r`n      `"Record field observations and issues`",`r`n      `"Verify compliance protocols`",`r`n    ],`r`n  },`r`n};"
        $rText = $rText -replace '("Request upgrade to Enterprise Farm Admin",?\s*\],?\s*\},?\s*\};)', $catAdd

        [System.IO.File]::WriteAllText($rbacPath, $rText, $utf8NoBom)
    }
}

# 3. src/modules/auth/domain/rolePolicy.ts
$policyPath = Join-Path $WorkspacePath "src\modules\auth\domain\rolePolicy.ts"
if (Test-Path $policyPath) {
    $polText = [System.IO.File]::ReadAllText($policyPath)
    if ($polText -notmatch "\bAUDITOR\b") {
        $polText = $polText -replace "/\*\*\s*System roles seeded into RoleDefinition — mirrors rbac\.ts catalog\.\s*\*/", "/** System roles seeded into RoleDefinition — mirrors rbac.ts catalog (including AUDITOR and FIELD_INSPECTOR). */"
        [System.IO.File]::WriteAllText($policyPath, $polText, $utf8NoBom)
    }
}

# 4. src/modules/auth/domain/roleSurfaces.ts
$surfacesPath = Join-Path $WorkspacePath "src\modules\auth\domain\roleSurfaces.ts"
if (Test-Path $surfacesPath) {
    $sText = [System.IO.File]::ReadAllText($surfacesPath)
    if ($sText -notmatch "\bAUDITOR\b") {
        $sText = $sText -replace 'prefix:\s*"/hq/system",\s*roles:\s*\["SUPER_ADMIN"\]', 'prefix: "/hq/system", roles: ["SUPER_ADMIN", "AUDITOR"]'
        $sText = $sText -replace 'prefix:\s*"/officer/day",\s*roles:\s*\["SUPER_ADMIN",\s*"FARM_OFFICER"\]', 'prefix: "/officer/day", roles: ["SUPER_ADMIN", "FARM_OFFICER", "FIELD_INSPECTOR"]'
        if ($sText -notmatch "ROLE_SURFACE_MAP") {
            $sText = $sText + "`r`n`r`n/** Alias for E2E surface compatibility */`r`nexport const ROLE_SURFACE_MAP = ROLE_SURFACES;`r`n"
        }
        [System.IO.File]::WriteAllText($surfacesPath, $sText, $utf8NoBom)
    }
}

# 5. src/components/navigation/config.ts
$configPath = Join-Path $WorkspacePath "src\components\navigation\config.ts"
if (Test-Path $configPath) {
    $cText = [System.IO.File]::ReadAllText($configPath)
    if ($cText -notmatch "\bAUDITOR\b") {
        # BAR_ORDER
        $barAdd = "const DIRECT_FARMER_BAR = [`"/farmer`", `"/farmer/farm`", `"/chat`", `"/farmer/settings`"];`r`nconst AUDITOR_BAR = [`"/hq/system`"];`r`nconst FIELD_INSPECTOR_BAR = [`"/officer/day`"];`r`n`r`nconst BAR_ORDER: Partial<Record<Role, string[]>> = {`r`n  FARM_ADMIN: FARM_ADMIN_BAR,`r`n  FARM_OFFICER: FARM_OFFICER_BAR,`r`n  AGRONOMIST: AGRONOMIST_BAR,`r`n  SUPER_ADMIN: SUPER_ADMIN_BAR,`r`n  DIRECT_FARMER: DIRECT_FARMER_BAR,`r`n  AUDITOR: AUDITOR_BAR,`r`n  FIELD_INSPECTOR: FIELD_INSPECTOR_BAR,`r`n};"
        $cText = $cText -replace 'const DIRECT_FARMER_BAR = \["/farmer", "/farmer/farm", "/chat", "/farmer/settings"\];\s*const BAR_ORDER: Partial<Record<Role, string\[\]>> = \{[^}]+\};', $barAdd

        # NAV_ITEMS /officer/day
        $cText = $cText -replace '(href:\s*"/officer/day",\s*label:\s*"Today",\s*icon:\s*"Home",\s*roles:\s*\[)"FARM_OFFICER"(\])', '$1"FARM_OFFICER", "FIELD_INSPECTOR"$2'

        # NAV_ITEMS /hq/system
        $cText = $cText -replace '(href:\s*"/hq/system",\s*label:\s*"System Governance",\s*icon:\s*"Settings",\s*roles:\s*\[)"SUPER_ADMIN"(\])', '$1"SUPER_ADMIN", "AUDITOR"$2'

        # ROLE_LABELS & ROLE_HOME_URLS
        $cText = $cText -replace 'DIRECT_FARMER:\s*"FARMER",', "DIRECT_FARMER: `"FARMER`",`r`n  AUDITOR: `"AUDITOR`",`r`n  FIELD_INSPECTOR: `"FIELD INSPECTOR`","
        $cText = $cText -replace 'DIRECT_FARMER:\s*"/farmer",', "DIRECT_FARMER: `"/farmer`",`r`n  AUDITOR: `"/hq/system`",`r`n  FIELD_INSPECTOR: `"/officer/day`","

        [System.IO.File]::WriteAllText($configPath, $cText, $utf8NoBom)
    }
}

# 6. src/app/(control)/hq/farms/hq-farm-pages.authz.test.ts
$testAuthzPath = Join-Path $WorkspacePath "src\app\(control)\hq\farms\hq-farm-pages.authz.test.ts"
if (Test-Path $testAuthzPath) {
    $tText = [System.IO.File]::ReadAllText($testAuthzPath)
    if ($tText -notmatch "\bAUDITOR\b") {
        $tText = $tText -replace 'DIRECT_FARMER:\s*"self",\s*\};', "DIRECT_FARMER: `"self`",`r`n  AUDITOR: `"platform`",`r`n  FIELD_INSPECTOR: `"assigned`",`r`n};"
        [System.IO.File]::WriteAllText($testAuthzPath, $tText, $utf8NoBom)
    }
}

Write-Host "Role extension mutation successfully applied to $WorkspacePath" -ForegroundColor Green
