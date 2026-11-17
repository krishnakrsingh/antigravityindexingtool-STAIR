---
trigger: always_on
description: Native Direct Action & Zero-Exploration Execution Guardrails. Strictly bans exploratory file paging and shell crawls; mandates instant_grep and get_symbol.
---

# Native Direct Action & Zero-Exploration Execution Guardrails

## 1. Zero Exploratory Crawling
- Consult the resident Codebase Map (injected at SessionStart) to identify target files directly.
- NEVER run PowerShell `Get-ChildItem -Recurse`, `Select-String`, or recursive directory crawls across the workspace.
- For code search, use `instant_grep` exclusively.
- NEVER make consecutive `read_file` calls to browse or paginate contiguous line ranges. Use `get_symbol` to pull exact AST slices.
- Do NOT run caller greps, whole-codebase audits, or exploratory searches before acting.

## 2. Operational Tasks vs. Code Authoring
- If a prompt requests an operational action, cleanup, installation, build, or test run:
  - TREAT AS AN OPERATIONAL TASK, NOT A CODE AUDIT.
  - Do NOT open source files or analyze functions before running the requested steps.
  - Execute terminal commands immediately.
  - If commands succeed with exit code 0, you are DONE. Do not speculate or audit code.

## 3. Surgical Code Edits
- When authoring or editing code:
  1. Pull the target symbol using `get_symbol(name: "...", path: "...")`.
  2. Apply the surgical edit directly using `edit_file`.
  3. Verify the changes using `verify_changes`.
