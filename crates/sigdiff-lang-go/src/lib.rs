use sigdiff_core::{
    Error, FileSignatures, LanguageProvider, Reference, Result, Signature, SignatureKind,
    Visibility,
};
use std::path::Path;
use tree_sitter_tags::{TagsConfiguration, TagsContext};

pub struct GoProvider {
    config: TagsConfiguration,
}

impl GoProvider {
    pub fn new() -> Self {
        let language = tree_sitter_go::LANGUAGE.into();
        let config = TagsConfiguration::new(language, tree_sitter_go::TAGS_QUERY, "")
            .expect("failed to create Go TagsConfiguration");
        Self { config }
    }
}

impl Default for GoProvider {
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

fn detect_visibility(name: &str) -> Visibility {
    // Go convention: exported identifiers start with uppercase
    if name.starts_with(|c: char| c.is_uppercase()) {
        Visibility::Public
    } else {
        Visibility::Private
    }
}

impl LanguageProvider for GoProvider {
    fn name(&self) -> &'static str {
        "go"
    }

    fn extensions(&self) -> &[&'static str] {
        &["go"]
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
            let visibility = detect_visibility(&name);

            let text = extract_signature_text(source, tag.range.start, tag.range.end);
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
    fn extracts_function() {
        let provider = GoProvider::new();
        let source = b"package main\n\nfunc Add(a, b int) int {\n\treturn a + b\n}\n";
        let result = provider
            .extract_signatures(Path::new("test.go"), source)
            .unwrap();
        let add_sig = result.signatures.iter().find(|s| s.name == "Add").unwrap();
        assert!(matches!(add_sig.kind, SignatureKind::Function));
        assert!(matches!(add_sig.visibility, Visibility::Public));
        // Should not include the body
        assert!(!add_sig.text.contains("return"));
    }

    #[test]
    fn detects_unexported_as_private() {
        let provider = GoProvider::new();
        let source = b"package main\n\nfunc helper() int {\n\treturn 42\n}\n";
        let result = provider
            .extract_signatures(Path::new("test.go"), source)
            .unwrap();
        let helper_sig = result
            .signatures
            .iter()
            .find(|s| s.name == "helper")
            .unwrap();
        assert!(matches!(helper_sig.visibility, Visibility::Private));
    }
}
