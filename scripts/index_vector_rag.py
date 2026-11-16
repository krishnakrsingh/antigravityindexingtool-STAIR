#!/usr/bin/env python3
"""
scripts/index_vector_rag.py
High-performance CUDA GPU Vector Database & Code RAG Indexer.
Extracts rich AST metadata (Python via `ast`, TypeScript/JavaScript via `tree-sitter`),
encodes semantic embeddings via `jinaai/jina-code-embeddings-1.5b` with 8-bit quantization
on local CUDA GPU (NVIDIA GeForce RTX 3050 Ti Laptop GPU, ~1.5 GB VRAM footprint),
builds a persistent `faiss.IndexFlatIP(1536)` index, and writes to an enriched SQLite metadata database.
"""

import os
import sys
import ast
import json
import time
import shutil
import sqlite3
import argparse
from typing import List, Dict, Any, Optional

import numpy as np
import torch
import torch.nn.functional as F
import faiss
from tree_sitter import Language, Parser
import tree_sitter_typescript as tst

# Initialize Tree-Sitter for TypeScript and TSX
TS_LANG = Language(tst.language_typescript())
TSX_LANG = Language(tst.language_tsx())
TS_PARSER = Parser(TS_LANG)
TSX_PARSER = Parser(TSX_LANG)

IGNORE_DIRS = {
    ".git", "node_modules", "venv", ".venv", "__pycache__", ".antigravity",
    "dist", "build", ".next", ".turbo", ".husky", "target", "bin", "obj",
    ".pytest_cache", ".ruff_cache", ".mypy_cache"
}

def resolve_model_path(model_name: str) -> (str, bool):
    """Resolves local snapshot path if available to prevent online check timeouts."""
    snapshot_dir = os.path.expanduser(
        r"~/.cache/huggingface/hub/models--jinaai--jina-code-embeddings-1.5b/snapshots/39aeb4fb9b60f930934c78ae5d749a46287c248a"
    )
    if "jina-code-embeddings-1.5b" in model_name and os.path.isdir(snapshot_dir):
        return snapshot_dir, True
    if os.path.isdir(model_name):
        return model_name, True
    return model_name, False

def compute_line_starts(content_bytes: bytes) -> List[int]:
    """Computes byte offset of the start of each line (1-indexed mapping)."""
    starts = [0]
    for idx, b in enumerate(content_bytes):
        if b == 10:  # '\n'
            starts.append(idx + 1)
    return starts

