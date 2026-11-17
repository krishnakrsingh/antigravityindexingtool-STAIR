use crate::crepe::cache_mgr;
use crate::crepe::{CrepeIndex, Verifier};
use crate::embeddings::LocalSemanticIndex;
use crate::merkle::FileWatcher;
use crate::stair::SymbolTrie;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

pub fn normalize_path(path: &Path) -> PathBuf {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = canonical.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        canonical
    }
}

pub struct WorkspaceState {
    pub root: PathBuf,
    #[allow(dead_code)]
    pub cache_dir: PathBuf,
    pub watcher: Arc<FileWatcher>,
    pub crepe: Arc<CrepeIndex>,
    pub verifier: Arc<Verifier>,
    pub trie: Arc<SymbolTrie>,
    pub semantic: Arc<LocalSemanticIndex>,
    pub is_indexing: Arc<AtomicBool>,
}

impl WorkspaceState {
    pub fn new(root: PathBuf) -> Self {
        let canonical = normalize_path(&root);
        let cache_dir = cache_mgr::get_workspace_cache_dir(&canonical);
        let watcher = Arc::new(FileWatcher::new(canonical.clone()));
        let crepe = Arc::new(CrepeIndex::new_with_index_dir(
            canonical.clone(),
            cache_dir.clone(),
        ));
        let verifier = Arc::new(Verifier::new(canonical.clone()));
        let trie = Arc::new(SymbolTrie::new(canonical.clone()));
        let semantic = Arc::new(LocalSemanticIndex::new());
        let is_indexing = Arc::new(AtomicBool::new(false));

        // Build AST symbol trie and local semantic index
        let _sym_count = trie.build();
        let mut items = Vec::new();
        for (file, symbols) in trie.get_outline(None) {
            for sym in symbols {
                let sym_path = format!("{}::{}", file, sym.name);
                items.push((sym_path, sym.signature, file.clone()));
            }
        }
        semantic.index_symbols_batch(items);

        let has_mmap = crepe.total_files() > 0;

        let ws = Self {
            root: canonical.clone(),
            cache_dir,
            watcher,
            crepe: crepe.clone(),
            verifier,
            trie: trie.clone(),
            semantic: semantic.clone(),
            is_indexing: is_indexing.clone(),
        };

        if !has_mmap {
            is_indexing.store(true, Ordering::SeqCst);
            let crepe_bg = crepe.clone();
            let indexing_flag = is_indexing.clone();

            std::thread::spawn(move || {
                let _ = crepe_bg.build();
                indexing_flag.store(false, Ordering::SeqCst);
            });
        }

        ws
    }

    /// Synchronously waits for indexing to complete if needed (e.g. for exact tests)
    #[allow(dead_code)]
    pub fn wait_until_indexed(&self) {
        while self.is_indexing.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

pub struct WorkspacePool {
    workspaces: RwLock<HashMap<PathBuf, Arc<WorkspaceState>>>,
    default_root: RwLock<PathBuf>,
}

impl WorkspacePool {
    pub fn new(initial_root: PathBuf) -> Self {
        let canonical = normalize_path(&initial_root);
        let ws = Arc::new(WorkspaceState::new(canonical.clone()));
        let mut map = HashMap::new();
        map.insert(canonical.clone(), ws);

        Self {
            workspaces: RwLock::new(map),
            default_root: RwLock::new(canonical),
        }
    }

    pub fn set_default(&self, root: PathBuf) {
        let canonical = normalize_path(&root);
        self.get_or_create(&canonical);
        let mut def = self.default_root.write().unwrap();
        *def = canonical;
    }

    pub fn get_default(&self) -> Arc<WorkspaceState> {
        let def_root = self.default_root.read().unwrap().clone();
        self.get_or_create(&def_root)
    }

    pub fn get_or_create(&self, root: &Path) -> Arc<WorkspaceState> {
        let canonical = normalize_path(root);
        {
            let map = self.workspaces.read().unwrap();
            if let Some(ws) = map.get(&canonical) {
                return ws.clone();
            }
        }

        let new_ws = Arc::new(WorkspaceState::new(canonical.clone()));
        let mut map = self.workspaces.write().unwrap();
        map.entry(canonical).or_insert(new_ws).clone()
    }

    /// Intelligently resolves the target workspace from optional hints:
    /// 1. Explicit workspace parameter (e.g. from tool call)
    /// 2. Absolute file path parameter matching a known workspace or git root
    /// 3. Default active workspace
    pub fn resolve(
        &self,
        workspace_hint: Option<&str>,
        path_hint: Option<&str>,
    ) -> Arc<WorkspaceState> {
        if let Some(ws_str) = workspace_hint {
            let trimmed = ws_str.trim();
            if !trimmed.is_empty() {
                let p = PathBuf::from(trimmed);
                return self.get_or_create(&p);
            }
        }

        if let Some(path_str) = path_hint {
            let p = normalize_path(Path::new(path_str));
            if p.is_absolute() {
                // Check if already matches an active workspace prefix
                {
                    let map = self.workspaces.read().unwrap();
                    for (ws_path, ws_state) in map.iter() {
                        if p.starts_with(ws_path) {
                            return ws_state.clone();
                        }
                    }
                }

                // Check ancestors for .git or existing directory
                let mut current = p.parent();
                while let Some(parent) = current {
                    if parent.join(".git").exists() {
                        return self.get_or_create(parent);
                    }
                    current = parent.parent();
                }
            }
        }

        self.get_default()
    }

    pub fn list_active(&self) -> Vec<(PathBuf, bool, usize)> {
        let map = self.workspaces.read().unwrap();
        map.iter()
            .map(|(path, state)| {
                (
                    path.clone(),
                    state.is_indexing.load(Ordering::SeqCst),
                    state.crepe.total_files(),
                )
            })
            .collect()
    }
}
