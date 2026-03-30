use crate::{FileSignatures, SignatureKind, Visibility};

pub struct MapFilter {
    pub lang: Option<Vec<String>>,
    pub public_only: bool,
    pub kinds: Option<Vec<SignatureKind>>,
    pub grep: Option<String>,
    pub max_depth: Option<usize>,
    pub path_prefix: Option<String>,
}

impl MapFilter {
    pub fn apply(&self, files: &[FileSignatures]) -> Vec<FileSignatures> {
        let mut result: Vec<FileSignatures> = files
            .iter()
            .filter(|f| self.file_matches(f))
            .map(|f| {
                let sigs: Vec<_> = f
                    .signatures
                    .iter()
                    .filter(|s| self.sig_matches(s))
                    .cloned()
                    .collect();
                FileSignatures {
                    path: f.path.clone(),
                    language: f.language.clone(),
                    signatures: sigs,
                }
            })
            .collect();

        // Remove files with no signatures after filtering
        result.retain(|f| !f.signatures.is_empty());
        result
    }

    fn file_matches(&self, f: &FileSignatures) -> bool {
        // Language filter
        if let Some(langs) = &self.lang {
            let lang_lower = f.language.to_lowercase();
            if !langs.iter().any(|l| l.to_lowercase() == lang_lower) {
                return false;
            }
        }

        // Path prefix filter
        if let Some(prefix) = &self.path_prefix {
            let path_str = f.path.to_string_lossy();
            if !path_str.starts_with(prefix.as_str()) {
                return false;
            }
        }

        // Max depth filter
        if let Some(max_depth) = self.max_depth {
            let depth = f.path.components().count().saturating_sub(1);
            if depth > max_depth {
                return false;
            }
        }

        true
    }

    fn sig_matches(&self, s: &crate::Signature) -> bool {
        // Visibility filter
        if self.public_only && s.visibility != Visibility::Public {
            return false;
        }

        // Kind filter (struct and class are treated as equivalent)
        if let Some(kinds) = &self.kinds {
            let matches = kinds.iter().any(|k| {
                *k == s.kind
                    || (*k == SignatureKind::Struct && s.kind == SignatureKind::Class)
                    || (*k == SignatureKind::Class && s.kind == SignatureKind::Struct)
            });
            if !matches {
                return false;
            }
        }

        // Grep filter
        if let Some(ref pattern) = self.grep
            && !s.name.to_lowercase().contains(&pattern.to_lowercase())
        {
            return false;
        }

        true
    }
}

