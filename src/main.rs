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
            let source = match std::fs::read(file) {
                Ok(s) => s,
                Err(_) => continue, // skip missing/unreadable files (broken symlinks, deleted but tracked)
            };
            let sigs = match provider.extract_signatures(file, &source) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let refs = provider
                .extract_references(file, &source)
                .unwrap_or_default();
            if let Ok(meta) = std::fs::metadata(file)
                && let Ok(mtime) = meta.modified()
            {
                let entry = CacheEntry {
                    mtime,
                    signatures: sigs.signatures.clone(),
                    references: refs.clone(),
                };
                let _ = cache.put(file, &entry);
            }
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

struct MapArgs<'a> {
    path: Option<PathBuf>,
    max_tokens: Option<usize>,
    max_depth: Option<usize>,
    format: &'a Format,
    no_color: bool,
    lang: Option<&'a str>,
    public_only: bool,
    kind: Option<&'a str>,
    grep: Option<&'a str>,
}

fn cmd_map(args: MapArgs<'_>) -> anyhow::Result<()> {
    let start = args.path.clone().unwrap_or_else(|| PathBuf::from("."));
    let start = std::fs::canonicalize(&start)
        .with_context(|| format!("cannot canonicalize path {}", start.display()))?;

    let repo_root = git::repo_root(&start).context("could not find git repository root")?;

    // Determine path prefix for filtering (relative to repo root)
    let path_prefix = if let Some(p) = &args.path {
        let abs_path = std::fs::canonicalize(p)
            .with_context(|| format!("cannot canonicalize path {}", p.display()))?;
        if abs_path != repo_root {
            abs_path.strip_prefix(&repo_root).ok().map(|rel| {
                let mut prefix = rel.to_string_lossy().into_owned();
                if !prefix.ends_with('/') {
                    prefix.push('/');
                }
                prefix
            })
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
    let langs = args.lang.map(|l| {
        l.split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    });
    let kinds = args.kind.map(|k| {
        k.split(',')
            .filter_map(|s| parse_kind(s.trim()))
            .collect::<Vec<_>>()
    });
    let map_filter = MapFilter {
        lang: langs,
        public_only: args.public_only,
        kinds,
        grep: args.grep.map(|s| s.to_string()),
        max_depth: args.max_depth,
        path_prefix,
    };
    let file_sigs = map_filter.apply(&file_sigs);

    use std::io::IsTerminal;
    let color = !args.no_color && std::io::stdout().is_terminal();

    let output = match args.format {
        Format::Text => render_text::render_map(&file_sigs, color),
        Format::Json => {
            render_json::render_map_json(&file_sigs).map_err(|e| anyhow::anyhow!("{e}"))?
        }
    };

    // Apply max_tokens budget: truncate if over limit
    let output = if let Some(budget) = args.max_tokens {
        let approx_tokens = output.len() / 4;
        if approx_tokens > budget {
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
        let changed = git::diff_worktree(&repo_root).context("git diff failed")?;

        let mut old_sigs: Vec<FileSignatures> = Vec::new();
        let mut new_sigs: Vec<FileSignatures> = Vec::new();

        for (status, rel_path) in &changed {
            let provider = match registry.detect(rel_path) {
                Some(p) => p,
                None => continue,
            };
            let path_str = rel_path.to_string_lossy();

            // Old from HEAD (skip for newly added files)
            if *status != git::FileStatus::Added
                && let Ok(old_source) = git::show_file(&repo_root, "HEAD", &path_str)
                && let Ok(fs) = provider.extract_signatures(rel_path, &old_source)
            {
                old_sigs.push(fs);
            }

            // New from working tree (skip for deleted files)
            if *status != git::FileStatus::Deleted {
                let abs_path = repo_root.join(rel_path);
                if let Ok(new_source) = std::fs::read(&abs_path)
                    && let Ok(fs) = provider.extract_signatures(rel_path, &new_source)
                {
                    new_sigs.push(fs);
                }
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
    // Try to canonicalize; if the file doesn't exist, resolve repo root from cwd
    let (repo_root, rel_target) = if let Ok(abs_path) = std::fs::canonicalize(&path) {
        let parent = abs_path.parent().unwrap_or(&abs_path);
        let root = git::repo_root(parent).context("could not find git repository root")?;
        let rel = abs_path
            .strip_prefix(&root)
            .unwrap_or(&abs_path)
            .to_path_buf();
        (root, rel)
    } else {
        // File doesn't exist on disk — use cwd to find repo root, treat path as relative
        let root = git::repo_root(Path::new(".")).context("could not find git repository root")?;
        (root, path.clone())
    };

    let files = git::list_files(&repo_root).context("git ls-files failed")?;

    let cache_dir = repo_root.join(".sigdiff").join("cache");
    let cache = Cache::new(cache_dir);

    let registry = build_registry();
    let (_file_sigs, all_signatures, all_references) =
        scan_files(&registry, &repo_root, &files, &cache)?;

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
        } => cmd_map(MapArgs {
            path: path.clone(),
            max_tokens: *max_tokens,
            max_depth: *max_depth,
            format,
            no_color: *no_color,
            lang: lang.as_deref(),
            public_only: *public_only,
            kind: kind.as_deref(),
            grep: grep.as_deref(),
        })?,
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
