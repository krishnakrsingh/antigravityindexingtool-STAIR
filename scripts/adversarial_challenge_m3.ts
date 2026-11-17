/**
 * Adversarial Challenge Suite for Milestone 3 Verification
 * Run by: challenger_m3_1
 * Purpose: Independently stress-test benchmark data integrity, timing monotonicity,
 * schema validity, duration positivity, CSV consistency, and detect mock/synthetic data.
 */

import * as fs from 'fs';
import * as path from 'path';

interface TimingBreakdown {
  t_recon_ms: number;
  t_inspect_ms: number;
  t_mutation_ms: number;
  t_codegen_ms: number;
  t_verify_ms: number;
  t_total_ms: number;
}

interface VerificationDetails {
  prisma: boolean;
  tsc: boolean;
  vitest: boolean;
  ast: boolean;
  build: boolean;
}

interface BenchmarkRecord {
  permutation_id: string;
  workspace_id: string;
  name: string;
  archetype: string;
  category: string;
  recon_axis: string;
  inspection_axis: string;
  mutation_axis: string;
  verification_depth: string;
  workspace_path: string;
  start_time_utc: string;
  end_time_utc: string;
  duration_ms: number;
  wall_clock_seconds: number;
  task_status: string;
  verification_status: string;
  verification_details: VerificationDetails;
  timing_breakdown: TimingBreakdown;
  phase_timings_ms: Record<string, number>;
  exit_codes: Record<string, number | null>;
  error_message: string | null;
}

interface MetaSummary {
  benchmark_meta: {
    suite_name: string;
    run_mode: string;
    timestamp_utc: string;
    host_platform: string;
    total_permutations_tested: number;
    total_duration_seconds: number;
    baseline_duration_ms: number;
  };
  summary_metrics: {
    total_tested: number;
    succeeded: number;
    failed: number;
    success_rate_percent: number;
    median_duration_ms: number;
    fastest_permutation: string;
    slowest_permutation: string;
  };
  permutations: BenchmarkRecord[];
}

interface TestResult {
  suite: string;
  name: string;
  passed: boolean;
  error?: string;
  details?: Record<string, any>;
}

const results: TestResult[] = [];

function assert(condition: boolean, suite: string, name: string, errorMsg: string, details?: Record<string, any>) {
  if (condition) {
    results.push({ suite, name, passed: true, details });
  } else {
    results.push({ suite, name, passed: false, error: errorMsg, details });
  }
}

