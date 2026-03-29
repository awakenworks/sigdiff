use crate::signature::FileSignatures;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    pub file: PathBuf,
    pub name: String,
    pub line: usize,
}

pub trait LanguageProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &[&'static str];
    fn extract_signatures(&self, path: &Path, source: &[u8]) -> crate::Result<FileSignatures>;
    fn extract_references(&self, path: &Path, source: &[u8]) -> crate::Result<Vec<Reference>>;
}

pub struct LanguageRegistry {
    providers: Vec<Box<dyn LanguageProvider>>,
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: impl LanguageProvider + 'static) {
        self.providers.push(Box::new(provider));
    }

    pub fn detect(&self, path: &Path) -> Option<&dyn LanguageProvider> {
        let ext = path.extension()?.to_str()?;
        self.providers
            .iter()
            .find(|p| p.extensions().contains(&ext))
            .map(|p| p.as_ref())
    }

    pub fn providers(&self) -> &[Box<dyn LanguageProvider>] {
        &self.providers
    }
}
