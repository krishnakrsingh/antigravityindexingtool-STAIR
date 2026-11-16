use super::sparse_ngram::SparseNgramExtractor;
use ignore::WalkBuilder;
use memmap2::Mmap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::RwLock;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LookupEntry {
    pub hash: u64,
    pub postings_offset: u64,
    pub postings_count: u32,
    pub _reserved: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IndexMetadata {
    pub total_files: usize,
    pub total_ngrams: usize,
    pub total_postings: usize,
    pub build_duration_ms: u64,
    pub created_at: String,
}

pub struct CrepeIndex {
    workspace_root: PathBuf,
    index_dir: PathBuf,
    files: RwLock<Vec<String>>,
    file_to_id: RwLock<HashMap<String, u32>>,
    mmap_lookup: RwLock<Option<Mmap>>,
    postings_file: RwLock<Option<File>>,
    metadata: RwLock<Option<IndexMetadata>>,
    dirty_overlay: RwLock<HashSet<String>>,
}

impl CrepeIndex {
    pub fn new(workspace_root: PathBuf) -> Self {
        let index_dir = workspace_root.join(".antigravity").join("crepe");
        let index = Self {
            workspace_root,
            index_dir,
            files: RwLock::new(Vec::new()),
            file_to_id: RwLock::new(HashMap::new()),
            mmap_lookup: RwLock::new(None),
            postings_file: RwLock::new(None),
            metadata: RwLock::new(None),
            dirty_overlay: RwLock::new(HashSet::new()),
        };

        // Try loading existing index if present
        let _ = index.load_mmap();
        index
    }

    pub fn mark_dirty(&self, rel_path: &str) {
        let mut overlay = self.dirty_overlay.write().unwrap();
        overlay.insert(rel_path.replace('\\', "/"));
    }

    pub fn build(&self) -> std::io::Result<IndexMetadata> {
        let t0 = std::time::Instant::now();

        // Release existing memory map and file handles before rebuilding files on Windows
        {
            *self.mmap_lookup.write().unwrap() = None;
            *self.postings_file.write().unwrap() = None;
        }

        fs::create_dir_all(&self.index_dir)?;

        // Walk workspace collecting non-ignored text files
        let mut walker = WalkBuilder::new(&self.workspace_root);
        walker
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .filter_entry(|entry| {
                let name = entry.file_name().to_string_lossy();
                name != ".git" && name != "target" && name != "node_modules" && name != ".antigravity"
            });

        let mut file_paths = Vec::new();
        for res in walker.build() {
            if let Ok(entry) = res {
                let p = entry.path().to_path_buf();
                if p.is_file() {
                    file_paths.push(p);
                }
            }
        }

        let mut files = Vec::new();
        let mut file_to_id = HashMap::new();
        for p in &file_paths {
            if let Ok(rel) = p.strip_prefix(&self.workspace_root) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let id = files.len() as u32;
                file_to_id.insert(rel_str.clone(), id);
                files.push(rel_str);
            }
        }

        // Parallel extraction of sparse n-grams for each file
        let indexed_entries: Vec<(u32, Vec<u64>)> = files
            .par_iter()
            .enumerate()
            .filter_map(|(id, rel_path)| {
                let abs_path = self.workspace_root.join(rel_path);
                let content = fs::read(&abs_path).ok()?;
                // Skip binary files (check first 1024 bytes for null byte)
                let check_len = content.len().min(1024);
                if content[..check_len].contains(&0) {
                    return None;
                }
                let ngrams = SparseNgramExtractor::extract_all(&content);
                Some((id as u32, ngrams))
            })
            .collect();

        // Invert: n-gram hash -> list of doc IDs
        let mut inverted_map: HashMap<u64, Vec<u32>> = HashMap::new();
        for (doc_id, ngrams) in indexed_entries {
            for hash in ngrams {
                inverted_map.entry(hash).or_default().push(doc_id);
            }
        }

        // Sort hashes deterministically for binary search
        let mut sorted_hashes: Vec<u64> = inverted_map.keys().copied().collect();
        sorted_hashes.sort_unstable();

        // Write postings.bin and build lookup entries
        let postings_path = self.index_dir.join("postings.bin");
        let lookup_path = self.index_dir.join("lookup.bin");
        let files_path = self.index_dir.join("files.json");

        let mut postings_writer = BufWriter::new(File::create(&postings_path)?);
        let mut lookup_entries = Vec::with_capacity(sorted_hashes.len());
        let mut current_offset = 0u64;
        let mut total_postings = 0usize;

        for hash in &sorted_hashes {
            let doc_ids = &inverted_map[hash];
            let count = doc_ids.len() as u32;
            lookup_entries.push(LookupEntry {
                hash: *hash,
                postings_offset: current_offset,
                postings_count: count,
                _reserved: 0,
            });

            for &id in doc_ids {
                postings_writer.write_all(&id.to_le_bytes())?;
            }
            current_offset += (count as u64) * 4;
            total_postings += doc_ids.len();
        }
        postings_writer.flush()?;

        // Write lookup.bin
        let mut lookup_writer = BufWriter::new(File::create(&lookup_path)?);
        for entry in &lookup_entries {
            lookup_writer.write_all(&entry.hash.to_le_bytes())?;
            lookup_writer.write_all(&entry.postings_offset.to_le_bytes())?;
            lookup_writer.write_all(&entry.postings_count.to_le_bytes())?;
            lookup_writer.write_all(&entry._reserved.to_le_bytes())?;
        }
        lookup_writer.flush()?;

        // Write files.json
        let files_json = serde_json::to_string(&files).unwrap_or_default();
        fs::write(&files_path, files_json)?;

        let metadata = IndexMetadata {
            total_files: files.len(),
            total_ngrams: lookup_entries.len(),
            total_postings,
            build_duration_ms: t0.elapsed().as_millis() as u64,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let meta_path = self.index_dir.join("metadata.json");
        fs::write(
            &meta_path,
            serde_json::to_string_pretty(&metadata).unwrap_or_default(),
        )?;

        // Update in-memory structures
        *self.files.write().unwrap() = files;
        *self.file_to_id.write().unwrap() = file_to_id;
        *self.metadata.write().unwrap() = Some(metadata.clone());

        // Re-load mmap
        self.load_mmap()?;

        Ok(metadata)
    }

    pub fn load_mmap(&self) -> std::io::Result<()> {
        let lookup_path = self.index_dir.join("lookup.bin");
        let postings_path = self.index_dir.join("postings.bin");
        let files_path = self.index_dir.join("files.json");
        let meta_path = self.index_dir.join("metadata.json");

        if !lookup_path.exists() || !postings_path.exists() || !files_path.exists() {
            return Ok(());
        }

        let lookup_file = File::open(&lookup_path)?;
        let mmap = unsafe { Mmap::map(&lookup_file)? };
        *self.mmap_lookup.write().unwrap() = Some(mmap);

        let postings_file = File::open(&postings_path)?;
        *self.postings_file.write().unwrap() = Some(postings_file);

        let files_content = fs::read_to_string(&files_path)?;
        let files: Vec<String> = serde_json::from_str(&files_content).unwrap_or_default();
        let mut file_to_id = HashMap::new();
        for (i, f) in files.iter().enumerate() {
            file_to_id.insert(f.clone(), i as u32);
        }
        *self.files.write().unwrap() = files;
        *self.file_to_id.write().unwrap() = file_to_id;

        if let Ok(meta_content) = fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<IndexMetadata>(&meta_content) {
                *self.metadata.write().unwrap() = Some(meta);
            }
        }

        Ok(())
    }

    pub fn find_candidates(&self, query: &str) -> Vec<String> {
        let mut candidates = HashSet::new();
        let ngrams = SparseNgramExtractor::extract_query_ngrams(query);

        // Always include dirty overlay files so agent sees its writes
        {
            let overlay = self.dirty_overlay.read().unwrap();
            for f in overlay.iter() {
                candidates.insert(f.clone());
            }
        }

        if ngrams.is_empty() {
            // If query is smaller than MIN_NGRAM_LEN, fallback to all indexed files
            let files = self.files.read().unwrap();
            return files.clone();
        }

        let mmap_guard = self.mmap_lookup.read().unwrap();
        let postings_guard = self.postings_file.read().unwrap();

        if let (Some(mmap), Some(mut postings_file)) = (mmap_guard.as_ref(), postings_guard.as_ref()) {
            let entry_size = 24usize;
            let total_entries = mmap.len() / entry_size;

            let mut posting_sets: Vec<HashSet<u32>> = Vec::new();

            for &hash in &ngrams {
                // Binary search over memory-mapped lookup table in O(log N)
                let mut low = 0usize;
                let mut high = total_entries;
                let mut found_entry: Option<LookupEntry> = None;

                while low < high {
                    let mid = (low + high) / 2;
                    let offset = mid * entry_size;
                    let entry_hash = u64::from_le_bytes(
                        mmap[offset..offset + 8].try_into().unwrap_or_default(),
                    );

                    if entry_hash == hash {
                        let postings_offset = u64::from_le_bytes(
                            mmap[offset + 8..offset + 16].try_into().unwrap_or_default(),
                        );
                        let postings_count = u32::from_le_bytes(
                            mmap[offset + 16..offset + 20].try_into().unwrap_or_default(),
                        );
                        found_entry = Some(LookupEntry {
                            hash: entry_hash,
                            postings_offset,
                            postings_count,
                            _reserved: 0,
                        });
                        break;
                    } else if entry_hash < hash {
                        low = mid + 1;
                    } else {
                        high = mid;
                    }
                }

                if let Some(entry) = found_entry {
                    let mut doc_set = HashSet::new();
                    let byte_offset = entry.postings_offset;
                    let count = entry.postings_count as usize;
                    let mut buf = vec![0u8; count * 4];

                    if postings_file.seek(SeekFrom::Start(byte_offset)).is_ok()
                        && postings_file.read_exact(&mut buf).is_ok()
                    {
                        for chunk in buf.chunks_exact(4) {
                            let doc_id = u32::from_le_bytes(chunk.try_into().unwrap_or_default());
                            doc_set.insert(doc_id);
                        }
                    }
                    posting_sets.push(doc_set);
                }
            }

            if !posting_sets.is_empty() {
                // Sort sets by size ascending for fastest intersection
                posting_sets.sort_by_key(|s| s.len());
                let mut intersected = posting_sets[0].clone();
                for next_set in &posting_sets[1..] {
                    intersected.retain(|id| next_set.contains(id));
                    if intersected.is_empty() {
                        break;
                    }
                }

                let files = self.files.read().unwrap();
                for doc_id in intersected {
                    if let Some(path) = files.get(doc_id as usize) {
                        candidates.insert(path.clone());
                    }
                }
            }
        }

        candidates.into_iter().collect()
    }

    pub fn get_metadata(&self) -> Option<IndexMetadata> {
        self.metadata.read().unwrap().clone()
    }

    pub fn total_files(&self) -> usize {
        self.files.read().unwrap().len()
    }
}