async function runAdversarialChallenges() {
  const root = process.cwd();
  const benchResultsJsonPath = path.join(root, 'benchmark_results', 'benchmark_results.json');
  const benchMetaJsonPath = path.join(root, 'benchmark_results', 'benchmark_meta_summary.json');
  const permutationsJsonPath = path.join(root, 'scripts', 'permutations.json');
  const csvPath1 = path.join(root, 'benchmark_results', 'benchmark_results.csv');
  const csvPath2 = path.join(root, 'benchmark_results', 'benchmark_matrix.csv');
  const csvPath3 = path.join(root, 'results', 'benchmark_matrix.csv');

  console.log('================================================================');
  console.log(' ADVERSARIAL CHALLENGE SUITE: MILESTONE 3 VERIFICATION');
  console.log('================================================================');

  // -------------------------------------------------------------
  // SUITE 1: FILE EXISTENCE & ARTIFACT SYNCHRONIZATION
  // -------------------------------------------------------------
  const suite1 = 'Artifact Existence & Integrity';
  assert(fs.existsSync(benchResultsJsonPath), suite1, 'benchmark_results.json exists', 'Missing benchmark_results.json');
  assert(fs.existsSync(benchMetaJsonPath), suite1, 'benchmark_meta_summary.json exists', 'Missing benchmark_meta_summary.json');
  assert(fs.existsSync(csvPath1), suite1, 'benchmark_results.csv exists', 'Missing benchmark_results.csv');
  assert(fs.existsSync(csvPath2), suite1, 'benchmark_matrix.csv exists', 'Missing benchmark_matrix.csv');
  assert(fs.existsSync(csvPath3), suite1, 'results/benchmark_matrix.csv exists', 'Missing results/benchmark_matrix.csv');

  const runs: BenchmarkRecord[] = JSON.parse(fs.readFileSync(benchResultsJsonPath, 'utf8'));
  const meta: MetaSummary = JSON.parse(fs.readFileSync(benchMetaJsonPath, 'utf8'));
  const permsDef = JSON.parse(fs.readFileSync(permutationsJsonPath, 'utf8'));

  // -------------------------------------------------------------
  // SUITE 2: PERMUTATION COUNT, ID UNIQUENESS & ARCHETYPE COVERAGE
  // -------------------------------------------------------------
  const suite2 = 'Permutation Count & Archetype Matrix';
  assert(runs.length === 40, suite2, 'Permutations count strictly 40', `Expected exactly 40 runs, found ${runs.length}`);
  assert(permsDef.length === 40, suite2, 'Permutations definition count strictly 40', `Expected 40 definitions, found ${permsDef.length}`);

  const uniqueIds = new Set(runs.map(r => r.permutation_id));
  assert(uniqueIds.size === 40, suite2, 'Permutation IDs strictly unique', `Expected 40 unique IDs, found ${uniqueIds.size}`);

  const expectedIds = Array.from({ length: 40 }, (_, i) => `P${String(i + 1).padStart(2, '0')}`);
  const missingExpected = expectedIds.filter(id => !uniqueIds.has(id));
  assert(missingExpected.length === 0, suite2, 'Complete P01-P40 sequence present', `Missing IDs: ${missingExpected.join(', ')}`);

  // Archetype distribution check: 5 archetypes, 8 each
  const archetypeCounts: Record<string, number> = {};
  for (const r of runs) {
    archetypeCounts[r.archetype] = (archetypeCounts[r.archetype] || 0) + 1;
  }
  const requiredArchetypes = [
    'FastCore_Rust_MCP',
    'Stock_Native_AI',
    'Shell_CLI_Native',
    'Programmatic_AST',
    'Hybrid_Pipeline'
  ];
  for (const arch of requiredArchetypes) {
    assert(
      archetypeCounts[arch] === 8,
      suite2,
      `Archetype balance: ${arch} has exactly 8 permutations`,
      `Expected 8 for ${arch}, got ${archetypeCounts[arch]}`
    );
  }

  // -------------------------------------------------------------
  // SUITE 3: TIMING MONOTONICITY & UTC TIMESTAMP VALIDITY
  // -------------------------------------------------------------
  const suite3 = 'Timing Monotonicity & ISO 8601 Compliance';
  const isoUtcRegex = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;

  let allTimestampsValid = true;
  let allMonotonic = true;
  let allDurationsMatchDiff = true;
  let allWallClockAccurate = true;
  const invalidTimestampErrors: string[] = [];

  for (const r of runs) {
    if (!isoUtcRegex.test(r.start_time_utc)) {
      allTimestampsValid = false;
      invalidTimestampErrors.push(`${r.permutation_id} start_time_utc invalid: ${r.start_time_utc}`);
    }
    if (!isoUtcRegex.test(r.end_time_utc)) {
      allTimestampsValid = false;
      invalidTimestampErrors.push(`${r.permutation_id} end_time_utc invalid: ${r.end_time_utc}`);
    }

    const startMs = Date.parse(r.start_time_utc);
    const endMs = Date.parse(r.end_time_utc);

    if (isNaN(startMs) || isNaN(endMs)) {
      allTimestampsValid = false;
      invalidTimestampErrors.push(`${r.permutation_id} NaN timestamp: ${r.start_time_utc} or ${r.end_time_utc}`);
    }

    if (endMs <= startMs) {
      allMonotonic = false;
      invalidTimestampErrors.push(`${r.permutation_id} non-monotonic: start=${r.start_time_utc}, end=${r.end_time_utc}`);
    }

    const elapsedMs = endMs - startMs;
    // Allow up to 1ms tolerance due to integer millisecond rounding in ISO string representation
    if (Math.abs(elapsedMs - r.duration_ms) > 1.0) {
      allDurationsMatchDiff = false;
      invalidTimestampErrors.push(`${r.permutation_id} elapsedMs (${elapsedMs}) != duration_ms (${r.duration_ms})`);
    }

    const expectedWallSec = Math.round((r.duration_ms / 1000.0) * 1000) / 1000;
    if (Math.abs(r.wall_clock_seconds - expectedWallSec) > 0.001) {
      allWallClockAccurate = false;
      invalidTimestampErrors.push(`${r.permutation_id} wall_clock_seconds (${r.wall_clock_seconds}) != expected (${expectedWallSec})`);
    }
  }

  assert(allTimestampsValid, suite3, 'Strict ISO 8601 UTC regex on all 40 runs', invalidTimestampErrors.join('; '));
  assert(allMonotonic, suite3, 'Strict start_time < end_time monotonicity across all 40 runs', invalidTimestampErrors.join('; '));
  assert(allDurationsMatchDiff, suite3, 'Duration_ms matches (end_time - start_time) within 1ms tolerance', invalidTimestampErrors.join('; '));
  assert(allWallClockAccurate, suite3, 'wall_clock_seconds equals duration_ms / 1000 to 3 decimal places', invalidTimestampErrors.join('; '));

  // -------------------------------------------------------------
  // SUITE 4: DURATION POSITIVITY, GRANULARITY & SYNTHETIC DATA CHECKS
  // -------------------------------------------------------------
  const suite4 = 'Duration Positivity & Synthetic Data Defense';
  let allPositive = true;
  let allBreakdownsConsistent = true;
  const durations: number[] = [];

  for (const r of runs) {
    if (r.duration_ms <= 0 || r.wall_clock_seconds <= 0) {
      allPositive = false;
    }
    durations.push(r.duration_ms);

    const b = r.timing_breakdown;
    if (!b || b.t_recon_ms <= 0 || b.t_inspect_ms <= 0 || b.t_mutation_ms <= 0 || b.t_total_ms <= 0) {
      allBreakdownsConsistent = false;
    }

    if (b.t_total_ms !== r.duration_ms) {
      allBreakdownsConsistent = false;
    }
  }

  assert(allPositive, suite4, 'All 40 duration_ms and wall_clock_seconds strictly > 0', 'Detected zero or negative durations');
  assert(allBreakdownsConsistent, suite4, 'Granular phase timing breakdown consistent and positive', 'Inconsistent phase breakdown');

  // Statistical variance check: ensure timings are not synthetic constants
  const meanDuration = durations.reduce((a, b) => a + b, 0) / durations.length;
  const variance = durations.reduce((sum, d) => sum + Math.pow(d - meanDuration, 2), 0) / durations.length;
  const stdDev = Math.sqrt(variance);

  assert(stdDev > 10000, suite4, `Execution duration variance check (stdDev=${stdDev.toFixed(2)}ms > 10000ms)`, 'Suspiciously uniform timing durations');

  // Check non-integer millisecond precision (real Stopwatch has decimal sub-millisecond precision)
  const fractionalCount = durations.filter(d => !Number.isInteger(d)).length;
  assert(fractionalCount >= 35, suite4, `Sub-millisecond Stopwatch precision (${fractionalCount}/40 fractional)`, 'Suspicious integer-only timings detected');

  // -------------------------------------------------------------
  // SUITE 5: BASELINE SPEEDUP MATHEMATICAL VERIFICATION
  // -------------------------------------------------------------
  const suite5 = 'Baseline Speedup Formula & Accuracy';
  const stockRuns = runs.filter(r => r.archetype === 'Stock_Native_AI');
  const computedStockAvg = stockRuns.reduce((sum, r) => sum + r.duration_ms, 0) / stockRuns.length;
  const baselineInMeta = meta.benchmark_meta.baseline_duration_ms;

  assert(
    Math.abs(computedStockAvg - baselineInMeta) < 0.01,
    suite5,
    `Baseline matches Stock_Native_AI mean (${computedStockAvg.toFixed(2)}ms)`,
    `Meta baseline ${baselineInMeta} does not match computed average ${computedStockAvg}`
  );

  let allSpeedupsCorrect = true;
  for (const r of runs) {
    const expectedSpeedup = Math.round((baselineInMeta / r.duration_ms) * 100) / 100;
    // Verify against meta summary or csv
    if (expectedSpeedup <= 0 || isNaN(expectedSpeedup) || !isFinite(expectedSpeedup)) {
      allSpeedupsCorrect = false;
    }
  }
  assert(allSpeedupsCorrect, suite5, 'All 40 speedup factors are positive, finite, and mathematically valid', 'Invalid speedup detected');

  // -------------------------------------------------------------
  // SUITE 6: CSV CONSISTENCY & SCHEMA CONFORMANCE
  // -------------------------------------------------------------
  const suite6 = 'CSV Consistency & 1:1 Record Alignment';

  for (const [csvFile, p] of [['benchmark_results.csv', csvPath1], ['benchmark_matrix.csv', csvPath2], ['results/benchmark_matrix.csv', csvPath3]] as const) {
    const lines = fs.readFileSync(p, 'utf8').trim().split(/\r?\n/);
    assert(lines.length === 41, suite6, `${csvFile} line count strictly 41 (1 header + 40 data)`, `Expected 41 lines, found ${lines.length}`);

    const header = lines[0];
    const expectedHeader = 'Permutation_ID,Name,Archetype,Wall_Clock_Seconds,Duration_MS,Task_Status,Verification_Status,Speedup_Vs_Baseline';
    assert(header === expectedHeader, suite6, `${csvFile} header matches expected schema`, `Header mismatch: ${header}`);

    let allRowsMatch = true;
    for (let i = 1; i <= 40; i++) {
      const parts = lines[i].split(',');
      const [permId, name, archetype, wallSec, durMs, taskStatus, verStatus, speedupStr] = parts;
      const expectedRecord = runs[i - 1];

      if (
        permId !== expectedRecord.permutation_id ||
        name !== expectedRecord.name ||
        archetype !== expectedRecord.archetype ||
        parseFloat(wallSec) !== expectedRecord.wall_clock_seconds ||
        parseFloat(durMs) !== expectedRecord.duration_ms ||
        taskStatus !== expectedRecord.task_status ||
        verStatus !== expectedRecord.verification_status
      ) {
        allRowsMatch = false;
      }

      const expectedSpeedupStr = (Math.round((baselineInMeta / expectedRecord.duration_ms) * 100) / 100).toFixed(2) + 'x';
      if (speedupStr !== expectedSpeedupStr) {
        allRowsMatch = false;
      }
    }
    assert(allRowsMatch, suite6, `${csvFile} data rows match JSON records exactly in all columns`, `Row mismatch in ${csvFile}`);
  }

  // -------------------------------------------------------------
  // SUITE 7: PHYSICAL WORKSPACE DISK MUTATION AUDIT
  // -------------------------------------------------------------
  const suite7 = 'Physical Workspace Disk Mutation Audit';
  const testWorkspaceIndices = [1, 8, 14, 25, 40];

  for (const idx of testWorkspaceIndices) {
    const wsName = `ws_${String(idx).padStart(2, '0')}`;
    const wsDir = path.join(root, 'workspaces', wsName);

    assert(fs.existsSync(wsDir), suite7, `Workspace ${wsName} directory exists on disk`, `Missing ${wsDir}`);

    const schemaPath = path.join(wsDir, 'prisma', 'schema.prisma');
    assert(fs.existsSync(schemaPath), suite7, `${wsName} prisma/schema.prisma exists`, `Missing schema in ${wsName}`);

    const schemaContent = fs.readFileSync(schemaPath, 'utf8');
    assert(
      schemaContent.includes('AUDITOR') && schemaContent.includes('FIELD_INSPECTOR'),
      suite7,
      `${wsName} schema.prisma contains AUDITOR and FIELD_INSPECTOR enums`,
      `Schema in ${wsName} lacks target roles`
    );

    const rbacPath = path.join(wsDir, 'src', 'modules', 'auth', 'domain', 'rbac.ts');
    assert(fs.existsSync(rbacPath), suite7, `${wsName} domain rbac.ts exists`, `Missing rbac.ts in ${wsName}`);
    const rbacContent = fs.readFileSync(rbacPath, 'utf8');
    assert(
      rbacContent.includes('AUDITOR') && rbacContent.includes('FIELD_INSPECTOR'),
      suite7,
      `${wsName} rbac.ts defines AUDITOR and FIELD_INSPECTOR roles and permissions`,
      `rbac.ts in ${wsName} lacks target roles`
    );

    const navConfigPath = path.join(wsDir, 'src', 'components', 'navigation', 'config.ts');
    assert(fs.existsSync(navConfigPath), suite7, `${wsName} navigation config.ts exists`, `Missing config.ts in ${wsName}`);
    const navContent = fs.readFileSync(navConfigPath, 'utf8');
    assert(
      navContent.includes('AUDITOR') && navContent.includes('FIELD_INSPECTOR'),
      suite7,
      `${wsName} config.ts registers navigation items for AUDITOR and FIELD_INSPECTOR`,
      `config.ts in ${wsName} lacks target roles`
    );
  }

  // -------------------------------------------------------------
  // SUITE 8: HARNESS LOG ARTIFACTS AUDIT
  // -------------------------------------------------------------
  const suite8 = 'Benchmark Process Log Evidence';
  const logsDir = path.join(root, 'benchmark_results', 'logs');
  assert(fs.existsSync(logsDir), suite8, 'benchmark_results/logs directory exists', 'Missing logs directory');

  let allLogFilesPresent = true;
  for (const r of runs) {
    const resultJson = path.join(logsDir, `${r.permutation_id}_result.json`);
    if (!fs.existsSync(resultJson)) {
      allLogFilesPresent = false;
    }
  }
  assert(allLogFilesPresent, suite8, 'All 40 individual run results JSON files exist in logs/', 'Missing individual run json logs');

  // Summary of results
  console.log('\n================================================================');
  console.log(' ADVERSARIAL CHALLENGE EXECUTION SUMMARY');
  console.log('================================================================');

  let passed = 0;
  let failed = 0;
  for (const r of results) {
    if (r.passed) {
      passed++;
      console.log(`  [PASS] [${r.suite}] ${r.name}`);
    } else {
      failed++;
      console.log(`  [FAIL] [${r.suite}] ${r.name} -> ${r.error}`);
    }
  }

  console.log('----------------------------------------------------------------');
  console.log(`Total Assertions: ${results.length}`);
  console.log(`Passed:           ${passed}`);
  console.log(`Failed:           ${failed}`);
  console.log('================================================================');

  if (failed > 0) {
    process.exit(1);
  } else {
    process.exit(0);
  }
}

runAdversarialChallenges().catch(err => {
  console.error('Fatal execution error:', err);
  process.exit(1);
});
