"""
Workspace Provisioning & Isolation Engine
Provisions 40 independent, isolated copies of `agaateapp` inside `benchmark_runs`.
Uses Robocopy /MT:16 for ultra-fast source copying and Windows NTFS directory
junctions (mklink /J) for node_modules to achieve fast provisioning and zero disk bloat.
Patches pre-existing TypeScript defect in `src/app/(control)/hq/people/page.tsx`.
"""

import os
import sys
import json
import time
import shutil
import subprocess
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

SOURCE_DIR = Path(r"C:\Users\krish\Downloads\agaateapp").resolve()
BENCHMARK_ROOT = Path(r"c:\Users\krish\Downloads\antigravitytoolbuild").resolve()
RUNS_DIR = BENCHMARK_ROOT / "benchmark_runs"
MATRIX_FILE = BENCHMARK_ROOT / "scripts" / "benchmark" / "permutation_matrix.json"

PEOPLE_PAGE_RELATIVE = Path("src/app/(control)/hq/people/page.tsx")

FIXED_PAGE_SNIPPET = """    prisma.user.groupBy({
      by: ["phone"],
      where: { phone: { not: null, notIn: [""] } },
      _count: { _all: true },
      having: { phone: { _count: { gt: 1 } } },
      orderBy: { phone: "asc" },
      take: 20,
    }).catch(() => []),
    prisma.user.groupBy({
      by: ["email"],
      where: { email: { notIn: [""] } },
      _count: { _all: true },
      having: { email: { _count: { gt: 1 } } },
      orderBy: { email: "asc" },
      take: 20,
    }).catch(() => []),"""

TARGET_PAGE_SNIPPET_UNFIXED = """    prisma.user.groupBy({
      by: ["phone"],
      where: { phone: { not: null, notIn: [""] } },
      _count: { _all: true },
      having: { phone: { _count: { gt: 1 } } },
      take: 20,
    }).catch(() => []),
    prisma.user.groupBy({
      by: ["email"],
      where: { email: { notIn: [""] } },
      _count: { _all: true },
      having: { email: { _count: { gt: 1 } } },
      take: 20,
    }).catch(() => []),"""

def safe_clean_workspace(ws_path: Path):
    """Safely cleans workspace, ensuring junctions are unlinked first so target node_modules is NEVER deleted."""
    if not ws_path.exists():
        return
    nm_path = ws_path / "node_modules"
    if nm_path.exists():
        # In Windows, use cmd /c rmdir to remove junction point without traversing files
        subprocess.run(f'cmd /c rmdir "{nm_path}"', shell=True, capture_output=True)
    if ws_path.exists():
        try:
            shutil.rmtree(ws_path, ignore_errors=True)
        except Exception as e:
            pass
        if ws_path.exists():
            subprocess.run(f'cmd /c rd /s /q "{ws_path}"', shell=True, capture_output=True)

