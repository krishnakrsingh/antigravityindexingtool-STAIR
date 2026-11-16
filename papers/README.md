# Code Intelligence & Fast Retrieval Research Compendium

This folder contains the authoritative research papers, engineering disclosures, and reference benchmarks assembled to architect a high-speed code retrieval and intelligence engine for Antigravity.

---

## 1. Primary Research Papers & Disclosures

### 1.1 STAIR: STructure Aware Information Retriever
- **File**: [`STAIR_IBM_Research_arXiv_2609.03874.pdf`](file:///c:/Users/krish/Downloads/antigravitytoolbuild/papers/STAIR_IBM_Research_arXiv_2609.03874.pdf)
- **HTML**: [`STAIR_IBM_Research_arXiv_2609.03874.html`](file:///c:/Users/krish/Downloads/antigravitytoolbuild/papers/STAIR_IBM_Research_arXiv_2609.03874.html)
- **Authors**: Vineet Kumar, Meghanadh Pulivarthi, Vishwajeet Kumar, Jaydeep Sen, Riyaz Ahmad Bhat, Sachindra Joshi (IBM Research, September 2026)
- **Core Concept**:
  - Replaces traditional chunk-and-embed RAG (which suffers from arbitrary boundary splitting and the "lost in the middle" problem) with Generative Information Retrieval (GIR / Differentiable Search Index).
  - Uses the **Table of Contents (ToC) and global document hierarchy** as the primary addressing space.
  - Guarantees **<0.05% hallucination rate** by enforcing constrained trie decoding over valid section paths.
  - Achieves **82.6% Recall@1** on the SearchTome benchmark (vs DSI 76.9%, DPR 68.7%, BM25 59.5%).

---

### 1.2 Cursor Fast Regex Search: Indexing Text for Agent Tools
- **File**: [`cursor_fast_regex_search.html`](file:///c:/Users/krish/Downloads/antigravitytoolbuild/papers/cursor_fast_regex_search.html)
- **Date**: March 23, 2026
- **Core Concept**:
  - AI agents heavily rely on regex searches (grep), but unindexed scanners (ripgrep) scale linearly with codebase size (15+ seconds per search on monorepos).
  - Implements **sparse n-grams** with character-pair frequency weighting derived from a 2TB open-source corpus.
  - Two-stage architecture:
    1. Memory-mapped (`mmap`) lookup table binary searches n-gram hashes and reads posting lists to identify candidate files (<5ms).
    2. Exact regex verification executes only against candidate files.
  - Stored locally in `.git/cursor/crepe` with an in-memory dirty overlay on top of Git commits.

---

### 1.3 Cursor Secure Codebase Indexing (Older Semantic Architecture)
- **File**: [`cursor_secure_codebase_indexing.html`](file:///c:/Users/krish/Downloads/antigravitytoolbuild/papers/cursor_secure_codebase_indexing.html)
- **Core Concept**:
  - Merkle tree filesystem synchronization: Computes SHA-256 hash DAGs of workspaces to sync only modified files to indexing services.
  - Syntactic AST chunking + Turbopuffer vector database for embedding search.
  - Why it was sunset for agents in 2026: Remote embedding sync had high latency, drift against dirty working trees, and semantic vector similarity was less reliable for agents than exact fast grep.

---

### 1.4 Cursor: Continually Improving Our Agent Harness
- **File**: [`cursor_continually_improving_agent_harness.html`](file:///c:/Users/krish/Downloads/antigravitytoolbuild/papers/cursor_continually_improving_agent_harness.html)
- **Core Concept**:
  - Dynamic context discovery: Minimal initial prompt (OS, git status, active files); agent retrieves context on demand.
  - Long tool outputs written to temporary files; agent inspects via range/tail.
  - Retrievable conversation history and dynamic MCP tool definition loading (46.9% token reduction).
  - Model-specific adapters (patches for OpenAI, string replacement for Anthropic).

---

### 1.5 Cursor: What We've Learned Building Cloud Agents
- **File**: [`cursor_cloud_agent_lessons.html`](file:///c:/Users/krish/Downloads/antigravitytoolbuild/papers/cursor_cloud_agent_lessons.html)
- **Core Concept**:
  - Distributed execution infrastructure using Temporal workflows (>50M actions/day).
  - Isolated VM environments, hibernation, resumption, snapshotting, and environment forking.
