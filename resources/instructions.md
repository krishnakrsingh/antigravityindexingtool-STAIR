This server provides sub-millisecond AST code intelligence, sparse n-gram search, and BLAKE3 verification via antigravity-fast-core.
1. Use `instant_grep` to locate keywords, symbol names, or routes across the workspace in <5ms. NEVER run PowerShell Select-String or recursive directory crawls.
2. Use `get_symbol` to extract exact AST definitions (or file outlines via mode="outline"). NEVER paginate full files with sequential read_file calls.
3. Use `verify_changes` after editing code to instantly check modified files and run compiler checks.
