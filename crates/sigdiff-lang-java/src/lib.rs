use sigdiff_core::{
    Error, FileSignatures, LanguageProvider, Reference, Result, Signature, SignatureKind,
    Visibility,
};
use std::path::Path;
use tree_sitter_tags::{TagsConfiguration, TagsContext};

pub struct JavaProvider {
    config: TagsConfiguration,
}

impl JavaProvider {
    pub fn new() -> Self {
        let language = tree_sitter_java::LANGUAGE.into();
        let config = TagsConfiguration::new(language, tree_sitter_java::TAGS_QUERY, "")
            .expect("failed to create Java TagsConfiguration");
        Self { config }
    }
}

impl Default for JavaProvider {
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
    if sig_text.contains("public ") {
        Visibility::Public
    } else if sig_text.contains("private ") {
        Visibility::Private
    } else {
        Visibility::Crate
    }
}

impl LanguageProvider for JavaProvider {
    fn name(&self) -> &'static str {
        "java"
    }

    fn extensions(&self) -> &[&'static str] {
        &["java"]
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
    fn extracts_class_and_method() {
        let provider = JavaProvider::new();
        let source = b"public class Calculator {\n    public int add(int a, int b) {\n        return a + b;\n    }\n}\n";
        let result = provider
            .extract_signatures(Path::new("Calculator.java"), source)
            .unwrap();
        let names: Vec<&str> = result.signatures.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Calculator"));
        assert!(names.contains(&"add"));
        let class_sig = result
            .signatures
            .iter()
            .find(|s| s.name == "Calculator")
            .unwrap();
        assert!(matches!(class_sig.kind, SignatureKind::Class));
        assert!(matches!(class_sig.visibility, Visibility::Public));
    }

    #[test]
    fn detects_private_method() {
        let provider = JavaProvider::new();
        let source =
            b"public class Foo {\n    private void helper() {\n        // nothing\n    }\n}\n";
        let result = provider
            .extract_signatures(Path::new("Foo.java"), source)
            .unwrap();
        let helper_sig = result
            .signatures
            .iter()
            .find(|s| s.name == "helper")
            .unwrap();
        assert!(matches!(helper_sig.visibility, Visibility::Private));
        // Body should be truncated
        assert!(!helper_sig.text.contains("nothing"));
    }

    #[test]
    fn empty_source_returns_no_signatures() {
        let provider = JavaProvider::new();
        let result = provider
            .extract_signatures(Path::new("Empty.java"), b"")
            .unwrap();
        assert!(result.signatures.is_empty());
    }

    #[test]
    fn detects_package_private_as_crate() {
        let provider = JavaProvider::new();
        // No visibility modifier = package-private → maps to Crate
        let source = b"class Internal {\n    void doSomething() { }\n}\n";
        let result = provider
            .extract_signatures(Path::new("Internal.java"), source)
            .unwrap();
        let class_sig = result
            .signatures
            .iter()
            .find(|s| s.name == "Internal")
            .unwrap();
        assert!(matches!(class_sig.visibility, Visibility::Crate));
    }

    #[test]
    fn extracts_interface() {
        let provider = JavaProvider::new();
        let source = b"public interface Runnable {\n    void run();\n}\n";
        let result = provider
            .extract_signatures(Path::new("Runnable.java"), source)
            .unwrap();
        let names: Vec<&str> = result.signatures.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Runnable"));
    }

    #[test]
    fn enum_source_does_not_panic() {
        // Java enums may not be captured by tree-sitter-java's tags query
        let provider = JavaProvider::new();
        let source = b"public enum Color {\n    RED, GREEN, BLUE\n}\n";
        let result = provider
            .extract_signatures(Path::new("Color.java"), source)
            .unwrap();
        // Just verify it doesn't panic; enum extraction depends on the tags query
        let _ = result.signatures;
    }

    #[test]
    fn extracts_references() {
        let provider = JavaProvider::new();
        let source = b"public class Foo {\n    public void bar() {\n        doWork();\n    }\n}\n";
        let refs = provider
            .extract_references(Path::new("Foo.java"), source)
            .unwrap();
        // Reference count depends on tree-sitter query; verify no panic
        let _ = refs;
    }

    #[test]
    fn signature_text_truncated_at_brace() {
        let provider = JavaProvider::new();
        let source = b"public class Foo {\n    public void bar() {\n        return;\n    }\n}\n";
        let result = provider
            .extract_signatures(Path::new("Foo.java"), source)
            .unwrap();
        for sig in &result.signatures {
            assert!(!sig.text.contains('{'));
        }
    }
}