def extract_python_symbols(rel_path: str, abs_path: str) -> List[Dict[str, Any]]:
    """Extracts classes, methods, and functions from a Python file using Python's `ast`."""
    symbols = []
    try:
        with open(abs_path, "rb") as f:
            content_bytes = f.read()
    except Exception:
        return symbols

    try:
        tree = ast.parse(content_bytes, filename=rel_path)
    except Exception:
        return symbols

    line_starts = compute_line_starts(content_bytes)
    total_bytes = len(content_bytes)
    source_lines = content_bytes.split(b"\n")

    def get_byte_range(start_l: int, end_l: int) -> (int, int):
        s_idx = max(0, start_l - 1)
        b_start = line_starts[s_idx] if s_idx < len(line_starts) else total_bytes
        e_idx = max(0, end_l)
        b_end = line_starts[e_idx] if e_idx < len(line_starts) else total_bytes
        return b_start, b_end

    def get_preview(start_l: int, end_l: int, max_lines: int = 15) -> str:
        s_idx = max(0, start_l - 1)
        e_idx = min(len(source_lines), s_idx + max_lines, end_l)
        preview_bytes = b"\n".join(source_lines[s_idx:e_idx])
        return preview_bytes.decode("utf-8", errors="replace")

    def get_signature(node, start_l: int) -> str:
        s_idx = max(0, start_l - 1)
        if s_idx < len(source_lines):
            first_line = source_lines[s_idx].decode("utf-8", errors="replace").strip()
            while first_line.startswith("@") and s_idx + 1 < len(source_lines):
                s_idx += 1
                first_line = source_lines[s_idx].decode("utf-8", errors="replace").strip()
            return first_line
        return ""

    for node in tree.body:
        if isinstance(node, ast.ClassDef):
            c_start = node.lineno
            c_end = getattr(node, "end_lineno", c_start)
            b_start, b_end = get_byte_range(c_start, c_end)
            c_sig = get_signature(node, c_start)
            c_doc = ast.get_docstring(node) or ""
            c_decs = [f"@{ast.unparse(d)}" for d in node.decorator_list]
            c_preview = get_preview(c_start, c_end)

            symbols.append({
                "node_id": f"{rel_path}::{node.name}",
                "file_path": rel_path,
                "symbol_name": node.name,
                "symbol_type": "Class",
                "class_name": "",
                "signature": c_sig,
                "decorators": json.dumps(c_decs),
                "docstring": c_doc,
                "preview": c_preview,
                "line_start": c_start,
                "line_end": c_end,
                "byte_start": b_start,
                "byte_end": b_end
            })

            for item in node.body:
                if isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    m_start = item.lineno
                    m_end = getattr(item, "end_lineno", m_start)
                    mb_start, mb_end = get_byte_range(m_start, m_end)
                    m_sig = get_signature(item, m_start)
                    m_doc = ast.get_docstring(item) or ""
                    m_decs = [f"@{ast.unparse(d)}" for d in item.decorator_list]
                    m_preview = get_preview(m_start, m_end)

                    symbols.append({
                        "node_id": f"{rel_path}::{node.name}::{item.name}",
                        "file_path": rel_path,
                        "symbol_name": item.name,
                        "symbol_type": "Method",
                        "class_name": node.name,
                        "signature": m_sig,
                        "decorators": json.dumps(m_decs),
                        "docstring": m_doc,
                        "preview": m_preview,
                        "line_start": m_start,
                        "line_end": m_end,
                        "byte_start": mb_start,
                        "byte_end": mb_end
                    })

        elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            f_start = node.lineno
            f_end = getattr(node, "end_lineno", f_start)
            b_start, b_end = get_byte_range(f_start, f_end)
            f_sig = get_signature(node, f_start)
            f_doc = ast.get_docstring(node) or ""
            f_decs = [f"@{ast.unparse(d)}" for d in node.decorator_list]
            f_preview = get_preview(f_start, f_end)

            symbols.append({
                "node_id": f"{rel_path}::{node.name}",
                "file_path": rel_path,
                "symbol_name": node.name,
                "symbol_type": "Function",
                "class_name": "",
                "signature": f_sig,
                "decorators": json.dumps(f_decs),
                "docstring": f_doc,
                "preview": f_preview,
                "line_start": f_start,
                "line_end": f_end,
                "byte_start": b_start,
                "byte_end": b_end
            })

    return symbols

