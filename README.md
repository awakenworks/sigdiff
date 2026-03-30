# sigdiff

A signature-level code diff tool. Instead of showing line-by-line changes, sigdiff extracts function, struct, class, and other symbol signatures from your codebase and tracks their changes at the semantic level.

Built with [tree-sitter](https://tree-sitter.github.io/) for fast, accurate parsing across multiple languages.

## Features

- **Signature map** -- List all function/struct/class/trait signatures in a repository
- **Signature diff** -- Show added, removed, and modified signatures between git commits or the working tree
- **Cross-file references** -- Trace which files use or are used by a given file's definitions
- **Multi-language** -- Rust, Python, TypeScript/JavaScript (including JSX/TSX), Go, Java
- **Filtering** -- By language, visibility (public/private), signature kind, name pattern, directory depth
- **Output formats** -- Colored terminal text or JSON
- **Caching** -- Signatures are cached by file mtime for fast repeated scans

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
git clone https://github.com/awakenworks/sigdiff.git
cd sigdiff
cargo build --release
# Binary is at target/release/sigdiff
```

### Feature flags

All language providers are enabled by default. Disable unneeded ones to reduce binary size:

```bash
cargo install --path . --no-default-features --features lang-rust,lang-python
```

Available features: `lang-rust`, `lang-python`, `lang-ts`, `lang-go`, `lang-java`

## Usage

### `sigdiff map` -- Show all signatures

```bash
# Show all signatures in the repo
sigdiff map

# Filter to a specific directory
sigdiff map src/

# Filter by language and visibility
sigdiff map --lang rust --public-only

# Filter by kind (function, method, struct, enum, trait, class, interface, const, module)
sigdiff map --kind struct,trait

# Search by name pattern (case-insensitive)
sigdiff map --grep Provider

# JSON output
sigdiff map --format json

# Limit output size (approximate token budget)
sigdiff map --max-tokens 2000
```

Example output:

```
crates/sigdiff-core/src/diff.rs:
    pub enum SignatureChange
    pub struct FileDiff
    pub fn diff_signatures(old: &[Signature], new: &[Signature]) -> Vec<SignatureChange>
    pub fn diff_file_signatures(...)  -> Vec<FileDiff>

crates/sigdiff-core/src/filter.rs:
    pub struct MapFilter
    pub fn apply(&self, files: &[FileSignatures]) -> Vec<FileSignatures>
    pub fn parse_kind(s: &str) -> Option<SignatureKind>
```

### `sigdiff diff` -- Signature-level diff

```bash
# Diff working tree against HEAD
sigdiff diff

# Diff between two commits
sigdiff diff HEAD~3..HEAD

# Diff a single ref against HEAD
sigdiff diff main

# JSON output
sigdiff diff --format json
```

Example output:

```
src/lib.rs:
+   pub fn new_feature(config: &Config) -> Result<()>
-   pub fn deprecated_fn()
~   pub fn update(data: &[u8])
    ->  pub fn update(data: &[u8], opts: Options)
```

### `sigdiff refs` -- Cross-file references

```bash
# Show what a file uses and what uses it
sigdiff refs src/main.rs

# Show only outgoing references
sigdiff refs --direction uses src/main.rs

# Show only incoming references
sigdiff refs --direction used-by src/lib.rs
```

### `sigdiff langs` -- List supported languages

```bash
sigdiff langs
```

```
rust: .rs
python: .py
typescript: .ts, .tsx, .js, .jsx
go: .go
java: .java
```

## Architecture

sigdiff is organized as a Cargo workspace:

| Crate | Purpose |
|-------|---------|
| `sigdiff` | CLI binary |
| `sigdiff-core` | Core library: diff engine, filter, git integration, caching, rendering |
| `sigdiff-lang-rust` | Rust language provider |
| `sigdiff-lang-python` | Python language provider |
| `sigdiff-lang-ts` | TypeScript/JavaScript provider |
| `sigdiff-lang-go` | Go language provider |
| `sigdiff-lang-java` | Java language provider |

Each language provider implements the `LanguageProvider` trait and uses tree-sitter for parsing.

## Adding a new language

1. Create a new crate `crates/sigdiff-lang-<name>/`
2. Implement the `LanguageProvider` trait from `sigdiff-core`
3. Add the crate to the workspace in `Cargo.toml`
4. Register it in `src/main.rs` behind a feature flag

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Ensure all tests pass: `cargo test --all`
4. Ensure clippy is clean: `cargo clippy --all`
5. Submit a pull request

## License

Dual-licensed under MIT or Apache-2.0, at your option. See [LICENSE](LICENSE) for details.
