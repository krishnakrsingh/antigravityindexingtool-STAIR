# Zero Sliding-Window File Paging & Code Discovery Protocol

## 1. Zero Guessing via Shell / PowerShell
- **NEVER use `Select-String`, `git grep`, or PowerShell regex pipelines** across `src/` to guess conversational UI phrases (e.g., `"Select Plot"`, `"Choose your crop"`, `"Add Crop to Plot"`, `"Alternate Beds"`). This burns minutes in trial-and-error shell loops.
- **Always use `instant_grep`** (or `antigravity-fast-core --grep "<symbol>"`) for sub-5ms exact code search across the entire workspace.

## 2. Never Paginate Files in Sequential Chunks
- **Do not make repeated `read_file` calls** across contiguous line ranges (`#L1-200`, `#L201-350`) to browse or discover code in large files. This causes 3+ minutes of round-trip network turn latency and token exhaustion.
- **Always use AST symbols first:**
  - `get_symbol_outline(path: "path/to/file.tsx")` returns the complete AST outline showing every component, function, and interface in ~50 tokens.
  - `get_symbol(path: "path/to/file.tsx", query: "target query")` or `get_symbol_definition(path: "file.tsx", symbol_name: "MyComponent")` extracts the exact code block with 0.00% line drift.

## 3. Outside-In Discovery Order
When solving user-reported behavior or UI issues:
1. **Domain/Permission Gate**: Check roles/permissions or policy files first (e.g., who is allowed to perform the action).
2. **Route / Page**: Check the page route component.
3. **Target Symbol**: Extract the exact component/function via `get_symbol`.

