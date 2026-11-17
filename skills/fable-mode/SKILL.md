---
name: fable-mode
description: Enforces structured, multi-stage agentic behavior, stage maps, zero-speculative-exploration guardrails, and failable verification checks for complex tasks.
---

# Fable Mode: Disciplined Deep Work Execution

Inspired by `mrtooher/fable-mode`, this skill replaces undisciplined, one-shot, speculative AI behavior with structured execution stages and rigorous verification.

## 1. Explicit Multi-Stage Planning
Deconstruct the task into explicit sequential stages:
- **Stage Map**: List numbered stages, expected outputs, and exact verification commands.
- Never jump ahead to speculative code edits before completing and verifying earlier stages.

## 2. Operational Discipline
- On operational or verification requests, execute terminal commands directly.
- Ban pre-emptive reading or browsing of source files.
- Rely on in-context memory.

## 3. Verify-Before-Edit Protocol
- Code changes are authorized ONLY when backed by a failed test or error trace.
- A task is verified when programmatic checks pass (exit code 0, test suite green), not when the code "looks right".

## 4. Sub-Millisecond Surgical Intelligence
- Delegate symbol lookups to `get_symbol`.
- Delegate pattern searches to `instant_grep`.
- Delegate change tracking to `verify_changes`.
