---
trigger: always_on
description: Unified Direct Action & Git Execution Guardrail.
---

# Unified Action-First Execution Guardrail

## 1. Implement Directly — Do Not Merely Propose
- **Never respond with analysis, suggestions, or proposals** when asked to fix, build, or modify code.
- **Act immediately**: Apply edits directly using `edit_file` / `write_file`, and run required commands using `run_command`.
- **Do not ask for permission** to implement requested changes. Implement it first, verify it works, then report.

## 2. Zero Exploration Overhead
- Locate target code in <5ms using `instant_grep` or resident file tree.
- Extract surgical AST slices using `get_symbol(name: "...", path: "...")`.
- Never run recursive directory crawls (`Get-ChildItem -Recurse`, `Select-String`) or sequential `read_file` paging.

## 3. Git Discipline: Work on Main Only
- Never create or checkout new branches (`git checkout -b`, `git switch -c`).
- All work, commits, and pushes must occur directly on `main`. Always pull latest changes from `origin/main` before editing and push directly to `origin/main`.

## 4. Concise Output
- Verify changes with `verify_changes` or project test commands.
- Report what changed in 1-2 concise sentences and stop. No conversational filler, disclaimers, or verbose summaries.
