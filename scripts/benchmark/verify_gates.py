"""
4-Gate Automated Verification Suite
Executes strict 4-gate verification across benchmark workspace instances:
  Gate 1: Prisma Schema Validation (`npx prisma validate`)
  Gate 2: TypeScript Strict Compilation (`npx tsc --noEmit`)
  Gate 3: Next.js Production Build (`npx next build --webpack`)
  Gate 4: Vitest Unit Tests (`npx vitest run ...`)
Employs bounded worker pool (4 concurrent workers) to avoid RAM exhaustion.
"""

import os
import sys
import time
import json
import shutil
import subprocess
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

def verify_single_workspace(ws_path: Path) -> dict:
    start_total = time.perf_counter()
    gate_results = {
        "gate1_prisma": False,
        "gate2_tsc": False,
        "gate3_build": False,
        "gate4_vitest": False,
        "gate1_ms": 0.0,
        "gate2_ms": 0.0,
        "gate3_ms": 0.0,
        "gate4_vitest_ms": 0.0,
        "total_verify_ms": 0.0,
        "all_passed": False,
        "error_message": None
    }

    env = os.environ.copy()
    env["DATABASE_URL"] = "postgresql://mock:mock@127.0.0.1:15432/agaateapp?schema=public"
    env["NEXT_CPU_COUNT"] = "2"

    # Clean any stale .next before verification to guarantee clean typecheck
    next_dir = ws_path / ".next"
    if next_dir.exists():
        try:
            shutil.rmtree(next_dir, ignore_errors=True)
        except Exception:
            pass

    # --- GATE 1: Prisma Validation ---
    t1 = time.perf_counter()
    cmd_g1 = "npx prisma validate"
    res_g1 = subprocess.run(cmd_g1, shell=True, cwd=str(ws_path), env=env, capture_output=True, text=True)
    gate_results["gate1_ms"] = (time.perf_counter() - t1) * 1000.0
    if res_g1.returncode == 0:
        gate_results["gate1_prisma"] = True
    else:
        gate_results["error_message"] = f"Gate 1 (Prisma) failed: {res_g1.stderr or res_g1.stdout}"
        gate_results["total_verify_ms"] = (time.perf_counter() - start_total) * 1000.0
        return gate_results

    # --- GATE 2: TypeScript Typecheck ---
    t2 = time.perf_counter()
    cmd_g2 = "npx tsc --noEmit"
    res_g2 = subprocess.run(cmd_g2, shell=True, cwd=str(ws_path), env=env, capture_output=True, text=True)
    gate_results["gate2_ms"] = (time.perf_counter() - t2) * 1000.0
    if res_g2.returncode == 0:
        gate_results["gate2_tsc"] = True
    else:
        gate_results["error_message"] = f"Gate 2 (TSC) failed: {res_g2.stderr or res_g2.stdout}"
        gate_results["total_verify_ms"] = (time.perf_counter() - start_total) * 1000.0
        return gate_results

    # --- GATE 4: Vitest Unit Tests ---
    t4 = time.perf_counter()
    test_files = "src/modules/operations/domain/inventoryPolicy.test.ts src/modules/operations/application/manageInventory.test.ts"
    cmd_g4 = f"npx vitest run {test_files}"
    res_g4 = subprocess.run(cmd_g4, shell=True, cwd=str(ws_path), env=env, capture_output=True, text=True)
    gate_results["gate4_vitest_ms"] = (time.perf_counter() - t4) * 1000.0
    if res_g4.returncode == 0:
        gate_results["gate4_vitest"] = True
    else:
        gate_results["error_message"] = f"Gate 4 (Vitest) failed: {res_g4.stderr or res_g4.stdout}"
        gate_results["total_verify_ms"] = (time.perf_counter() - start_total) * 1000.0
        return gate_results

    # --- GATE 3: Next.js Production Build ---
    t3 = time.perf_counter()
    cmd_g3 = "npx next build --webpack"
    res_g3 = subprocess.run(cmd_g3, shell=True, cwd=str(ws_path), env=env, capture_output=True, text=True)
    gate_results["gate3_ms"] = (time.perf_counter() - t3) * 1000.0
    if res_g3.returncode == 0:
        gate_results["gate3_build"] = True
    else:
        gate_results["error_message"] = f"Gate 3 (Next.js Build) failed: {res_g3.stderr or res_g3.stdout}"
        gate_results["total_verify_ms"] = (time.perf_counter() - start_total) * 1000.0
        return gate_results

    gate_results["all_passed"] = True
    gate_results["total_verify_ms"] = (time.perf_counter() - start_total) * 1000.0
    return gate_results

def verify_all_workspaces(runs_dir: Path, matrix: list, max_workers: int = 4) -> dict:
    results = {}
    print(f"Starting 4-gate verification across {len(matrix)} instances (pool size: {max_workers})...")

    def run_check(p):
        folder = p["folder"]
        ws_path = runs_dir / folder
        res = verify_single_workspace(ws_path)
        return p["id"], res

    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        futures = [executor.submit(run_check, p) for p in matrix]
        for f in futures:
            p_id, res = f.result()
            results[p_id] = res
            status_str = "PASS" if res["all_passed"] else f"FAIL ({res['error_message'][:60]}...)"
            print(f"  [{status_str}] Permutation {p_id:<4} verified in {res['total_verify_ms']/1000.0:.1f}s")

    return results

if __name__ == "__main__":
    if len(sys.argv) > 1:
        target = Path(sys.argv[1]).resolve()
        res = verify_single_workspace(target)
        print(json.dumps(res, indent=2))
