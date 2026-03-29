use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignatureKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Impl,
    Const,
    TypeAlias,
    Module,
    Interface,
    Class,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Private,
    Crate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    pub file: PathBuf,
    pub name: String,
    pub kind: SignatureKind,
    pub visibility: Visibility,
    pub text: String,
    pub line: usize,
    pub parent: Option<String>,
}

impl Signature {
    pub fn match_key(&self) -> (&PathBuf, &str, &SignatureKind, &Option<String>) {
        (&self.file, &self.name, &self.kind, &self.parent)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSignatures {
    pub path: PathBuf,
    pub language: String,
    pub signatures: Vec<Signature>,
}