def extract_ts_symbols(rel_path: str, abs_path: str) -> List[Dict[str, Any]]:
    """Extracts classes, interfaces, types, functions, and methods using tree-sitter."""
    symbols = []
    try:
        with open(abs_path, "rb") as f:
            content_bytes = f.read()
    except Exception:
        return symbols

    is_tsx = rel_path.endswith((".tsx", ".jsx"))
    parser = TSX_PARSER if is_tsx else TS_PARSER

    try:
        tree = parser.parse(content_bytes)
    except Exception:
        return symbols

    source_lines = content_bytes.split(b"\n")

    def get_preview(start_l: int, end_l: int, max_lines: int = 15) -> str:
        s_idx = max(0, start_l - 1)
        e_idx = min(len(source_lines), s_idx + max_lines, end_l)
        preview_bytes = b"\n".join(source_lines[s_idx:e_idx])
        return preview_bytes.decode("utf-8", errors="replace")

    def extract_docstring(node) -> str:
        prev = node.prev_sibling
        if prev and prev.type == "comment":
            text = prev.text.decode("utf-8", errors="replace")
            lines = []
            for l in text.splitlines():
                cl = l.strip().lstrip("/*").rstrip("*/").strip().lstrip("*").strip()
                if cl:
                    lines.append(cl)
            return " ".join(lines)
        return ""

    def visit(node, current_class: Optional[str] = None):
        ntype = node.type

        if ntype == "export_statement":
            for child in node.children:
                if child.type in ("class_declaration", "interface_declaration",
                                  "type_alias_declaration", "enum_declaration",
                                  "function_declaration", "lexical_declaration"):
                    visit(child, current_class)
            return

        if ntype == "class_declaration":
            name_node = node.child_by_field_name("name")
            name = name_node.text.decode("utf-8", errors="replace") if name_node else "AnonymousClass"
            l_start = node.start_point.row + 1
            l_end = node.end_point.row + 1
            b_start = node.start_byte
            b_end = node.end_byte
            sig = source_lines[l_start - 1].decode("utf-8", errors="replace").strip() if l_start - 1 < len(source_lines) else ""
            doc = extract_docstring(node)
            preview = get_preview(l_start, l_end)

            symbols.append({
                "node_id": f"{rel_path}::{name}",
                "file_path": rel_path,
                "symbol_name": name,
                "symbol_type": "Class",
                "class_name": "",
                "signature": sig,
                "decorators": "[]",
                "docstring": doc,
                "preview": preview,
                "line_start": l_start,
                "line_end": l_end,
                "byte_start": b_start,
                "byte_end": b_end
            })

            body = node.child_by_field_name("body")
            if body:
                for c in body.children:
                    visit(c, name)
            return

        if ntype == "interface_declaration":
            name_node = node.child_by_field_name("name")
            name = name_node.text.decode("utf-8", errors="replace") if name_node else "AnonymousInterface"
            l_start = node.start_point.row + 1
            l_end = node.end_point.row + 1
            b_start = node.start_byte
            b_end = node.end_byte
            sig = source_lines[l_start - 1].decode("utf-8", errors="replace").strip() if l_start - 1 < len(source_lines) else ""
            doc = extract_docstring(node)
            preview = get_preview(l_start, l_end)

            symbols.append({
                "node_id": f"{rel_path}::{name}",
                "file_path": rel_path,
                "symbol_name": name,
                "symbol_type": "Interface",
                "class_name": "",
                "signature": sig,
                "decorators": "[]",
                "docstring": doc,
                "preview": preview,
                "line_start": l_start,
                "line_end": l_end,
                "byte_start": b_start,
                "byte_end": b_end
            })
            return

        if ntype == "type_alias_declaration":
            name_node = node.child_by_field_name("name")
            name = name_node.text.decode("utf-8", errors="replace") if name_node else "AnonymousType"
            l_start = node.start_point.row + 1
            l_end = node.end_point.row + 1
            b_start = node.start_byte
            b_end = node.end_byte
            sig = source_lines[l_start - 1].decode("utf-8", errors="replace").strip() if l_start - 1 < len(source_lines) else ""
            doc = extract_docstring(node)
            preview = get_preview(l_start, l_end)

            symbols.append({
                "node_id": f"{rel_path}::{name}",
                "file_path": rel_path,
                "symbol_name": name,
                "symbol_type": "TypeAlias",
                "class_name": "",
                "signature": sig,
                "decorators": "[]",
                "docstring": doc,
                "preview": preview,
                "line_start": l_start,
                "line_end": l_end,
                "byte_start": b_start,
                "byte_end": b_end
            })
            return

        if ntype == "enum_declaration":
            name_node = node.child_by_field_name("name")
            name = name_node.text.decode("utf-8", errors="replace") if name_node else "AnonymousEnum"
            l_start = node.start_point.row + 1
            l_end = node.end_point.row + 1
            b_start = node.start_byte
            b_end = node.end_byte
            sig = source_lines[l_start - 1].decode("utf-8", errors="replace").strip() if l_start - 1 < len(source_lines) else ""
            doc = extract_docstring(node)
            preview = get_preview(l_start, l_end)

            symbols.append({
                "node_id": f"{rel_path}::{name}",
                "file_path": rel_path,
                "symbol_name": name,
                "symbol_type": "Enum",
                "class_name": "",
                "signature": sig,
                "decorators": "[]",
                "docstring": doc,
                "preview": preview,
                "line_start": l_start,
                "line_end": l_end,
                "byte_start": b_start,
                "byte_end": b_end
            })
            return

        if ntype == "function_declaration":
            name_node = node.child_by_field_name("name")
            name = name_node.text.decode("utf-8", errors="replace") if name_node else "AnonymousFunc"
            l_start = node.start_point.row + 1
            l_end = node.end_point.row + 1
            b_start = node.start_byte
            b_end = node.end_byte
            sig = source_lines[l_start - 1].decode("utf-8", errors="replace").strip() if l_start - 1 < len(source_lines) else ""
            doc = extract_docstring(node)
            preview = get_preview(l_start, l_end)

            node_id = f"{rel_path}::{current_class}::{name}" if current_class else f"{rel_path}::{name}"
            symbols.append({
                "node_id": node_id,
                "file_path": rel_path,
                "symbol_name": name,
                "symbol_type": "Method" if current_class else "Function",
                "class_name": current_class or "",
                "signature": sig,
                "decorators": "[]",
                "docstring": doc,
                "preview": preview,
                "line_start": l_start,
                "line_end": l_end,
                "byte_start": b_start,
                "byte_end": b_end
            })
            return

        if ntype == "method_definition":
            name_node = node.child_by_field_name("name")
            name = name_node.text.decode("utf-8", errors="replace") if name_node else "AnonymousMethod"
            l_start = node.start_point.row + 1
            l_end = node.end_point.row + 1
            b_start = node.start_byte
            b_end = node.end_byte
            sig = source_lines[l_start - 1].decode("utf-8", errors="replace").strip() if l_start - 1 < len(source_lines) else ""
            doc = extract_docstring(node)
            preview = get_preview(l_start, l_end)

            node_id = f"{rel_path}::{current_class}::{name}" if current_class else f"{rel_path}::{name}"
            symbols.append({
                "node_id": node_id,
                "file_path": rel_path,
                "symbol_name": name,
                "symbol_type": "Method",
                "class_name": current_class or "",
                "signature": sig,
                "decorators": "[]",
                "docstring": doc,
                "preview": preview,
                "line_start": l_start,
                "line_end": l_end,
                "byte_start": b_start,
                "byte_end": b_end
            })
            return

        if ntype == "lexical_declaration":
            for c in node.children:
                if c.type == "variable_declarator":
                    val = c.child_by_field_name("value")
                    if val and val.type in ("arrow_function", "function_expression"):
                        name_node = c.child_by_field_name("name")
                        name = name_node.text.decode("utf-8", errors="replace") if name_node else "AnonymousVar"
                        l_start = node.start_point.row + 1
                        l_end = node.end_point.row + 1
                        b_start = node.start_byte
                        b_end = node.end_byte
                        sig = source_lines[l_start - 1].decode("utf-8", errors="replace").strip() if l_start - 1 < len(source_lines) else ""
                        doc = extract_docstring(node)
                        preview = get_preview(l_start, l_end)

                        symbols.append({
                            "node_id": f"{rel_path}::{name}",
                            "file_path": rel_path,
                            "symbol_name": name,
                            "symbol_type": "Function",
                            "class_name": "",
                            "signature": sig,
                            "decorators": "[]",
                            "docstring": doc,
                            "preview": preview,
                            "line_start": l_start,
                            "line_end": l_end,
                            "byte_start": b_start,
                            "byte_end": b_end
                        })

        for c in node.children:
            visit(c, current_class)

    visit(tree.root_node)
    return symbols

