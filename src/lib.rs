pub mod bench;
pub mod crepe;
pub mod embeddings;
pub mod mcp;
pub mod merkle;
pub mod stair;

pub use crepe::{CrepeIndex, SearchMatch, SearchResult, SparseNgramExtractor, Verifier};
pub use embeddings::LocalSemanticIndex;
pub use mcp::{McpHandler, McpToolCallResult, McpToolInfo};
pub use merkle::{FileWatcher, MerkleDiff, MerkleNode, MerkleTree, MerkleTreeSnapshot};
pub use stair::{AstOutlineParser, CodeSymbol, SymbolEntry, SymbolKind, SymbolTrie};
