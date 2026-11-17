/**
 * Standalone Automated Multi-Layer Verification Script for Role Extension
 * 
 * Verifies 4 layers of role extension integrity against any target workspace path:
 *   Layer 1: Prisma Schema enum includes AUDITOR and FIELD_INSPECTOR (or REGIONAL_DIRECTOR)
 *   Layer 2: TypeScript domain RBAC (src/modules/auth/domain/rbac.ts) defines new roles in
 *            ALL_ROLES, non-empty ROLE_PERMISSIONS, and complete ROLE_CATALOG metadata.
 *   Layer 3: Role surfaces (src/modules/auth/domain/roleSurfaces.ts) maps authorized surfaces.
 *   Layer 4: Navigation config (src/components/navigation/config.ts) has ROLE_HOME_URLS
 *            and accessible nav items via getNavForRole.
 *
 * Exit code: 0 on complete pass, 1 on failure.
 * Output: Detailed JSON object with verification status per check.
 *
 * Usage:
 *   npx tsx scripts/verify_role_extension.ts [target_workspace_path]
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { pathToFileURL } from "node:url";

export interface VerificationResult {
  workspace: string;
  timestamp: string;
  passed: boolean;
  target_roles: string[];
  summary: {
    total_checks: number;
    passed_checks: number;
    failed_checks: number;
  };
  layers: {
    layer1_prisma_schema: LayerResult;
    layer2_rbac_domain: LayerResult;
    layer3_role_surfaces: LayerResult;
    layer4_navigation: LayerResult;
  };
  checks: Record<string, boolean>;
  errors?: string[];
}

export interface LayerResult {
  passed: boolean;
  checks: Record<string, boolean>;
  details?: Record<string, unknown>;
}

async function verifyWorkspace(targetDir: string): Promise<VerificationResult> {
  const resolvedDir = path.resolve(targetDir);
  const errors: string[] = [];
  const checks: Record<string, boolean> = {};

  // Check workspace existence
  if (!fs.existsSync(resolvedDir)) {
    return {
      workspace: resolvedDir,
      timestamp: new Date().toISOString(),
      passed: false,
      target_roles: [],
      summary: { total_checks: 1, passed_checks: 0, failed_checks: 1 },
      layers: {
        layer1_prisma_schema: { passed: false, checks: { workspace_exists: false } },
        layer2_rbac_domain: { passed: false, checks: { workspace_exists: false } },
        layer3_role_surfaces: { passed: false, checks: { workspace_exists: false } },
        layer4_navigation: { passed: false, checks: { workspace_exists: false } },
      },
      checks: { workspace_exists: false },
      errors: [`Workspace directory does not exist: ${resolvedDir}`],
    };
  }

  // --------------------------------------------------------------------------
  // Layer 1: Prisma Schema Integrity
  // --------------------------------------------------------------------------
  const prismaSchemaPath = path.join(resolvedDir, "prisma", "schema.prisma");
  const layer1Checks: Record<string, boolean> = {};
  let targetRoles = ["AUDITOR", "FIELD_INSPECTOR"];

  if (fs.existsSync(prismaSchemaPath)) {
    layer1Checks["prisma_file_exists"] = true;
    const schemaContent = fs.readFileSync(prismaSchemaPath, "utf-8");

    // Extract enum Role block
    const enumMatch = schemaContent.match(/enum\s+Role\s*\{([^}]+)\}/);
    if (enumMatch) {
      layer1Checks["prisma_role_enum_found"] = true;
      const enumBody = enumMatch[1];
      const hasAuditor = /\bAUDITOR\b/.test(enumBody);
      const hasFieldInspector = /\bFIELD_INSPECTOR\b/.test(enumBody);
      const hasRegionalDirector = /\bREGIONAL_DIRECTOR\b/.test(enumBody);

      layer1Checks["prisma_enum_includes_auditor"] = hasAuditor;
      layer1Checks["prisma_enum_includes_field_inspector_or_regional_director"] =
        hasFieldInspector || hasRegionalDirector;

      // Determine active target secondary role
      if (hasRegionalDirector && !hasFieldInspector) {
        targetRoles = ["AUDITOR", "REGIONAL_DIRECTOR"];
      } else if (hasFieldInspector && hasRegionalDirector) {
        targetRoles = ["AUDITOR", "FIELD_INSPECTOR", "REGIONAL_DIRECTOR"];
      }

      layer1Checks["prisma_enum_all_target_roles_present"] = targetRoles.every((role) =>
        new RegExp(`\\b${role}\\b`).test(enumBody)
      );
    } else {
      layer1Checks["prisma_role_enum_found"] = false;
      layer1Checks["prisma_enum_includes_auditor"] = false;
      layer1Checks["prisma_enum_includes_field_inspector_or_regional_director"] = false;
      layer1Checks["prisma_enum_all_target_roles_present"] = false;
    }
  } else {
    layer1Checks["prisma_file_exists"] = false;
    layer1Checks["prisma_role_enum_found"] = false;
    layer1Checks["prisma_enum_includes_auditor"] = false;
    layer1Checks["prisma_enum_includes_field_inspector_or_regional_director"] = false;
    layer1Checks["prisma_enum_all_target_roles_present"] = false;
    errors.push(`Missing Prisma schema file at: ${prismaSchemaPath}`);
  }

  const layer1Passed = Object.values(layer1Checks).every(Boolean);

  // --------------------------------------------------------------------------
  // Layer 2: TypeScript Domain RBAC (src/modules/auth/domain/rbac.ts)
  // --------------------------------------------------------------------------
  const rbacPath = path.join(resolvedDir, "src", "modules", "auth", "domain", "rbac.ts");
  const layer2Checks: Record<string, boolean> = {};
  let rbacModule: any = null;

  if (fs.existsSync(rbacPath)) {
    layer2Checks["rbac_file_exists"] = true;
    try {
      const rbacUrl = pathToFileURL(rbacPath).href;
      rbacModule = await import(rbacUrl);
      layer2Checks["rbac_module_importable"] = true;

      const allRoles: string[] = Array.isArray(rbacModule.ALL_ROLES) ? rbacModule.ALL_ROLES : [];
      const rolePermissions: Record<string, string[]> = rbacModule.ROLE_PERMISSIONS || {};
      const roleCatalog: Record<string, any> = rbacModule.ROLE_CATALOG || {};

      for (const role of targetRoles) {
        layer2Checks[`rbac_all_roles_includes_${role}`] = allRoles.includes(role);
        const perms = rolePermissions[role];
        layer2Checks[`rbac_permissions_defined_${role}`] = Array.isArray(perms) && perms.length > 0;
        const meta = roleCatalog[role];
        layer2Checks[`rbac_catalog_defined_${role}`] = Boolean(
          meta &&
            typeof meta.label === "string" &&
            meta.label.length > 0 &&
            typeof meta.tier === "string" &&
            ["hq", "client", "field"].includes(meta.tier)
        );
      }
    } catch (err: any) {
      layer2Checks["rbac_module_importable"] = false;
      errors.push(`Failed to import RBAC module: ${err?.message || String(err)}`);
      // Fallback: static source inspection
      const rbacContent = fs.readFileSync(rbacPath, "utf-8");
      for (const role of targetRoles) {
        layer2Checks[`rbac_all_roles_includes_${role}`] = new RegExp(`ALL_ROLES[\\s\\S]*?\\b${role}\\b`).test(rbacContent);
        layer2Checks[`rbac_permissions_defined_${role}`] = new RegExp(`ROLE_PERMISSIONS[\\s\\S]*?\\b${role}\\b`).test(rbacContent);
        layer2Checks[`rbac_catalog_defined_${role}`] = new RegExp(`ROLE_CATALOG[\\s\\S]*?\\b${role}\\b`).test(rbacContent);
      }
    }
  } else {
    layer2Checks["rbac_file_exists"] = false;
    layer2Checks["rbac_module_importable"] = false;
    for (const role of targetRoles) {
      layer2Checks[`rbac_all_roles_includes_${role}`] = false;
      layer2Checks[`rbac_permissions_defined_${role}`] = false;
      layer2Checks[`rbac_catalog_defined_${role}`] = false;
    }
    errors.push(`Missing RBAC file at: ${rbacPath}`);
  }

  const layer2Passed = Object.values(layer2Checks).every(Boolean);

  // --------------------------------------------------------------------------
  // Layer 3: Role Surfaces (src/modules/auth/domain/roleSurfaces.ts)
  // --------------------------------------------------------------------------
  const surfacesPath = path.join(resolvedDir, "src", "modules", "auth", "domain", "roleSurfaces.ts");
  const layer3Checks: Record<string, boolean> = {};
  let surfacesModule: any = null;

  if (fs.existsSync(surfacesPath)) {
    layer3Checks["surfaces_file_exists"] = true;
    try {
      const surfacesUrl = pathToFileURL(surfacesPath).href;
      surfacesModule = await import(surfacesUrl);
      layer3Checks["surfaces_module_importable"] = true;

      const roleSurfaces: Array<{ prefix: string; roles: string[]; gate?: string; purpose?: string }> =
        Array.isArray(surfacesModule.ROLE_SURFACES) ? surfacesModule.ROLE_SURFACES : [];

      for (const role of targetRoles) {
        const matchingSurfaces = roleSurfaces.filter(
          (s) => Array.isArray(s.roles) && s.roles.includes(role)
        );
        layer3Checks[`surface_mapped_${role}`] = matchingSurfaces.length > 0;
        layer3Checks[`surface_purpose_valid_${role}`] =
          matchingSurfaces.length > 0 &&
          matchingSurfaces.every((s) => typeof s.purpose === "string" && s.purpose.length > 0);
      }
    } catch (err: any) {
      layer3Checks["surfaces_module_importable"] = false;
      errors.push(`Failed to import roleSurfaces module: ${err?.message || String(err)}`);
      // Fallback: static regex check
      const surfacesContent = fs.readFileSync(surfacesPath, "utf-8");
      for (const role of targetRoles) {
        layer3Checks[`surface_mapped_${role}`] = new RegExp(`ROLE_SURFACES[\\s\\S]*?\\b${role}\\b`).test(surfacesContent);
        layer3Checks[`surface_purpose_valid_${role}`] = layer3Checks[`surface_mapped_${role}`];
      }
    }
  } else {
    layer3Checks["surfaces_file_exists"] = false;
    layer3Checks["surfaces_module_importable"] = false;
    for (const role of targetRoles) {
      layer3Checks[`surface_mapped_${role}`] = false;
      layer3Checks[`surface_purpose_valid_${role}`] = false;
    }
    errors.push(`Missing roleSurfaces file at: ${surfacesPath}`);
  }

  const layer3Passed = Object.values(layer3Checks).every(Boolean);

  // --------------------------------------------------------------------------
  // Layer 4: UI Navigation Mirror (src/components/navigation/config.ts)
  // --------------------------------------------------------------------------
  const configPath = path.join(resolvedDir, "src", "components", "navigation", "config.ts");
  const layer4Checks: Record<string, boolean> = {};
  let configModule: any = null;

  if (fs.existsSync(configPath)) {
    layer4Checks["nav_config_file_exists"] = true;
    try {
      const configUrl = pathToFileURL(configPath).href;
      configModule = await import(configUrl);
      layer4Checks["nav_config_module_importable"] = true;

      const roleHomeUrls: Record<string, string> = configModule.ROLE_HOME_URLS || {};
      const getNavForRole = typeof configModule.getNavForRole === "function" ? configModule.getNavForRole : null;

      for (const role of targetRoles) {
        const homeUrl = roleHomeUrls[role];
        layer4Checks[`nav_home_url_defined_${role}`] =
          typeof homeUrl === "string" && homeUrl.startsWith("/");

        if (getNavForRole) {
          try {
            const navItems = getNavForRole(role);
            layer4Checks[`nav_items_accessible_${role}`] = Array.isArray(navItems) && navItems.length > 0;
          } catch {
            layer4Checks[`nav_items_accessible_${role}`] = false;
          }
        } else {
          layer4Checks[`nav_items_accessible_${role}`] = false;
        }
      }
    } catch (err: any) {
      layer4Checks["nav_config_module_importable"] = false;
      errors.push(`Failed to import nav config module: ${err?.message || String(err)}`);
      // Fallback: static regex check
      const configContent = fs.readFileSync(configPath, "utf-8");
      for (const role of targetRoles) {
        layer4Checks[`nav_home_url_defined_${role}`] = new RegExp(`ROLE_HOME_URLS[\\s\\S]*?\\b${role}\\b`).test(configContent);
        layer4Checks[`nav_items_accessible_${role}`] = new RegExp(`roles:[\\s\\S]*?\\b${role}\\b`).test(configContent);
      }
    }
  } else {
    layer4Checks["nav_config_file_exists"] = false;
    layer4Checks["nav_config_module_importable"] = false;
    for (const role of targetRoles) {
      layer4Checks[`nav_home_url_defined_${role}`] = false;
      layer4Checks[`nav_items_accessible_${role}`] = false;
    }
    errors.push(`Missing navigation config file at: ${configPath}`);
  }

  const layer4Passed = Object.values(layer4Checks).every(Boolean);

  // Combine checks
  Object.assign(checks, layer1Checks, layer2Checks, layer3Checks, layer4Checks);

  const checkValues = Object.values(checks);
  const totalChecks = checkValues.length;
  const passedChecks = checkValues.filter(Boolean).length;
  const failedChecks = totalChecks - passedChecks;
  const overallPassed = layer1Passed && layer2Passed && layer3Passed && layer4Passed && failedChecks === 0;

  return {
    workspace: resolvedDir,
    timestamp: new Date().toISOString(),
    passed: overallPassed,
    target_roles: targetRoles,
    summary: {
      total_checks: totalChecks,
      passed_checks: passedChecks,
      failed_checks: failedChecks,
    },
    layers: {
      layer1_prisma_schema: { passed: layer1Passed, checks: layer1Checks },
      layer2_rbac_domain: { passed: layer2Passed, checks: layer2Checks },
      layer3_role_surfaces: { passed: layer3Passed, checks: layer3Checks },
      layer4_navigation: { passed: layer4Passed, checks: layer4Checks },
    },
    checks,
    ...(errors.length > 0 ? { errors } : {}),
  };
}

async function main() {
  const targetDir = process.argv[2] ? path.resolve(process.argv[2]) : process.cwd();
  try {
    const result = await verifyWorkspace(targetDir);
    console.log(JSON.stringify(result, null, 2));
    process.exit(result.passed ? 0 : 1);
  } catch (err: any) {
    console.error(
      JSON.stringify(
        {
          workspace: targetDir,
          timestamp: new Date().toISOString(),
          passed: false,
          error: err?.message || String(err),
        },
        null,
        2
      )
    );
    process.exit(1);
  }
}

// Only invoke main when run directly
if (process.argv[1] && (process.argv[1].endsWith("verify_role_extension.ts") || process.argv[1].endsWith("verify_role_extension.js"))) {
  main();
}