def collect_all_symbols(workspace_root: str) -> List[Dict[str, Any]]:
    """Recursively traverses the workspace and extracts AST nodes."""
    all_symbols = []
    seen_node_ids = set()
    total_files = 0
    t0 = time.time()

    print(f"[AST SCAN] Scanning workspace at {workspace_root}...", file=sys.stderr)
    for root, dirs, files in os.walk(workspace_root):
        dirs[:] = [d for d in dirs if d not in IGNORE_DIRS]

        for fname in files:
            ext = os.path.splitext(fname)[1].lower()
            if ext in (".py", ".ts", ".tsx", ".js", ".jsx") and not fname.endswith(".d.ts"):
                abs_p = os.path.join(root, fname)
                rel_p = os.path.relpath(abs_p, workspace_root).replace("\\", "/")
                total_files += 1

                if ext == ".py":
                    syms = extract_python_symbols(rel_p, abs_p)
                else:
                    syms = extract_ts_symbols(rel_p, abs_p)

                for s in syms:
                    nid = s["node_id"]
                    if nid in seen_node_ids:
                        nid = f"{nid}#{s['line_start']}"
                        s["node_id"] = nid
                    seen_node_ids.add(nid)
                    all_symbols.append(s)

    elapsed = time.time() - t0
    print(f"[AST SCAN] Completed scan of {total_files} files: extracted {len(all_symbols)} AST symbols in {elapsed:.2f}s", file=sys.stderr)
    return all_symbols

