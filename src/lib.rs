pub mod bench;
pub mod crepe;
pub mod embeddings;
pub mod installer;
pub mod mcp;
pub mod merkle;
pub mod stair;
pub mod tree;

pub use crepe::{clean_all_caches, clean_cache, get_base_cache_dir, list_caches, CrepeIndex, SearchMatch, SearchResult, SparseNgramExtractor, Verifier};
pub use embeddings::LocalSemanticIndex;
pub use installer::run_setup;
pub use mcp::{McpHandler, McpToolCallResult, McpToolInfo, WorkspacePool, WorkspaceState};
pub use merkle::{FileWatcher, MerkleDiff, MerkleNode, MerkleTree, MerkleTreeSnapshot};
pub use stair::{AstOutlineParser, CodeSymbol, SymbolEntry, SymbolKind, SymbolTrie};
pub use tree::CodebaseTree;