pub fn parse_kind(s: &str) -> Option<SignatureKind> {
    match s.to_lowercase().as_str() {
        "function" | "fn" => Some(SignatureKind::Function),
        "method" => Some(SignatureKind::Method),
        "struct" => Some(SignatureKind::Struct),
        "enum" => Some(SignatureKind::Enum),
        "trait" => Some(SignatureKind::Trait),
        "impl" => Some(SignatureKind::Impl),
        "const" => Some(SignatureKind::Const),
        "type-alias" | "type_alias" | "typealias" => Some(SignatureKind::TypeAlias),
        "module" | "mod" => Some(SignatureKind::Module),
        "interface" => Some(SignatureKind::Interface),
        "class" => Some(SignatureKind::Class),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileSignatures, Signature, SignatureKind, Visibility};
    use std::path::PathBuf;

    fn make_sig(name: &str, kind: SignatureKind, visibility: Visibility) -> Signature {
        Signature {
            file: PathBuf::from("test.rs"),
            name: name.to_string(),
            kind,
            visibility,
            text: format!("fn {name}()"),
            line: 1,
            parent: None,
        }
    }

    fn make_file(path: &str, lang: &str, sigs: Vec<Signature>) -> FileSignatures {
        FileSignatures {
            path: PathBuf::from(path),
            language: lang.to_string(),
            signatures: sigs,
        }
    }

    fn no_filter() -> MapFilter {
        MapFilter {
            lang: None,
            public_only: false,
            kinds: None,
            grep: None,
            max_depth: None,
            path_prefix: None,
        }
    }

    #[test]
    fn test_no_filter_passes_all() {
        let files = vec![
            make_file(
                "src/main.rs",
                "rust",
                vec![make_sig(
                    "main",
                    SignatureKind::Function,
                    Visibility::Public,
                )],
            ),
            make_file(
                "src/lib.rs",
                "rust",
                vec![make_sig(
                    "helper",
                    SignatureKind::Function,
                    Visibility::Private,
                )],
            ),
        ];
        let filter = no_filter();
        let result = filter.apply(&files);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_lang_filter() {
        let files = vec![
            make_file(
                "src/main.rs",
                "rust",
                vec![make_sig(
                    "main",
                    SignatureKind::Function,
                    Visibility::Public,
                )],
            ),
            make_file(
                "src/app.py",
                "python",
                vec![make_sig("run", SignatureKind::Function, Visibility::Public)],
            ),
        ];
        let filter = MapFilter {
            lang: Some(vec!["rust".to_string()]),
            ..no_filter()
        };
        let result = filter.apply(&files);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].language, "rust");
    }

    #[test]
    fn test_lang_filter_multi() {
        let files = vec![
            make_file(
                "src/main.rs",
                "rust",
                vec![make_sig(
                    "main",
                    SignatureKind::Function,
                    Visibility::Public,
                )],
            ),
            make_file(
                "src/app.py",
                "python",
                vec![make_sig("run", SignatureKind::Function, Visibility::Public)],
            ),
            make_file(
                "src/app.ts",
                "typescript",
                vec![make_sig(
                    "start",
                    SignatureKind::Function,
                    Visibility::Public,
                )],
            ),
        ];
        let filter = MapFilter {
            lang: Some(vec!["rust".to_string(), "python".to_string()]),
            ..no_filter()
        };
        let result = filter.apply(&files);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_public_only_filter() {
        let files = vec![make_file(
            "src/lib.rs",
            "rust",
            vec![
                make_sig("public_fn", SignatureKind::Function, Visibility::Public),
                make_sig("private_fn", SignatureKind::Function, Visibility::Private),
                make_sig("crate_fn", SignatureKind::Function, Visibility::Crate),
            ],
        )];
        let filter = MapFilter {
            public_only: true,
            ..no_filter()
        };
        let result = filter.apply(&files);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].signatures.len(), 1);
        assert_eq!(result[0].signatures[0].name, "public_fn");
    }

    #[test]
    fn test_public_only_removes_empty_files() {
        let files = vec![make_file(
            "src/internal.rs",
            "rust",
            vec![make_sig(
                "private_fn",
                SignatureKind::Function,
                Visibility::Private,
            )],
        )];
        let filter = MapFilter {
            public_only: true,
            ..no_filter()
        };
        let result = filter.apply(&files);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_kind_filter() {
        let files = vec![make_file(
            "src/lib.rs",
            "rust",
            vec![
                make_sig("MyStruct", SignatureKind::Struct, Visibility::Public),
                make_sig("MyTrait", SignatureKind::Trait, Visibility::Public),
                make_sig("my_fn", SignatureKind::Function, Visibility::Public),
                make_sig("MyEnum", SignatureKind::Enum, Visibility::Public),
            ],
        )];
        let filter = MapFilter {
            kinds: Some(vec![SignatureKind::Struct, SignatureKind::Trait]),
            ..no_filter()
        };
        let result = filter.apply(&files);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].signatures.len(), 2);
        assert!(
            result[0]
                .signatures
                .iter()
                .all(|s| s.kind == SignatureKind::Struct || s.kind == SignatureKind::Trait)
        );
    }

    #[test]
    fn test_grep_filter_case_insensitive() {
        let files = vec![make_file(
            "src/lib.rs",
            "rust",
            vec![
                make_sig("UserProvider", SignatureKind::Struct, Visibility::Public),
                make_sig("user_helper", SignatureKind::Function, Visibility::Public),
                make_sig("OtherThing", SignatureKind::Struct, Visibility::Public),
            ],
        )];
        let filter = MapFilter {
            grep: Some("user".to_string()),
            ..no_filter()
        };
        let result = filter.apply(&files);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].signatures.len(), 2);
    }

    #[test]
    fn test_max_depth_filter() {
        let files = vec![
            make_file(
                "main.rs",
                "rust",
                vec![make_sig(
                    "main",
                    SignatureKind::Function,
                    Visibility::Public,
                )],
            ),
            make_file(
                "src/lib.rs",
                "rust",
                vec![make_sig("lib", SignatureKind::Function, Visibility::Public)],
            ),
            make_file(
                "src/sub/deep.rs",
                "rust",
                vec![make_sig(
                    "deep",
                    SignatureKind::Function,
                    Visibility::Public,
                )],
            ),
        ];
        let filter = MapFilter {
            max_depth: Some(1),
            ..no_filter()
        };
        let result = filter.apply(&files);
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|f| f.path.to_str() == Some("main.rs")));
        assert!(result.iter().any(|f| f.path.to_str() == Some("src/lib.rs")));
    }

    #[test]
    fn test_path_prefix_filter() {
        let files = vec![
            make_file(
                "src/main.rs",
                "rust",
                vec![make_sig(
                    "main",
                    SignatureKind::Function,
                    Visibility::Public,
                )],
            ),
            make_file(
                "crates/core/src/lib.rs",
                "rust",
                vec![make_sig("lib", SignatureKind::Function, Visibility::Public)],
            ),
            make_file(
                "tests/test.rs",
                "rust",
                vec![make_sig(
                    "test",
                    SignatureKind::Function,
                    Visibility::Public,
                )],
            ),
        ];
        let filter = MapFilter {
            path_prefix: Some("crates/".to_string()),
            ..no_filter()
        };
        let result = filter.apply(&files);
        assert_eq!(result.len(), 1);
        assert!(result[0].path.to_str().unwrap().starts_with("crates/"));
    }

    #[test]
    fn test_combined_filters() {
        let files = vec![
            make_file(
                "src/lib.rs",
                "rust",
                vec![
                    make_sig("PublicUser", SignatureKind::Struct, Visibility::Public),
                    make_sig("PrivateUser", SignatureKind::Struct, Visibility::Private),
                    make_sig("PublicHelper", SignatureKind::Function, Visibility::Public),
                ],
            ),
            make_file(
                "src/app.py",
                "python",
                vec![make_sig("User", SignatureKind::Class, Visibility::Public)],
            ),
        ];
        let filter = MapFilter {
            lang: Some(vec!["rust".to_string()]),
            public_only: true,
            kinds: Some(vec![SignatureKind::Struct]),
            grep: Some("User".to_string()),
            ..no_filter()
        };
        let result = filter.apply(&files);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].signatures.len(), 1);
        assert_eq!(result[0].signatures[0].name, "PublicUser");
    }

    #[test]
    fn test_kind_filter_struct_class_equivalence() {
        // Filtering by Struct should also match Class (and vice versa)
        let files = vec![make_file(
            "src/lib.rs",
            "rust",
            vec![
                make_sig("MyStruct", SignatureKind::Struct, Visibility::Public),
                make_sig("MyClass", SignatureKind::Class, Visibility::Public),
                make_sig("my_fn", SignatureKind::Function, Visibility::Public),
            ],
        )];

        // Filter by Struct should match both Struct and Class
        let filter = MapFilter {
            kinds: Some(vec![SignatureKind::Struct]),
            ..no_filter()
        };
        let result = filter.apply(&files);
        assert_eq!(result[0].signatures.len(), 2);

        // Filter by Class should match both Struct and Class
        let filter = MapFilter {
            kinds: Some(vec![SignatureKind::Class]),
            ..no_filter()
        };
        let result = filter.apply(&files);
        assert_eq!(result[0].signatures.len(), 2);
    }

    #[test]
    fn test_all_filtered_out_returns_empty() {
        let files = vec![make_file(
            "src/lib.rs",
            "rust",
            vec![make_sig(
                "private_fn",
                SignatureKind::Function,
                Visibility::Private,
            )],
        )];
        let filter = MapFilter {
            public_only: true,
            ..no_filter()
        };
        let result = filter.apply(&files);
        assert!(result.is_empty());
    }

    #[test]
    fn test_lang_filter_case_insensitive() {
        let files = vec![make_file(
            "src/main.rs",
            "Rust",
            vec![make_sig(
                "main",
                SignatureKind::Function,
                Visibility::Public,
            )],
        )];
        let filter = MapFilter {
            lang: Some(vec!["rust".to_string()]),
            ..no_filter()
        };
        let result = filter.apply(&files);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_grep_no_match() {
        let files = vec![make_file(
            "src/lib.rs",
            "rust",
            vec![make_sig(
                "hello",
                SignatureKind::Function,
                Visibility::Public,
            )],
        )];
        let filter = MapFilter {
            grep: Some("nonexistent".to_string()),
            ..no_filter()
        };
        let result = filter.apply(&files);
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_kind() {
        assert_eq!(parse_kind("function"), Some(SignatureKind::Function));
        assert_eq!(parse_kind("fn"), Some(SignatureKind::Function));
        assert_eq!(parse_kind("method"), Some(SignatureKind::Method));
        assert_eq!(parse_kind("struct"), Some(SignatureKind::Struct));
        assert_eq!(parse_kind("enum"), Some(SignatureKind::Enum));
        assert_eq!(parse_kind("trait"), Some(SignatureKind::Trait));
        assert_eq!(parse_kind("impl"), Some(SignatureKind::Impl));
        assert_eq!(parse_kind("const"), Some(SignatureKind::Const));
        assert_eq!(parse_kind("type-alias"), Some(SignatureKind::TypeAlias));
        assert_eq!(parse_kind("type_alias"), Some(SignatureKind::TypeAlias));
        assert_eq!(parse_kind("typealias"), Some(SignatureKind::TypeAlias));
        assert_eq!(parse_kind("module"), Some(SignatureKind::Module));
        assert_eq!(parse_kind("mod"), Some(SignatureKind::Module));
        assert_eq!(parse_kind("interface"), Some(SignatureKind::Interface));
        assert_eq!(parse_kind("class"), Some(SignatureKind::Class));
        assert_eq!(parse_kind("unknown"), None);
    }
}