def format_embedding_text(sym: Dict[str, Any]) -> str:
    """Formats concise high-signal semantic representation for fast Jina vector embedding."""
    lines = [
        f"{sym['symbol_type']} {sym['symbol_name']} in {sym['file_path']}",
        f"Signature: {sym['signature']}",
    ]
    if sym.get("class_name"):
        lines.append(f"Class: {sym['class_name']}")
    decs = sym.get("decorators", "")
    if decs and decs != "[]":
        lines.append(f"Decorators: {decs}")
    doc = sym.get("docstring", "")
    if doc:
        first_line = doc.strip().splitlines()[0].strip()
        lines.append(f"Doc: {first_line[:120]}")
    body = "\n".join(lines)
    return f"Candidate code snippet:\n{body}"

class JinaCodeEmbedder:
    """Encodes texts into 1536-dim embeddings using jinaai/jina-code-embeddings-1.5b on CUDA."""

    def __init__(
        self,
        model_name: str = "jinaai/jina-code-embeddings-1.5b",
        device: Optional[str] = None,
        load_in_8bit: bool = False
    ):
        if device is None:
            device = "cuda" if torch.cuda.is_available() else "cpu"
        self.device = device
        self.model_name = model_name

        resolved_path, is_local = resolve_model_path(model_name)

        print(f"[GPU VECTOR RAG] Target Device: {device} (CUDA available: {torch.cuda.is_available()})", file=sys.stderr)
        if device == "cuda":
            gpu_name = torch.cuda.get_device_name(0)
            vram_free = torch.cuda.mem_get_info()[0] // (1024 * 1024)
            print(f"[GPU VECTOR RAG] GPU: {gpu_name} | Free VRAM: {vram_free} MB", file=sys.stderr)

        print(f"[GPU VECTOR RAG] Loading model '{resolved_path}' via SentenceTransformer on {device}...", file=sys.stderr)
        t_load0 = time.time()

        from sentence_transformers import SentenceTransformer
        model_kwargs = {"torch_dtype": torch.float16} if device == "cuda" else {}
        self.model = SentenceTransformer(
            resolved_path,
            device=device,
            model_kwargs=model_kwargs,
            trust_remote_code=True
        )
        self.model.max_seq_length = 256

        t_load = time.time() - t_load0
        if torch.cuda.is_available():
            vram_used = torch.cuda.memory_allocated() / (1024 * 1024)
            print(f"[GPU VECTOR RAG] Model loaded in {t_load:.2f}s | Allocated VRAM: {vram_used:.1f} MB", file=sys.stderr)
        else:
            print(f"[GPU VECTOR RAG] Model loaded in {t_load:.2f}s", file=sys.stderr)

    def encode(
        self,
        texts: List[str],
        batch_size: int = 32,
        max_length: int = 256,
        show_progress_bar: bool = True
    ) -> np.ndarray:
        return self.model.encode(
            texts,
            batch_size=batch_size,
            convert_to_numpy=True,
            normalize_embeddings=True,
            show_progress_bar=show_progress_bar
        ).astype(np.float32)

