use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use sigdiff_core::{
    FileSignatures, MapFilter, Reference, Signature,
    cache::{Cache, CacheEntry},
    filter::parse_kind,
    git,
    render::{json as render_json, text as render_text},
};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "sigdiff", about = "Signature-level code diff tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show a map of all signatures in the repository
    Map {
        /// Path to filter (show only files under this path)
        path: Option<PathBuf>,
        #[arg(long)]
        max_tokens: Option<usize>,
        #[arg(long)]
        max_depth: Option<usize>,
        #[arg(long, default_value = "text")]
        format: Format,
        #[arg(long)]
        no_color: bool,
        /// Filter by language (comma-separated, e.g. rust,python)
        #[arg(long)]
        lang: Option<String>,
        /// Show only public signatures
        #[arg(long)]
        public_only: bool,
        /// Filter by signature kind (comma-separated, e.g. struct,trait)
        #[arg(long)]
        kind: Option<String>,
        /// Show only signatures whose name contains this pattern (case-insensitive)
        #[arg(long)]
        grep: Option<String>,
    },
    /// Show signature-level diff between git commits or worktree
    Diff {
        /// Git range like HEAD~1..HEAD (defaults to HEAD vs working tree)
        range: Option<String>,
        /// Path to the repository (defaults to current directory)
        path: Option<PathBuf>,
        #[arg(long, default_value = "text")]
        format: Format,
        #[arg(long)]
        no_color: bool,
    },
    /// Show references (uses/used-by) for a file
    Refs {
        /// Path to the file to resolve references for
        path: PathBuf,
        #[arg(long, default_value = "1")]
        depth: usize,
        #[arg(long, default_value = "both")]
        direction: Direction,
        #[arg(long, default_value = "text")]
        format: Format,
        #[arg(long)]
        no_color: bool,
    },
    /// List all registered language providers
    Langs,
}

#[derive(Clone, ValueEnum)]
enum Format {
    Text,
    Json,
}

#[derive(Clone, ValueEnum)]
enum Direction {
    Uses,
    UsedBy,
    Both,
}

fn build_registry() -> sigdiff_core::LanguageRegistry {
    let mut reg = sigdiff_core::LanguageRegistry::new();
    #[cfg(feature = "lang-rust")]
    reg.register(sigdiff_lang_rust::RustProvider::new());
    #[cfg(feature = "lang-python")]
    reg.register(sigdiff_lang_python::PythonProvider::new());
    #[cfg(feature = "lang-ts")]
    reg.register(sigdiff_lang_ts::TypeScriptProvider::new());
    #[cfg(feature = "lang-go")]
    reg.register(sigdiff_lang_go::GoProvider::new());
    #[cfg(feature = "lang-java")]
    reg.register(sigdiff_lang_java::JavaProvider::new());
    reg
}

/// Scan a list of files using the registry and cache, returning all signatures and references.
fn scan_files(
    registry: &sigdiff_core::LanguageRegistry,
    repo_root: &Path,
    files: &[PathBuf],
    cache: &Cache,
) -> anyhow::Result<(Vec<FileSignatures>, Vec<Signature>, Vec<Reference>)> {
    let mut all_file_sigs: Vec<FileSignatures> = Vec::new();
    let mut all_signatures: Vec<Signature> = Vec::new();
    let mut all_references: Vec<Reference> = Vec::new();

    for file in files {
        let provider = match registry.detect(file) {
            Some(p) => p,
            None => continue,
        };

        let (sigs, refs) = if let Ok(Some(entry)) = cache.get(file) {
            (entry.signatures, entry.references)
        } else {
            let source = std::fs::read(file)
                .with_context(|| format!("failed to read {}", file.display()))?;
            let sigs = provider
                .extract_signatures(file, &source)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let refs = provider
                .extract_references(file, &source)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let mtime = std::fs::metadata(file)?.modified()?;
            let entry = CacheEntry {
                mtime,
                signatures: sigs.signatures.clone(),
                references: refs.clone(),
            };
            let _ = cache.put(file, &entry);
            (sigs.signatures, refs)
        };

        let rel_path = file.strip_prefix(repo_root).unwrap_or(file).to_path_buf();

        let file_sigs = FileSignatures {
            path: rel_path.clone(),
            language: provider.name().to_string(),
            signatures: sigs
                .iter()
                .map(|s| {
                    let mut s2 = s.clone();
                    s2.file = rel_path.clone();
                    s2
                })
                .collect(),
        };
        let fixed_refs: Vec<Reference> = refs
            .into_iter()
            .map(|r| Reference {
                file: rel_path.clone(),
                ..r
            })
            .collect();

        all_signatures.extend(file_sigs.signatures.clone());
        all_references.extend(fixed_refs);
        all_file_sigs.push(file_sigs);
    }

    // Sort by path for deterministic output
    all_file_sigs.sort_by(|a, b| a.path.cmp(&b.path));

    Ok((all_file_sigs, all_signatures, all_references))
}

