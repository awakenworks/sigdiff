pub mod cache;
pub mod diff;
pub mod filter;
pub mod git;
pub mod provider;
pub mod refs;
pub mod render;
pub mod signature;

pub use diff::{FileDiff, SignatureChange, diff_file_signatures, diff_signatures};
pub use filter::{MapFilter, parse_kind};
pub use provider::{LanguageProvider, LanguageRegistry, Reference};
pub use refs::{FileRefs, RefLink, resolve_refs};
pub use signature::{FileSignatures, Signature, SignatureKind, Visibility};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("tree-sitter parse error: {0}")]
    Parse(String),

    #[error("git error: {0}")]
    Git(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