def provision_instance(perm: dict) -> dict:
    start_t = time.perf_counter()
    folder_name = perm["folder"]
    ws_path = RUNS_DIR / folder_name
    ws_path = ws_path.resolve()

    # Step 1: Clean any existing folder
    safe_clean_workspace(ws_path)
    ws_path.mkdir(parents=True, exist_ok=True)

    # Step 2: Robocopy source files excluding node_modules, .next, .git, etc.
    cmd_robocopy = [
        "robocopy",
        str(SOURCE_DIR),
        str(ws_path),
        "/MIR",
        "/MT:16",
        "/XD", "node_modules", ".next", ".git", "backups", "tmp", "benchmark_workspaces", ".antigravity", "benchmark_runs",
        "/XF", "*.tmp", "*.log"
    ]
    res_rc = subprocess.run(cmd_robocopy, capture_output=True, text=True)
    # Robocopy return codes: 0-7 mean files copied or already exist; >=8 is fatal error
    if res_rc.returncode >= 8:
        raise RuntimeError(f"Robocopy failed for {folder_name} with code {res_rc.returncode}:\n{res_rc.stderr or res_rc.stdout}")

    # Step 3: Create directory junction for node_modules
    nm_source = SOURCE_DIR / "node_modules"
    nm_target = ws_path / "node_modules"
    cmd_mklink = f'cmd /c mklink /J "{nm_target}" "{nm_source}"'
    res_link = subprocess.run(cmd_mklink, shell=True, capture_output=True, text=True)
    if not nm_target.exists():
        raise RuntimeError(f"Failed to create junction for {folder_name}:\n{res_link.stderr or res_link.stdout}")

    # Step 4: Ensure pre-existing defect in people/page.tsx is patched
    page_file = ws_path / PEOPLE_PAGE_RELATIVE
    if page_file.exists():
        content = page_file.read_text(encoding="utf-8")
        if TARGET_PAGE_SNIPPET_UNFIXED in content:
            content = content.replace(TARGET_PAGE_SNIPPET_UNFIXED, FIXED_PAGE_SNIPPET)
            page_file.write_text(content, encoding="utf-8")

    # Step 5: Configure .env with Zero-DB mock tripwire
    env_file = ws_path / ".env"
    env_content = """# Benchmark Isolated Zero-DB Environment
DATABASE_URL="postgresql://mock:mock@127.0.0.1:15432/agaateapp?schema=public"
NEXT_PUBLIC_APP_URL="http://localhost:3000"
NODE_ENV="production"
PORT=3000
AUTH_SECRET="benchmark_auth_secret_01234567890123456789012345678901"
"""
    env_file.write_text(env_content, encoding="utf-8")

    # Step 6: Write tool configuration file (.antigravity/tools.json)
    ag_dir = ws_path / ".antigravity"
    ag_dir.mkdir(exist_ok=True)
    tools_config = {
        "permutation_id": perm["id"],
        "name": perm["name"],
        "category": perm["category"],
        "tools": perm["tools"],
        "flags": perm["flags"],
        "threads": perm.get("threads", "auto"),
        "cache": perm.get("cache", "warm"),
        "description": perm["description"]
    }
    (ag_dir / "tools.json").write_text(json.dumps(tools_config, indent=2), encoding="utf-8")

    # Step 7: Create .antigravity/crepe directory
    crepe_dir = ag_dir / "crepe"
    crepe_dir.mkdir(exist_ok=True)

    elapsed_ms = (time.perf_counter() - start_t) * 1000.0
    return {
        "id": perm["id"],
        "folder": folder_name,
        "path": str(ws_path),
        "elapsed_ms": elapsed_ms,
        "status": "READY"
    }

def main():
    print("=" * 80)
    print("FAST-CORE BENCHMARK WORKSPACE PROVISIONING ENGINE")
    print(f"Source Codebase: {SOURCE_DIR}")
    print(f"Target Workspaces Directory: {RUNS_DIR}")
    print("=" * 80)

    if not MATRIX_FILE.exists():
        print(f"ERROR: Matrix file not found at {MATRIX_FILE}")
        sys.exit(1)

    with open(MATRIX_FILE, "r") as f:
        matrix = json.load(f)

    RUNS_DIR.mkdir(parents=True, exist_ok=True)

    start_total = time.perf_counter()
    results = []

    print(f"Provisioning {len(matrix)} parallel isolated workspaces with Robocopy /MT:16 & NTFS junctions...")
    # Parallel provisioning with 8 workers
    with ThreadPoolExecutor(max_workers=8) as executor:
        futures = [executor.submit(provision_instance, p) for p in matrix]
        for f in futures:
            res = f.result()
            results.append(res)
            print(f"  [+] Provisioned {res['id']:<4} {res['folder']:<36} in {res['elapsed_ms']:>6.1f} ms")

    total_time_s = time.perf_counter() - start_total
    print("-" * 80)
    print(f"SUCCESS: Successfully provisioned {len(results)} isolated workspaces in {total_time_s:.2f} seconds.")
    avg_ms = sum(r['elapsed_ms'] for r in results) / len(results)
    print(f"Average provisioning latency per workspace: {avg_ms:.1f} ms")
    print("=" * 80)

if __name__ == "__main__":
    main()