fn cmd_map(
    path: Option<PathBuf>,
    max_tokens: Option<usize>,
    max_depth: Option<usize>,
    format: &Format,
    no_color: bool,
    lang: Option<&str>,
    public_only: bool,
    kind: Option<&str>,
    grep: Option<&str>,
) -> anyhow::Result<()> {
    let start = path.clone().unwrap_or_else(|| PathBuf::from("."));
    let start = std::fs::canonicalize(&start)
        .with_context(|| format!("cannot canonicalize path {}", start.display()))?;

    let repo_root = git::repo_root(&start).context("could not find git repository root")?;

    // Determine path prefix for filtering (relative to repo root)
    let path_prefix = if let Some(p) = &path {
        let abs_path = std::fs::canonicalize(p)
            .with_context(|| format!("cannot canonicalize path {}", p.display()))?;
        // Only apply prefix filter if the given path is inside the repo and not the repo root itself
        if abs_path != repo_root {
            if let Ok(rel) = abs_path.strip_prefix(&repo_root) {
                let mut prefix = rel.to_string_lossy().into_owned();
                if !prefix.ends_with('/') {
                    prefix.push('/');
                }
                Some(prefix)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let files = git::list_files(&repo_root).context("git ls-files failed")?;

    let cache_dir = repo_root.join(".sigdiff").join("cache");
    let cache = Cache::new(cache_dir);

    let registry = build_registry();
    let (file_sigs, _sigs, _refs) = scan_files(&registry, &repo_root, &files, &cache)?;

    // Build and apply filter
    let langs = lang.map(|l| {
        l.split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    });
    let kinds = kind.map(|k| {
        k.split(',')
            .filter_map(|s| parse_kind(s.trim()))
            .collect::<Vec<_>>()
    });
    let map_filter = MapFilter {
        lang: langs,
        public_only,
        kinds,
        grep: grep.map(|s| s.to_string()),
        max_depth,
        path_prefix,
    };
    let file_sigs = map_filter.apply(&file_sigs);

    use std::io::IsTerminal;
    let color = !no_color && std::io::stdout().is_terminal();

    let output = match format {
        Format::Text => render_text::render_map(&file_sigs, color),
        Format::Json => {
            render_json::render_map_json(&file_sigs).map_err(|e| anyhow::anyhow!("{e}"))?
        }
    };

    // Apply max_tokens budget: truncate if over limit
    let output = if let Some(budget) = max_tokens {
        let estimated_tokens = output.len() / 4;
        if estimated_tokens > budget {
            let char_limit = budget * 4;
            let mut truncated = output[..char_limit.min(output.len())].to_string();
            truncated.push_str("\n... (output truncated due to --max-tokens budget)\n");
            truncated
        } else {
            output
        }
    } else {
        output
    };

    print!("{output}");
    Ok(())
}

fn cmd_diff(
    range: Option<String>,
    path: Option<PathBuf>,
    format: &Format,
    no_color: bool,
) -> anyhow::Result<()> {
    let start = path.unwrap_or_else(|| PathBuf::from("."));
    let start = std::fs::canonicalize(&start)
        .with_context(|| format!("cannot canonicalize path {}", start.display()))?;

    let repo_root = git::repo_root(&start).context("could not find git repository root")?;

    let registry = build_registry();

    let (old_file_sigs, new_file_sigs) = if let Some(range) = range {
        // Parse "old..new" or use as single ref vs HEAD
        let (old_ref, new_ref) = if let Some((o, n)) = range.split_once("..") {
            (o.to_string(), n.to_string())
        } else {
            // treat argument as old ref, compare to HEAD
            (range, "HEAD".to_string())
        };

        let changed = git::diff_names(&repo_root, &old_ref, &new_ref)
            .context("git diff --name-status failed")?;

        let mut old_sigs: Vec<FileSignatures> = Vec::new();
        let mut new_sigs: Vec<FileSignatures> = Vec::new();

        for (_status, rel_path) in &changed {
            let path_str = rel_path.to_string_lossy();

            // Old version from git
            if let Ok(old_source) = git::show_file(&repo_root, &old_ref, &path_str)
                && let Some(provider) = registry.detect(rel_path)
                && let Ok(fs) = provider.extract_signatures(rel_path, &old_source)
            {
                old_sigs.push(fs);
            }

            // New version from git
            if let Ok(new_source) = git::show_file(&repo_root, &new_ref, &path_str)
                && let Some(provider) = registry.detect(rel_path)
                && let Ok(fs) = provider.extract_signatures(rel_path, &new_source)
            {
                new_sigs.push(fs);
            }
        }

        (old_sigs, new_sigs)
    } else {
        // Worktree diff: compare HEAD with current working tree
        let changed = git::diff_names(&repo_root, "HEAD", "")
            .or_else(|_| {
                // fallback: compare HEAD with working tree using diff against index
                git::diff_names(&repo_root, "--cached", "HEAD")
            })
            .context("git diff failed")?;

        let mut old_sigs: Vec<FileSignatures> = Vec::new();
        let mut new_sigs: Vec<FileSignatures> = Vec::new();

        // Get all tracked files and compare HEAD vs working tree
        let files = git::list_files(&repo_root).context("git ls-files failed")?;

        for file in &files {
            let rel_path = file.strip_prefix(&repo_root).unwrap_or(file).to_path_buf();
            let rel_str = rel_path.to_string_lossy();

            let provider = match registry.detect(&rel_path) {
                Some(p) => p,
                None => continue,
            };

            // Check if this file was in the diff
            let is_changed = changed.iter().any(|(_, p)| p == &rel_path);
            if !is_changed {
                continue;
            }

            // Old from HEAD
            if let Ok(old_source) = git::show_file(&repo_root, "HEAD", &rel_str)
                && let Ok(fs) = provider.extract_signatures(&rel_path, &old_source)
            {
                old_sigs.push(fs);
            }

            // New from working tree
            if let Ok(new_source) = std::fs::read(file)
                && let Ok(fs) = provider.extract_signatures(&rel_path, &new_source)
            {
                new_sigs.push(fs);
            }
        }

        (old_sigs, new_sigs)
    };

    let diffs = sigdiff_core::diff_file_signatures(&old_file_sigs, &new_file_sigs);

    use std::io::IsTerminal;
    let color = !no_color && std::io::stdout().is_terminal();

    let output = match format {
        Format::Text => render_text::render_diff(&diffs, color),
        Format::Json => {
            render_json::render_diff_json(&diffs).map_err(|e| anyhow::anyhow!("{e}"))?
        }
    };
    print!("{output}");
    Ok(())
}

fn cmd_refs(
    path: PathBuf,
    _depth: usize,
    direction: &Direction,
    format: &Format,
    no_color: bool,
) -> anyhow::Result<()> {
    // Determine repo root from the path's directory
    let abs_path = std::fs::canonicalize(&path)
        .with_context(|| format!("cannot canonicalize path {}", path.display()))?;

    let parent = abs_path.parent().unwrap_or(&abs_path);
    let repo_root = git::repo_root(parent).context("could not find git repository root")?;

    let files = git::list_files(&repo_root).context("git ls-files failed")?;

    let cache_dir = repo_root.join(".sigdiff").join("cache");
    let cache = Cache::new(cache_dir);

    let registry = build_registry();
    let (_file_sigs, all_signatures, all_references) =
        scan_files(&registry, &repo_root, &files, &cache)?;

    let rel_target = abs_path
        .strip_prefix(&repo_root)
        .unwrap_or(&abs_path)
        .to_path_buf();

    let mut file_refs = sigdiff_core::resolve_refs(&rel_target, &all_signatures, &all_references);

    // Filter by direction
    match direction {
        Direction::Uses => {
            file_refs.used_by.clear();
        }
        Direction::UsedBy => {
            file_refs.uses.clear();
        }
        Direction::Both => {}
    }

    use std::io::IsTerminal;
    let color = !no_color && std::io::stdout().is_terminal();

    let output = match format {
        Format::Text => render_text::render_refs(&file_refs, color),
        Format::Json => {
            render_json::render_refs_json(&file_refs).map_err(|e| anyhow::anyhow!("{e}"))?
        }
    };
    print!("{output}");
    Ok(())
}

fn cmd_langs() {
    let registry = build_registry();
    for provider in registry.providers() {
        let exts = provider
            .extensions()
            .iter()
            .map(|e| format!(".{e}"))
            .collect::<Vec<_>>()
            .join(", ");
        println!("{}: {}", provider.name(), exts);
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Map {
            path,
            max_tokens,
            max_depth,
            format,
            no_color,
            lang,
            public_only,
            kind,
            grep,
        } => cmd_map(
            path.clone(),
            *max_tokens,
            *max_depth,
            format,
            *no_color,
            lang.as_deref(),
            *public_only,
            kind.as_deref(),
            grep.as_deref(),
        )?,
        Commands::Diff {
            range,
            path,
            format,
            no_color,
        } => cmd_diff(range.clone(), path.clone(), format, *no_color)?,
        Commands::Refs {
            path,
            depth,
            direction,
            format,
            no_color,
        } => cmd_refs(path.clone(), *depth, direction, format, *no_color)?,
        Commands::Langs => cmd_langs(),
    }

    Ok(())
}
