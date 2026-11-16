mod bench;
mod crepe;
mod embeddings;
mod mcp;
mod merkle;
mod stair;

use bench::{BenchmarkRunner, Scalability50kRunner};
use clap::Parser;
use crepe::{CrepeIndex, Verifier};
use merkle::MerkleTree;
use stair::SymbolTrie;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "antigravity-fast-core",
    version = "0.1.0",
    about = "High-speed native code intelligence layer: BLAKE3 Merkle tree, Crepe sparse n-gram index, Code STAIR symbol trie, and stdio MCP server"
)]
struct Cli {
    /// Workspace root directory (default: current directory)
    #[arg(short, long, default_value = ".")]
    workspace: PathBuf,

    /// Run empirical benchmarks comparing against PowerShell/Select-String baseline
    #[arg(short, long)]
    benchmark: bool,

    /// Run massive 50,000-file empirical scalability benchmark (generates or benchmarks 50k repo)
    #[arg(long)]
    benchmark_50k: bool,

    /// Force rebuilding Crepe sparse n-gram index
    #[arg(long)]
    index: bool,

    /// Search workspace using instant crepe grep
    #[arg(short, long)]
    grep: Option<String>,

    /// Print AST Symbol Table of Contents outline
    #[arg(short, long)]
    outline: Option<Option<String>>,

    /// Extract exact symbol code definition from Symbol Prefix Trie
    #[arg(short, long)]
    def: Option<String>,

    /// Show current BLAKE3 Merkle tree root and snapshot
    #[arg(short, long)]
    merkle: bool,

    /// Run stdio MCP server (default if no other command specified)
    #[arg(long)]
    mcp: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let workspace = fs::canonicalize(&cli.workspace).unwrap_or(cli.workspace);

    if cli.benchmark {
        let runner = BenchmarkRunner::new(workspace.clone());
        let report = runner.run_suite();
        let report_path = workspace.join("BENCHMARK_RESULTS.md");
        fs::write(&report_path, &report.summary_markdown)?;
        println!("Benchmark report saved to: {:?}", report_path);
        return Ok(());
    }

    if cli.benchmark_50k {
        let bench_dir = workspace.join("benchmark_50k_repo");
        let runner = Scalability50kRunner::new(bench_dir);
        let report = runner.run_suite();
        let report_path = workspace.join("BENCHMARK_50K_REPORT.md");
        fs::write(&report_path, &report.markdown_summary)?;
        println!("50,000-File Scalability Benchmark report saved to: {:?}", report_path);
        return Ok(());
    }

    if cli.index {
        println!("Building Crepe sparse n-gram index for {:?}", workspace);
        let crepe = CrepeIndex::new(workspace.clone());
        let meta = crepe.build()?;
        println!("Indexed {} files, {} unique n-grams in {} ms", meta.total_files, meta.total_ngrams, meta.build_duration_ms);
        return Ok(());
    }

    if let Some(query) = cli.grep {
        let crepe = CrepeIndex::new(workspace.clone());
        if crepe.total_files() == 0 {
            println!("Crepe index not found. Building index first...");
            crepe.build()?;
        }
        let verifier = Verifier::new(workspace.clone());
        let t0 = std::time::Instant::now();
        let candidates = crepe.find_candidates(&query);
        let matches = verifier.verify_in_process(&query, &candidates, false)?;
        let dur = t0.elapsed();
        println!("Instant Grep for '{}' (Found {} matches across {}/{} candidate files in {:.2} ms):",
            query, matches.len(), candidates.len(), crepe.total_files(), dur.as_secs_f64() * 1000.0);
        for m in matches.iter().take(20) {
            println!("{}:{}:{}: {}", m.file, m.line_number, m.column, m.line_content.trim());
        }
        return Ok(());
    }

    if let Some(path_opt) = cli.outline {
        let trie = SymbolTrie::new(workspace.clone());
        let count = trie.build();
        println!("AST Symbol Outline ({} resident symbols):", count);
        let filter = path_opt.as_deref();
        for (file, symbols) in trie.get_outline(filter) {
            println!("File: {}", file);
            for s in symbols {
                println!("  [{:?}] {} (L{}-L{}): {}", s.kind, s.name, s.line_start, s.line_end, s.signature);
            }
        }
        return Ok(());
    }

    if let Some(sym_path) = cli.def {
        let trie = SymbolTrie::new(workspace.clone());
        trie.build();
        match trie.extract_symbol_code(&sym_path) {
            Some((entry, code)) => {
                println!("// Symbol: {} [{:?}] in {}:{}-{}", entry.symbol_path, entry.kind, entry.file_path, entry.line_start, entry.line_end);
                println!("{}", code);
            }
            None => {
                eprintln!("Symbol '{}' not found.", sym_path);
            }
        }
        return Ok(());
    }

    if cli.merkle {
        let tree = MerkleTree::new(workspace.clone());
        let snap = tree.get_snapshot();
        println!("BLAKE3 Merkle Tree Root: {}", snap.root_hash);
        println!("Total Files: {} | Total Dirs: {} | Total Bytes: {}", snap.total_files, snap.total_dirs, snap.total_bytes);
        let diff = tree.refresh();
        println!("Dirty Diff Duration: {} µs (Changes: {})", diff.duration_micros, diff.total_changes);
        return Ok(());
    }

    // Default: Run MCP stdio server
    mcp::run_stdio_server(workspace)?;
    Ok(())
}
