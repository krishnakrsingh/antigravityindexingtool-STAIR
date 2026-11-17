---
name: fast-code-intelligence
description: Supercharged, sub-millisecond codebase search and structural AST code intelligence. Use whenever exploring, navigating, or searching large repositories (1,000 to 50,000+ files) to eliminate search lag and context bloat.
---

# Fast Code Intelligence

`fast-code-intelligence` provides instant code navigation powered by `antigravity-fast-core`:

1. **Instant Grep (`instant_grep`)**:
   - Memory-mapped sparse n-gram inverted index (`crepe`).
   - Pinpoints candidate files in <0.05 ms, verifies with exact ripgrep engine in <3 ms.
   - 100% token precision; supports regex alternations, word boundaries, and case-insensitivity.

2. **Surgical AST Intelligence (`get_symbol`)**:
   - STAIR-RAG surgical context lens: extracts exact AST code definitions or context lenses by name or query.
   - Eliminates blind whole-file reading and achieves 94.5% token reduction.
   - Supports `mode: "definition"` (default), `mode: "outline"` (hierarchical symbol outline), and `mode: "context"` (definition + callers).

3. **Dirty-File Compiler Validation (`verify_changes`)**:
   - Microsecond BLAKE3 Merkle tree tracks uncommitted modified files.
   - Fail-fast compiler verification (`cargo check`, `tsc --noEmit`, `prisma validate`).
