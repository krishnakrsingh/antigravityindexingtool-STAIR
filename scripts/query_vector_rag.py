#!/usr/bin/env python3
"""
scripts/query_vector_rag.py
Semantic Query CLI for Persistent Vector Database & Code RAG.
Queries 1536-dim FAISS vector index (`ast.faiss`) using `jinaai/jina-code-embeddings-1.5b`
(in 8-bit mode on local CUDA GPU or resident HTTP sidecar), retrieves enriched AST context
from SQLite (`metadata.db`), and outputs structured JSON with enriched nodes
(class, signature, decorators, docstring, preview, line ranges, byte offsets, score).
"""

import os
import sys
import json
import sqlite3
import argparse
import urllib.request
import urllib.error
from typing import List, Dict, Any, Optional

import numpy as np
import faiss

JINA_QUERY_PREFIX = "Find the most relevant code snippet given the following query:\n"

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

def get_embedding_via_sidecar(query: str, port: int = 9819) -> Optional[np.ndarray]:
    """Attempts to encode query using the resident GPU embedding server."""
    url = f"http://127.0.0.1:{port}/embed"
    formatted_query = f"{JINA_QUERY_PREFIX}{query}"
    payload = json.dumps({"texts": [formatted_query], "is_query": True}).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=payload,
        headers={"Content-Type": "application/json"}
    )
    try:
        with urllib.request.urlopen(req, timeout=1.5) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            embs = data.get("embeddings")
            if embs and len(embs) > 0:
                vec = np.array(embs[0], dtype=np.float32).reshape(1, -1)
                faiss.normalize_L2(vec)
                return vec
    except Exception:
        pass
    return None

def get_embedding_via_cuda(
    query: str,
    model_name: str = "jinaai/jina-code-embeddings-1.5b",
    load_in_8bit: bool = True
) -> np.ndarray:
    """Encodes query directly on local CUDA GPU using Jina 1.5B (1536-dim), ensuring clean stdout."""
    import contextlib
    import torch
    from sentence_transformers import SentenceTransformer

    device = "cuda" if torch.cuda.is_available() else "cpu"
    formatted_query = f"{JINA_QUERY_PREFIX}{query}"

    with contextlib.redirect_stdout(sys.stderr):
        model_kwargs = {"torch_dtype": torch.float16} if device == "cuda" else {}
        model = SentenceTransformer(
            model_name,
            device=device,
            model_kwargs=model_kwargs,
            trust_remote_code=True
        )
        vec = model.encode(
            [formatted_query],
            convert_to_numpy=True,
            normalize_embeddings=True,
            show_progress_bar=False
        )
    return vec.astype(np.float32)

def locate_vector_files(workspace: str) -> (str, str):
    """Locates ast.faiss and metadata.db in workspace or global .antigravity/vectors."""
    candidates = [
        os.path.join(workspace, ".antigravity", "vectors"),
        os.path.join(workspace, "vectors"),
        os.path.join(workspace),
        os.path.abspath(".antigravity/vectors")
    ]

    for c in candidates:
        faiss_p = os.path.join(c, "ast.faiss")
        db_p = os.path.join(c, "metadata.db")
        if os.path.isfile(faiss_p) and os.path.isfile(db_p):
            return faiss_p, db_p

    raise FileNotFoundError(
        f"Vector files (ast.faiss and metadata.db) not found in workspace '{workspace}' "
        f"or candidate paths: {candidates}. Please run scripts/index_vector_rag.py first."
    )

def query_vector_rag(
    workspace: str,
    query: str,
    top_k: int = 10,
    force_direct_gpu: bool = False,
    model_name: str = "jinaai/jina-code-embeddings-1.5b",
    load_in_8bit: bool = True
) -> List[Dict[str, Any]]:
    """Performs semantic vector search and retrieves enriched AST metadata from SQLite."""
    faiss_path, db_path = locate_vector_files(workspace)

    # 1. Load FAISS index to inspect dimensionality
    index = faiss.read_index(faiss_path)
    if index.d != 1536:
        raise ValueError(
            f"FAISS index dimension mismatch: found {index.d}, expected 1536 (jinaai/jina-code-embeddings-1.5b). "
            f"Re-run scripts/index_vector_rag.py to build the 1536-dim index."
        )

    actual_k = min(top_k, index.ntotal)
    if actual_k == 0:
        return []

    # 2. Encode query matching 1536-dim FAISS index
    query_vec = None
    if not force_direct_gpu:
        query_vec = get_embedding_via_sidecar(query)
        if query_vec is not None and query_vec.shape[1] != index.d:
            query_vec = None

    if query_vec is None:
        query_vec = get_embedding_via_cuda(query, model_name=model_name, load_in_8bit=load_in_8bit)

    # 3. Search FAISS index
    scores, indices = index.search(query_vec, actual_k)
    flat_scores = scores[0]
    flat_indices = indices[0]

    # 3. Retrieve metadata from SQLite
    conn = sqlite3.connect(db_path)
    cur = conn.cursor()

    results = []
    for score, f_id in zip(flat_scores, flat_indices):
        if f_id < 0:
            continue
        cur.execute("""
            SELECT faiss_id, node_id, file_path, symbol_name, symbol_type, class_name,
                   signature, decorators, docstring, preview, line_start, line_end,
                   byte_start, byte_end
            FROM ast_nodes WHERE faiss_id = ?
        """, (int(f_id),))
        row = cur.fetchone()
        if not row:
            continue

        raw_decs = row[7]
        try:
            decs = json.loads(raw_decs) if raw_decs else []
        except Exception:
            decs = [raw_decs] if raw_decs else []

        results.append({
            "faiss_id": row[0],
            "node_id": row[1],
            "file_path": row[2],
            "symbol_name": row[3],
            "symbol_type": row[4],
            "class": row[5] or "",
            "class_name": row[5] or "",
            "signature": row[6],
            "decorators": decs,
            "docstring": row[8] or "",
            "preview": row[9] or "",
            "line_start": row[10],
            "line_end": row[11],
            "byte_start": row[12],
            "byte_end": row[13],
            "score": round(float(score), 4)
        })

    conn.close()
    return results

def main():
    parser = argparse.ArgumentParser(description="Query persistent AST Vector RAG on local CUDA GPU.")
    parser.add_argument("--workspace", type=str, default="target_repo/plane", help="Path to codebase workspace")
    parser.add_argument("--query", type=str, required=True, help="Semantic search query")
    parser.add_argument("--top-k", type=int, default=10, help="Number of results to return (default: 10)")
    parser.add_argument("--limit", type=int, default=None, help="Alternative for --top-k")
    parser.add_argument("--force-cuda", action="store_true", help="Force direct CUDA inference bypassing sidecar")
    parser.add_argument("--model", type=str, default="jinaai/jina-code-embeddings-1.5b", help="Model name")
    parser.add_argument("--no-8bit", action="store_true", help="Disable 8-bit quantization")
    args = parser.parse_args()

    k = args.limit if args.limit is not None else args.top_k

    results = query_vector_rag(
        workspace=args.workspace,
        query=args.query,
        top_k=k,
        force_direct_gpu=args.force_cuda,
        model_name=args.model,
        load_in_8bit=not args.no_8bit
    )

    print(json.dumps(results, indent=2))

if __name__ == "__main__":
    main()
