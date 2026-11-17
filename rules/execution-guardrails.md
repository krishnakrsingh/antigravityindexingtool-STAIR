# Native Direct Action & Zero-Exploration Execution Guardrails

## 1. Intent Classification: Operational Task vs. Code Authoring
- When a prompt requests an operational action, cleanup, environment setup, installation, or sanity test (e.g., "clean our PC, install the repo using the command, test if it works"):
  - **TREAT AS AN OPERATIONAL TASK, NOT A CODE AUDIT.**
  - **BANNED**: Opening source files, inspecting implementation details, pre-emptive grep searches, or analyzing functions before running the requested steps.
  - **MANDATORY**: Execute the requested operational steps immediately via terminal commands.

## 2. Strict Sequential Stage Map
- Always decompose operational tasks into a strict, linear stage map:
  - **Stage 1 (Prerequisites / Cleanup)**: Purge artifacts or clean state. Execute command. Check exit code.
  - **Stage 2 (Execution / Install)**: Run installer / build script. Check exit code.
  - **Stage 3 (Verification Check That Can Fail)**: Run the sanity test / verification command.
- **NEVER branch into speculative code refactoring or pre-emptive fixes during an operational plan.** Execute the plan as requested.

## 3. The Verify-Before-Edit Gate
- **NEVER modify or inspect source code unless an executed command actually fails** with an explicit error trace or non-zero exit code.
- If all steps succeed with exit code 0, the task is complete. Report what was run, provide concrete evidence (exit code, output summary), and stop.

## 4. Surgical Diagnostic Isolation (Only Upon Verified Failure)
- If and only if a command fails during execution:
  - Isolate the single failure from the terminal stderr / error trace.
  - Use `get_symbol` to pull only the specific failing symbol or signature.
  - Do NOT paginate or read surrounding unrelated code.
  - Apply the minimal targeted fix.
  - Re-run the exact failing command immediately.

## 5. Zero Sliding-Window File Paging & Zero Token Bloat
- Never make consecutive `read_file` calls to "browse" or "audit" a repository.
- Use `instant_grep` for candidate sub-millisecond lookups.
- Use `get_symbol` for exact AST definition extraction (0.00% hallucination, 94.5% token reduction).
