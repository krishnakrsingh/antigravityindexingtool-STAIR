#!/usr/bin/env python3
"""
scripts/jina_embed_server.py
Resident CUDA GPU Embedding & Semantic RAG Server for Antigravity Fast-Core.
Model: jinaai/jina-code-embeddings-1.5b (1536-dim) in float16 on NVIDIA RTX 3050 Ti Laptop GPU.
Provides <150ms semantic search queries by keeping model and FAISS vector index resident in memory.
"""

import os
import sys
import json
import time
import sqlite3
import argparse
from http.server import HTTPServer, BaseHTTPRequestHandler
from socketserver import ThreadingMixIn
from typing import List, Dict, Any, Optional

import numpy as np
import faiss
import torch
from sentence_transformers import SentenceTransformer

JINA_QUERY_PREFIX = "Find the most relevant code snippet given the following query:\n"

def resolve_model_path(model_name: str) -> (str, bool):
    snapshot_dir = os.path.expanduser(
        r"~/.cache/huggingface/hub/models--jinaai--jina-code-embeddings-1.5b/snapshots/39aeb4fb9b60f930934c78ae5d749a46287c248a"
    )
    if "jina-code-embeddings-1.5b" in model_name and os.path.isdir(snapshot_dir):
        return snapshot_dir, True
    if os.path.isdir(model_name):
        return model_name, True
    return model_name, False

class VectorStoreManager:
    def __init__(self, vector_dir: str):
        self.vector_dir = os.path.abspath(vector_dir)
        self.faiss_path = os.path.join(self.vector_dir, "ast.faiss")
        self.db_path = os.path.join(self.vector_dir, "metadata.db")
        self.index = None
        self.conn = None
        self.last_load_time = 0
        self.load()

    def load(self):
        if os.path.isfile(self.faiss_path) and os.path.isfile(self.db_path):
            try:
                self.index = faiss.read_index(self.faiss_path)
                self.conn = sqlite3.connect(self.db_path, check_same_thread=False)
                self.last_load_time = time.time()
                print(f"[STORE] Loaded FAISS index (dim={self.index.d}, total={self.index.ntotal}) from {self.faiss_path}", file=sys.stderr)
            except Exception as e:
                print(f"[WARN] Failed loading vector store: {e}", file=sys.stderr)
        else:
            print(f"[STORE] Vector files not found in {self.vector_dir} (will load on first search)", file=sys.stderr)

    def search(self, query_vec: np.ndarray, top_k: int = 10) -> List[Dict[str, Any]]:
        if self.index is None or not os.path.isfile(self.faiss_path):
            self.load()
        if self.index is None or self.index.ntotal == 0:
            return []

        actual_k = min(top_k, self.index.ntotal)
        scores, indices = self.index.search(query_vec, actual_k)
        flat_scores = scores[0]
        flat_indices = indices[0]

        results = []
        cur = self.conn.cursor()
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
                "symbol_path": row[1],
                "file_path": row[2],
                "symbol_name": row[3],
                "name": row[3],
                "symbol_type": row[4],
                "kind": row[4].lower(),
                "class_name": row[5] if row[5] else None,
                "signature": row[6],
                "decorators": decs,
                "docstring": row[8] if row[8] else None,
                "preview": row[9],
                "line_start": row[10],
                "line_end": row[11],
                "byte_start": row[12],
                "byte_end": row[13],
                "score": float(score)
            })
        return results

class ThreadedHTTPServer(ThreadingMixIn, HTTPServer):
    daemon_threads = True

