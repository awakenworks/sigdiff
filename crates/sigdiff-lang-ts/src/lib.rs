use sigdiff_core::{
    Error, FileSignatures, LanguageProvider, Reference, Result, Signature, SignatureKind,
    Visibility,
};
use std::path::Path;
use tree_sitter_tags::{TagsConfiguration, TagsContext};

pub struct TypeScriptProvider {
    ts_config: TagsConfiguration,
    tsx_config: TagsConfiguration,
}

impl TypeScriptProvider {
    pub fn new() -> Self {
        let ts_language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        let ts_config = TagsConfiguration::new(
            ts_language,
            tree_sitter_typescript::TAGS_QUERY,
            tree_sitter_typescript::LOCALS_QUERY,
        )
        .expect("failed to create TypeScript TagsConfiguration");

        let tsx_language = tree_sitter_typescript::LANGUAGE_TSX.into();
        let tsx_config = TagsConfiguration::new(
            tsx_language,
            tree_sitter_typescript::TAGS_QUERY,
            tree_sitter_typescript::LOCALS_QUERY,
        )
        .expect("failed to create TSX TagsConfiguration");

        Self {
            ts_config,
            tsx_config,
        }
    }

    fn config_for(&self, path: &Path) -> &TagsConfiguration {
        match path.extension().and_then(|e| e.to_str()) {
            Some("tsx") | Some("jsx") => &self.tsx_config,
            _ => &self.ts_config,
        }
    }
}

impl Default for TypeScriptProvider {
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

fn detect_visibility(_sig_text: &str) -> Visibility {
    // JS/TS doesn't have visibility keywords tracked by tree-sitter tags
    Visibility::Public
}

impl LanguageProvider for TypeScriptProvider {
    fn name(&self) -> &'static str {
        "typescript"
    }

    fn extensions(&self) -> &[&'static str] {
        &["ts", "tsx", "js", "jsx"]
    }

    fn extract_signatures(&self, path: &Path, source: &[u8]) -> Result<FileSignatures> {
        let config = self.config_for(path);
        let mut context = TagsContext::new();
        let (tags_iter, _has_locals) = context
            .generate_tags(config, source, None)
            .map_err(|e| Error::Parse(e.to_string()))?;

        let mut signatures = Vec::new();
        for tag_result in tags_iter {
            let tag = tag_result.map_err(|e| Error::Parse(e.to_string()))?;
            if !tag.is_definition {
                continue;
            }

            let syntax_type = config.syntax_type_name(tag.syntax_type_id);
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
        let config = self.config_for(path);
        let mut context = TagsContext::new();
        let (tags_iter, _has_locals) = context
            .generate_tags(config, source, None)
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
    fn extracts_function_signature() {
        let provider = TypeScriptProvider::new();
        // function_signature in an interface is what the TS tags query captures
        let source = b"interface Greeter {\n    greet(name: string): string;\n}\n";
        let result = provider
            .extract_signatures(Path::new("test.ts"), source)
            .unwrap();
        let names: Vec<&str> = result.signatures.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"greet"));
        let sig = result
            .signatures
            .iter()
            .find(|s| s.name == "greet")
            .unwrap();
        assert!(matches!(sig.kind, SignatureKind::Method));
    }

    #[test]
    fn extracts_interface() {
        let provider = TypeScriptProvider::new();
        let source = b"interface Animal {\n    name: string;\n    speak(): void;\n}\n";
        let result = provider
            .extract_signatures(Path::new("test.ts"), source)
            .unwrap();
        let names: Vec<&str> = result.signatures.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Animal"));
        let iface_sig = result
            .signatures
            .iter()
            .find(|s| s.name == "Animal")
            .unwrap();
        assert!(matches!(iface_sig.kind, SignatureKind::Trait));
    }
}