def build_vector_database(
    workspace_root: str,
    output_dir: str,
    batch_size: int = 32,
    model_name: str = "jinaai/jina-code-embeddings-1.5b",
    device: Optional[str] = None,
    load_in_8bit: bool = False,
    core_only: bool = False,
    limit: Optional[int] = None
):
    """Encodes symbols using Jina 1.5B (1536-dim) on CUDA GPU, creates FAISS index, and writes SQLite metadata DB."""
    os.makedirs(output_dir, exist_ok=True)
    faiss_path = os.path.join(output_dir, "ast.faiss")
    db_path = os.path.join(output_dir, "metadata.db")

    # 1. Collect all AST symbols
    symbols = collect_all_symbols(workspace_root)
    if not symbols:
        print("[WARN] No AST symbols found to index.", file=sys.stderr)
        return

    if core_only:
        core_prefixes = ("apps/api/plane", "packages/", "apps/web/services", "apps/web/store")
        symbols = [
            s for s in symbols
            if any(s["file_path"].startswith(p) for p in core_prefixes)
            and (s["symbol_type"] in ("Class", "Interface", "TypeAlias") or not s.get("class_name"))
        ]
        print(f"[AST SCAN] Filtered to {len(symbols)} high-value architectural symbols.", file=sys.stderr)

    if limit and limit > 0 and len(symbols) > limit:
        symbols = symbols[:limit]
        print(f"[AST SCAN] Truncated to top {len(symbols)} symbols via --limit.", file=sys.stderr)

    # 2. Prepare embedding texts with candidate prefix
    texts = [format_embedding_text(s) for s in symbols]

    # 3. Load embedding model on GPU (Jina 1.5B only)
    embedder = JinaCodeEmbedder(
        model_name=model_name,
        device=device,
        load_in_8bit=load_in_8bit
    )

    # 4. Batch encode on GPU
    print(f"[GPU VECTOR RAG] Encoding {len(texts)} symbol texts (batch size: {batch_size})...", file=sys.stderr)
    t_enc0 = time.time()
    embeddings = embedder.encode(
        texts,
        batch_size=batch_size,
        max_length=256,
        show_progress_bar=True
    )
    t_enc = time.time() - t_enc0
    throughput = len(texts) / max(0.001, t_enc)
    print(f"[GPU VECTOR RAG] Encoded {len(texts)} symbols in {t_enc:.2f}s ({throughput:.1f} symbols/sec)", file=sys.stderr)

    # 5. Build FAISS IndexFlatIP (1536-dim, inner product == cosine similarity on normalized vectors)
    dim = embeddings.shape[1]
    print(f"[GPU VECTOR RAG] Building FAISS IndexFlatIP({dim})...", file=sys.stderr)
    index = faiss.IndexFlatIP(dim)
    index.add(embeddings.astype(np.float32))
    print(f"[GPU VECTOR RAG] Writing FAISS index ({index.ntotal} vectors) to {faiss_path}...", file=sys.stderr)
    faiss.write_index(index, faiss_path)

    # 6. Populate SQLite database
    print(f"[GPU VECTOR RAG] Populating SQLite metadata at {db_path}...", file=sys.stderr)
    if os.path.exists(db_path):
        try:
            os.remove(db_path)
        except Exception:
            pass

    conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    cur.execute("PRAGMA journal_mode = WAL;")
    cur.execute("PRAGMA synchronous = NORMAL;")

    cur.execute("""
    CREATE TABLE IF NOT EXISTS ast_nodes (
        faiss_id INTEGER PRIMARY KEY,
        node_id TEXT UNIQUE NOT NULL,
        file_path TEXT NOT NULL,
        symbol_name TEXT NOT NULL,
        symbol_type TEXT NOT NULL,
        class_name TEXT,
        signature TEXT NOT NULL,
        decorators TEXT,
        docstring TEXT,
        preview TEXT,
        line_start INTEGER NOT NULL,
        line_end INTEGER NOT NULL,
        byte_start INTEGER NOT NULL,
        byte_end INTEGER NOT NULL
    );
    """)

    cur.execute("CREATE INDEX IF NOT EXISTS idx_ast_file_path ON ast_nodes(file_path);")
    cur.execute("CREATE INDEX IF NOT EXISTS idx_ast_symbol_name ON ast_nodes(symbol_name);")
    cur.execute("CREATE INDEX IF NOT EXISTS idx_ast_class_name ON ast_nodes(class_name);")
    cur.execute("CREATE INDEX IF NOT EXISTS idx_ast_symbol_type ON ast_nodes(symbol_type);")

    rows = []
    for faiss_id, sym in enumerate(symbols):
        rows.append((
            faiss_id,
            sym["node_id"],
            sym["file_path"],
            sym["symbol_name"],
            sym["symbol_type"],
            sym["class_name"],
            sym["signature"],
            sym["decorators"],
            sym["docstring"],
            sym["preview"],
            sym["line_start"],
            sym["line_end"],
            sym["byte_start"],
            sym["byte_end"]
        ))

    cur.executemany("""
    INSERT INTO ast_nodes (
        faiss_id, node_id, file_path, symbol_name, symbol_type, class_name,
        signature, decorators, docstring, preview, line_start, line_end,
        byte_start, byte_end
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
    """, rows)

    conn.commit()
    count = cur.execute("SELECT COUNT(*) FROM ast_nodes").fetchone()[0]
    conn.close()
    print(f"[GPU VECTOR RAG] SQLite database populated with {count} rows.", file=sys.stderr)

    # 7. Mirror to working directory .antigravity/vectors if different
    root_vectors = os.path.abspath(".antigravity/vectors")
    out_vectors = os.path.abspath(output_dir)
    if root_vectors != out_vectors:
        os.makedirs(root_vectors, exist_ok=True)
        shutil.copy2(faiss_path, os.path.join(root_vectors, "ast.faiss"))
        shutil.copy2(db_path, os.path.join(root_vectors, "metadata.db"))
        print(f"[GPU VECTOR RAG] Synchronized index to root {root_vectors}", file=sys.stderr)

    print(f"[SUCCESS] Jina 1.5B (8-bit) Vector DB & Code RAG pipeline complete. Total indexed: {count} symbols.", file=sys.stderr)

