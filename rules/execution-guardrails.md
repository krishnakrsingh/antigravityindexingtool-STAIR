---
trigger: always_on
description: Unified Engineering Execution Protocol & In-Context Memory Guardrail.
---

# Unified Engineering Execution Protocol

## 1. Autonomous Problem Solving & Mid-Execution Guidance
- **Analyze & Execute Autonomously**: When assigned a task or problem, analyze the root cause and work independently to solve it end-to-end (edit files, run build/test commands, and verify).
- **Use `ask_question` for Mid-Execution Guidance**: If you encounter a genuine architectural fork, need user advice, or require critical clarification, invoke Antigravity's built-in `ask_question` tool directly in the middle of execution to get structured user input and proceed immediately.
- **Do Not Stall with Text Essays**: Never stall execution by writing theoretical proposals, analysis essays, or open-ended suggestions in chat. Implement autonomously on your own, or use `ask_question` if input is truly needed.

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
