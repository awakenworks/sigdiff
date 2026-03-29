pub mod git;
pub mod provider;
pub mod signature;

pub use provider::{LanguageProvider, LanguageRegistry, Reference};
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
