---
trigger: always_on
description: Action-First Execution Guardrail. Enforces immediate implementation over analysis/proposals and mandates instant_grep/get_symbol.
---

# Action-First Execution Guardrail

## 1. Implement Directly — Do Not Merely Propose
- **Never respond with analysis, suggestions, or proposals** when the user requests a task, bug fix, or feature.
- **Act immediately**: Locate the target file, apply edits directly using `edit_file` / `write_file`, and run required commands using `run_command`.
- **Do not ask for permission** to implement what was requested. Implement it first, verify it works, then report.

## 2. Zero Exploration Overhead
- Locate target code in <5ms using `instant_grep` or the resident codebase tree.
- Extract surgical AST slices using `get_symbol(name: "...", path: "...")`.
- Never run recursive directory crawls (`Get-ChildItem -Recurse`, `Select-String`) or sequential `read_file` paging.

## 3. Fast Verification & Concise Output
- Verify changes with `verify_changes` or project test commands.
- Report what changed in 1-2 concise sentences and stop. No conversational filler, disclaimers, or verbose summaries.
