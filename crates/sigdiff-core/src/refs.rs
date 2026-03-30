use crate::{Reference, Signature, SignatureKind};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct RefLink {
    pub identifier: String,
    pub file: PathBuf,
    pub kind: SignatureKind,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileRefs {
    pub path: PathBuf,
    pub signatures: Vec<Signature>,
    pub uses: Vec<RefLink>,
    pub used_by: Vec<RefLink>,
}

pub fn resolve_refs(
    target: &Path,
    all_signatures: &[Signature],
    all_references: &[Reference],
) -> FileRefs {
    let mut def_index: HashMap<&str, Vec<(&Path, &SignatureKind)>> = HashMap::new();
    for sig in all_signatures {
        def_index
            .entry(&sig.name)
            .or_default()
            .push((&sig.file, &sig.kind));
    }

    let mut uses = Vec::new();
    for r in all_references {
        if r.file != target {
            continue;
        }
        if let Some(defs) = def_index.get(r.name.as_str()) {
            for (def_file, kind) in defs {
                if *def_file != target {
                    uses.push(RefLink {
                        identifier: r.name.clone(),
                        file: def_file.to_path_buf(),
                        kind: (*kind).clone(),
                    });
                }
            }
        }
    }

    let target_defs: Vec<&str> = all_signatures
        .iter()
        .filter(|s| s.file == target)
        .map(|s| s.name.as_str())
        .collect();
    let mut used_by = Vec::new();
    for r in all_references {
        if r.file == target {
            continue;
        }
        if target_defs.contains(&r.name.as_str())
            && let Some(defs) = def_index.get(r.name.as_str())
        {
            for (def_file, kind) in defs {
                if *def_file == target {
                    used_by.push(RefLink {
                        identifier: r.name.clone(),
                        file: r.file.clone(),
                        kind: (*kind).clone(),
                    });
                }
            }
        }
    }

    uses.sort_by(|a, b| (&a.file, &a.identifier).cmp(&(&b.file, &b.identifier)));
    uses.dedup_by(|a, b| a.file == b.file && a.identifier == b.identifier);
    used_by.sort_by(|a, b| (&a.file, &a.identifier).cmp(&(&b.file, &b.identifier)));
    used_by.dedup_by(|a, b| a.file == b.file && a.identifier == b.identifier);

    let signatures = all_signatures
        .iter()
        .filter(|s| s.file == target)
        .cloned()
        .collect();
    FileRefs {
        path: target.to_path_buf(),
        signatures,
        uses,
        used_by,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Reference, Signature, SignatureKind, Visibility};

    fn sig(file: &str, name: &str) -> Signature {
        Signature {
            file: file.into(),
            name: name.into(),
            kind: SignatureKind::Function,
            visibility: Visibility::Public,
            text: format!("pub fn {name}()"),
            line: 1,
            parent: None,
        }
    }

    fn reference(file: &str, name: &str) -> Reference {
        Reference {
            file: file.into(),
            name: name.into(),
            line: 1,
        }
    }

    #[test]
    fn resolves_uses_and_used_by() {
        let sigs = vec![sig("a.rs", "hello"), sig("b.rs", "world")];
        let refs = vec![reference("a.rs", "world")];
        let result = resolve_refs(Path::new("a.rs"), &sigs, &refs);
        assert!(
            result
                .uses
                .iter()
                .any(|r| r.identifier == "world" && r.file == PathBuf::from("b.rs"))
        );

        let result_b = resolve_refs(Path::new("b.rs"), &sigs, &refs);
        assert!(
            result_b
                .used_by
                .iter()
                .any(|r| r.identifier == "world" && r.file == PathBuf::from("a.rs"))
        );
    }

    #[test]
    fn no_refs_returns_empty_uses_and_used_by() {
        let sigs = vec![sig("a.rs", "hello")];
        let refs = vec![];
        let result = resolve_refs(Path::new("a.rs"), &sigs, &refs);
        assert!(result.uses.is_empty());
        assert!(result.used_by.is_empty());
        assert_eq!(result.signatures.len(), 1);
    }

    #[test]
    fn self_references_excluded() {
        // A reference within the same file to its own definition should not appear
        let sigs = vec![sig("a.rs", "hello"), sig("a.rs", "world")];
        let refs = vec![reference("a.rs", "hello")]; // a.rs references its own "hello"
        let result = resolve_refs(Path::new("a.rs"), &sigs, &refs);
        // "hello" is defined in a.rs and referenced from a.rs — self-ref excluded from uses
        assert!(result.uses.is_empty());
    }

    #[test]
    fn deduplicates_refs() {
        let sigs = vec![sig("a.rs", "hello"), sig("b.rs", "world")];
        // Same reference appearing conceptually (from same file to same def)
        let refs = vec![reference("a.rs", "world"), reference("a.rs", "world")];
        let result = resolve_refs(Path::new("a.rs"), &sigs, &refs);
        // Should be deduplicated
        assert_eq!(result.uses.len(), 1);
    }

    #[test]
    fn empty_inputs() {
        let result = resolve_refs(Path::new("a.rs"), &[], &[]);
        assert!(result.signatures.is_empty());
        assert!(result.uses.is_empty());
        assert!(result.used_by.is_empty());
    }

    #[test]
    fn target_signatures_collected() {
        let sigs = vec![
            sig("a.rs", "hello"),
            sig("a.rs", "world"),
            sig("b.rs", "other"),
        ];
        let result = resolve_refs(Path::new("a.rs"), &sigs, &[]);
        assert_eq!(result.signatures.len(), 2);
        assert!(
            result
                .signatures
                .iter()
                .all(|s| s.file == PathBuf::from("a.rs"))
        );
    }

    #[test]
    fn multiple_files_use_target() {
        let sigs = vec![
            sig("a.rs", "hello"),
            sig("b.rs", "other"),
            sig("c.rs", "third"),
        ];
        let refs = vec![reference("b.rs", "hello"), reference("c.rs", "hello")];
        let result = resolve_refs(Path::new("a.rs"), &sigs, &refs);
        assert_eq!(result.used_by.len(), 2);
    }
}
