use sigdiff_core::{
    Error, FileSignatures, LanguageProvider, Reference, Result, Signature, SignatureKind,
    Visibility,
};
use std::path::Path;
use tree_sitter_tags::{TagsConfiguration, TagsContext};

pub struct PythonProvider {
    config: TagsConfiguration,
}

impl PythonProvider {
    pub fn new() -> Self {
        let language = tree_sitter_python::LANGUAGE.into();
        let config = TagsConfiguration::new(language, tree_sitter_python::TAGS_QUERY, "")
            .expect("failed to create Python TagsConfiguration");
        Self { config }
    }
}

impl Default for PythonProvider {
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
    // For Python, take only the first line (the def/class line)
    if let Some(pos) = text.find('\n') {
        text[..pos].trim_end().to_string()
    } else {
        text.trim().to_string()
    }
}

fn detect_visibility(name: &str) -> Visibility {
    if name.starts_with("__") && name.ends_with("__") {
        // Dunder methods (__init__, __str__, etc.) are public
        Visibility::Public
    } else if name.starts_with("__") {
        Visibility::Private
    } else if name.starts_with('_') {
        Visibility::Crate
    } else {
        Visibility::Public
    }
}

impl LanguageProvider for PythonProvider {
    fn name(&self) -> &'static str {
        "python"
    }

    fn extensions(&self) -> &[&'static str] {
        &["py"]
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
    fn extracts_function_def() {
        let provider = PythonProvider::new();
        let source = b"def greet(name: str) -> str:\n    return 'Hello'\n";
        let result = provider
            .extract_signatures(Path::new("test.py"), source)
            .unwrap();
        assert_eq!(result.signatures.len(), 1);
        assert_eq!(result.signatures[0].name, "greet");
        assert!(matches!(result.signatures[0].kind, SignatureKind::Function));
        // Should only be first line, no newline in text
        assert!(!result.signatures[0].text.contains('\n'));
    }

    #[test]
    fn extracts_class_and_method() {
        let provider = PythonProvider::new();
        let source = b"class MyClass:\n    def my_method(self):\n        pass\n";
        let result = provider
            .extract_signatures(Path::new("test.py"), source)
            .unwrap();
        let names: Vec<&str> = result.signatures.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"MyClass"));
        assert!(names.contains(&"my_method"));
        let class_sig = result
            .signatures
            .iter()
            .find(|s| s.name == "MyClass")
            .unwrap();
        assert!(matches!(class_sig.kind, SignatureKind::Class));
    }

    #[test]
    fn empty_source_returns_no_signatures() {
        let provider = PythonProvider::new();
        let result = provider
            .extract_signatures(Path::new("empty.py"), b"")
            .unwrap();
        assert!(result.signatures.is_empty());
    }

    #[test]
    fn detects_private_double_underscore() {
        let provider = PythonProvider::new();
        let source = b"def __secret():\n    pass\n";
        let result = provider
            .extract_signatures(Path::new("test.py"), source)
            .unwrap();
        assert_eq!(result.signatures.len(), 1);
        assert!(matches!(
            result.signatures[0].visibility,
            Visibility::Private
        ));
    }

    #[test]
    fn detects_protected_single_underscore() {
        let provider = PythonProvider::new();
        let source = b"def _internal():\n    pass\n";
        let result = provider
            .extract_signatures(Path::new("test.py"), source)
            .unwrap();
        assert_eq!(result.signatures.len(), 1);
        assert!(matches!(result.signatures[0].visibility, Visibility::Crate));
    }

    #[test]
    fn dunder_methods_are_public() {
        let provider = PythonProvider::new();
        let source = b"class Foo:\n    def __init__(self):\n        pass\n";
        let result = provider
            .extract_signatures(Path::new("test.py"), source)
            .unwrap();
        let init_sig = result
            .signatures
            .iter()
            .find(|s| s.name == "__init__")
            .unwrap();
        // __init__ ends with __, so it's public (dunder method)
        assert!(matches!(init_sig.visibility, Visibility::Public));
    }

    #[test]
    fn extracts_references() {
        let provider = PythonProvider::new();
        let source = b"def greet():\n    pass\n\ndef main():\n    greet()\n";
        let refs = provider
            .extract_references(Path::new("test.py"), source)
            .unwrap();
        let names: Vec<&str> = refs.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"greet"));
    }

    #[test]
    fn signature_text_is_first_line_only() {
        let provider = PythonProvider::new();
        let source = b"def multi_line(\n    arg1: int,\n    arg2: str\n) -> None:\n    pass\n";
        let result = provider
            .extract_signatures(Path::new("test.py"), source)
            .unwrap();
        assert!(!result.signatures[0].text.contains('\n'));
    }
}
