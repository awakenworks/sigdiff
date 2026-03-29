use sigdiff_core::{
    Error, FileSignatures, LanguageProvider, Reference, Result, Signature, SignatureKind,
    Visibility,
};
use std::path::Path;
use tree_sitter_tags::{TagsConfiguration, TagsContext};

pub struct RustProvider {
    config: TagsConfiguration,
}

impl RustProvider {
    pub fn new() -> Self {
        let language = tree_sitter_rust::LANGUAGE.into();
        let config = TagsConfiguration::new(language, tree_sitter_rust::TAGS_QUERY, "")
            .expect("failed to create Rust TagsConfiguration");
        Self { config }
    }
}

impl Default for RustProvider {
    fn default() -> Self {
        Self::new()
    }
}

fn syntax_type_to_kind(syntax_type: &str) -> SignatureKind {
    match syntax_type {
        "function" => SignatureKind::Function,
        "method" => SignatureKind::Method,
        "struct" | "type" => SignatureKind::Struct,
        "enum" => SignatureKind::Enum,
        "trait" | "interface" => SignatureKind::Trait,
        "module" => SignatureKind::Module,
        "constant" => SignatureKind::Const,
        "class" => SignatureKind::Class,
        _ => SignatureKind::Function,
    }
}

fn extract_signature_text(source: &[u8], range_start: usize, range_end: usize) -> String {
    let slice = &source[range_start..range_end.min(source.len())];
    let text = String::from_utf8_lossy(slice).into_owned();
    // Truncate at the opening brace
    if let Some(pos) = text.find('{') {
        text[..pos].trim_end().to_string()
    } else {
        text.trim().to_string()
    }
}

fn detect_visibility(sig_text: &str) -> Visibility {
    if sig_text.contains("pub(crate)") {
        Visibility::Crate
    } else if sig_text.contains("pub") {
        Visibility::Public
    } else {
        Visibility::Private
    }
}

impl LanguageProvider for RustProvider {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn extensions(&self) -> &[&'static str] {
        &["rs"]
    }

    fn extract_signatures(&self, path: &Path, source: &[u8]) -> Result<FileSignatures> {
        let mut context = TagsContext::new();
        let (tags_iter, _has_locals) = context
            .generate_tags(&self.config, source, None)
            .map_err(|e| Error::Parse(e.to_string()))?;

        let mut signatures = Vec::new();
        for tag_result in tags_iter {
            let tag = tag_result.map_err(|e| Error::Parse(e.to_string()))?;
            if !tag.is_definition {
                continue;
            }

            let syntax_type = self.config.syntax_type_name(tag.syntax_type_id);
            let kind = syntax_type_to_kind(syntax_type);

            let name = String::from_utf8_lossy(&source[tag.name_range.clone()]).into_owned();

            let text = extract_signature_text(source, tag.range.start, tag.range.end);
            let visibility = detect_visibility(&text);

            let line = tag.span.start.row + 1;

            signatures.push(Signature {
                file: path.to_path_buf(),
                name,
                kind,
                visibility,
                text,
                line,
                parent: None,
            });
        }

        Ok(FileSignatures {
            path: path.to_path_buf(),
            language: self.name().to_string(),
            signatures,
        })
    }

    fn extract_references(&self, path: &Path, source: &[u8]) -> Result<Vec<Reference>> {
        let mut context = TagsContext::new();
        let (tags_iter, _has_locals) = context
            .generate_tags(&self.config, source, None)
            .map_err(|e| Error::Parse(e.to_string()))?;

        let mut references = Vec::new();
        for tag_result in tags_iter {
            let tag = tag_result.map_err(|e| Error::Parse(e.to_string()))?;
            if tag.is_definition {
                continue;
            }

            let name = String::from_utf8_lossy(&source[tag.name_range.clone()]).into_owned();
            let line = tag.span.start.row + 1;

            references.push(Reference {
                file: path.to_path_buf(),
                name,
                line,
            });
        }

        Ok(references)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sigdiff_core::SignatureKind;
    use std::path::Path;

    #[test]
    fn extracts_function_signatures() {
        let provider = RustProvider::new();
        let source = b"pub fn hello(name: &str) -> String { name.to_string() }";
        let result = provider
            .extract_signatures(Path::new("lib.rs"), source)
            .unwrap();
        assert_eq!(result.signatures.len(), 1);
        assert_eq!(result.signatures[0].name, "hello");
        assert!(matches!(result.signatures[0].kind, SignatureKind::Function));
    }

    #[test]
    fn extracts_struct_and_methods() {
        let provider = RustProvider::new();
        let source = br#"
pub struct User {
    name: String,
}

impl User {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}
"#;
        let result = provider
            .extract_signatures(Path::new("lib.rs"), source)
            .unwrap();
        let names: Vec<&str> = result.signatures.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"User"));
        assert!(names.contains(&"new"));
    }

    #[test]
    fn extracts_references() {
        let provider = RustProvider::new();
        let source = br#"
fn greet(name: &str) -> String {
    name.to_uppercase()
}
fn main() {
    greet("world");
}
"#;
        let refs = provider
            .extract_references(Path::new("lib.rs"), source)
            .unwrap();
        let names: Vec<&str> = refs.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"greet"));
    }
}