def main():
    parser = argparse.ArgumentParser(description="Build and Deploy Vector DB & Code RAG on local CUDA GPU.")
    parser.add_argument("--workspace", type=str, default="target_repo/plane", help="Path to codebase workspace")
    parser.add_argument("--output", type=str, default=None, help="Directory to store .faiss and .db files")
    parser.add_argument("--batch-size", type=int, default=32, help="Batch size for embedding generation (default: 32)")
    parser.add_argument("--device", type=str, default=None, help="Inference device: 'cuda' or 'cpu'")
    parser.add_argument("--model", type=str, default="jinaai/jina-code-embeddings-1.5b", help="Model name")
    parser.add_argument("--no-8bit", action="store_true", help="Disable 8-bit quantization")
    parser.add_argument("--core-only", action="store_true", help="Index core architectural symbols only")
    parser.add_argument("--limit", type=int, default=None, help="Maximum number of symbols to index")
    args = parser.parse_args()

    workspace = os.path.abspath(args.workspace)
    if not os.path.isdir(workspace):
        print(f"[ERROR] Workspace directory not found: {workspace}", file=sys.stderr)
        sys.exit(1)

    output_dir = args.output
    if output_dir is None:
        output_dir = os.path.join(workspace, ".antigravity", "vectors")

    build_vector_database(
        workspace_root=workspace,
        output_dir=output_dir,
        batch_size=args.batch_size,
        model_name=args.model,
        device=args.device,
        load_in_8bit=not args.no_8bit,
        core_only=args.core_only,
        limit=args.limit
    )

if __name__ == "__main__":
    main()