def make_handler(model, store_manager: VectorStoreManager):
    class RequestHandler(BaseHTTPRequestHandler):
        def log_message(self, format, *args):
            pass

        def do_GET(self):
            if self.path in ("/health", "/status"):
                vram_mb = torch.cuda.memory_allocated() / (1024 * 1024) if torch.cuda.is_available() else 0
                payload = {
                    "status": "ok",
                    "model": "jinaai/jina-code-embeddings-1.5b",
                    "dim": 1536,
                    "cuda": torch.cuda.is_available(),
                    "vram_allocated_mb": round(vram_mb, 2),
                    "indexed_symbols": store_manager.index.ntotal if store_manager.index else 0
                }
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps(payload).encode("utf-8"))
            else:
                self.send_response(404)
                self.end_headers()

        def do_POST(self):
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length).decode("utf-8")
            try:
                data = json.loads(body)
            except Exception as e:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(json.dumps({"error": f"Invalid JSON: {e}"}).encode("utf-8"))
                return

            if self.path == "/embed":
                texts = data.get("texts", [])
                is_query = data.get("is_query", False)
                if not texts:
                    self.send_response(400)
                    self.end_headers()
                    self.wfile.write(json.dumps({"error": "Missing 'texts' field"}).encode("utf-8"))
                    return

                if is_query:
                    texts = [f"{JINA_QUERY_PREFIX}{t}" for t in texts]

                embs = model.encode(texts, batch_size=32, convert_to_numpy=True, normalize_embeddings=True)
                resp = {"embeddings": embs.tolist()}
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps(resp).encode("utf-8"))

            elif self.path in ("/search", "/rag/search"):
                query = data.get("query", "")
                top_k = int(data.get("top_k", 10))
                workspace = data.get("workspace")
                if not query:
                    self.send_response(400)
                    self.end_headers()
                    self.wfile.write(json.dumps({"error": "Missing 'query' field"}).encode("utf-8"))
                    return

                if workspace:
                    ws_vec = os.path.join(workspace, ".antigravity", "vectors")
                    if os.path.isfile(os.path.join(ws_vec, "ast.faiss")) and ws_vec != store_manager.vector_dir:
                        store_manager.vector_dir = ws_vec
                        store_manager.faiss_path = os.path.join(ws_vec, "ast.faiss")
                        store_manager.db_path = os.path.join(ws_vec, "metadata.db")
                        store_manager.load()

                formatted_query = f"{JINA_QUERY_PREFIX}{query}"
                q_vec = model.encode([formatted_query], convert_to_numpy=True, normalize_embeddings=True).astype(np.float32)

                results = store_manager.search(q_vec, top_k=top_k)
                resp = {
                    "results": results,
                    "backend": "Resident CUDA GPU Sidecar (Jina 1.5B 1536d)"
                }
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps(resp).encode("utf-8"))
            else:
                self.send_response(404)
                self.end_headers()

    return RequestHandler

def main():
    parser = argparse.ArgumentParser(description="Resident Jina 1.5B Embedding & Semantic RAG Server.")
    parser.add_argument("--port", type=int, default=9819, help="Listening port (default: 9819)")
    parser.add_argument("--vector-dir", type=str, default=".antigravity/vectors", help="Path to vectors dir")
    parser.add_argument("--model", type=str, default="jinaai/jina-code-embeddings-1.5b", help="Model name")
    args = parser.parse_args()

    device = "cuda" if torch.cuda.is_available() else "cpu"
    resolved_path, is_local = resolve_model_path(args.model)
    print(f"[EMBED-GPU] Loading {args.model} on {device} (resolved: {resolved_path})...", file=sys.stderr)
    t0 = time.time()

    model_kwargs = {"torch_dtype": torch.float16} if device == "cuda" else {}
    model = SentenceTransformer(
        resolved_path,
        device=device,
        model_kwargs=model_kwargs,
        trust_remote_code=True
    )
    t_load = time.time() - t0
    vram_mb = torch.cuda.memory_allocated() / (1024 * 1024) if device == "cuda" else 0
    print(f"[EMBED-GPU] Loaded Jina 1.5B in {t_load:.2f}s on {device} (VRAM: {vram_mb:.1f} MB)", file=sys.stderr)

    store_manager = VectorStoreManager(args.vector_dir)

    server = ThreadedHTTPServer(("127.0.0.1", args.port), make_handler(model, store_manager))
    print(f"[EMBED SERVER] Listening on http://127.0.0.1:{args.port} (Model: Jina 1.5B 1536d, Dim: 1536)", file=sys.stderr)

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("[EMBED SERVER] Shutting down...", file=sys.stderr)
        server.server_close()

if __name__ == "__main__":
    main()
