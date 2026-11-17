---
trigger: always_on
description: Unified Engineering Execution Protocol & In-Context Memory Guardrail.
---

# Unified Engineering Execution Protocol

## 1. Execution Protocol: Action-First Implementation
- **Default to Direct Implementation**: When tasked with a bug fix, feature, or refactor, deliver the solution directly in code rather than outlining theoretical options or speculative analysis.
- **Autonomous Delivery**: Execute edits end-to-end using `edit_file` / `write_file`, run required build and test commands via `run_command`, and verify outcomes before reporting. Only pause to ask questions when core requirements are genuinely ambiguous.

## 2. In-Context Memory Retention (Zero Re-Analysis)
- **Rely on Active Memory**: Never re-read, re-grep, or re-analyze files and symbols that were already read, edited, or presented in earlier turns of the current conversation. 
- **Single-Pass Orientation**: Extract the required context once. Subsequent turns in the same workflow must apply changes directly to known targets without redundant orientation or exploratory calls.

## 3. High-Speed Navigation & Surgical Extraction
- **Sub-Millisecond Lookups**: Use `instant_grep` (<5ms) for unknown keywords/routes and `get_symbol` for precise AST code definitions or outlines.
- **Zero Crawling Overhead**: Consult the resident file tree (injected at session start). Never run recursive shell directory crawls (`Get-ChildItem -Recurse`, `Select-String`) or sequential `read_file` paging.

## 4. Git Discipline: Work on Main Only
- **Single Branch Workflow**: Never create or checkout new branches (`git checkout -b`, `git switch -c`).
- **Continuous Sync**: All work, commits, and pushes occur directly on `main`. Always pull latest changes with rebase before editing and push directly to `origin/main`.

## 5. Concise Verification & Delivery
- **Targeted Verification**: Validate modified files with `verify_changes` or project-specific test/build commands.
- **Crisp Technical Reporting**: State what was accomplished in 1–2 concrete sentences. Omit conversational filler, throat-clearing, and disclaimers.
