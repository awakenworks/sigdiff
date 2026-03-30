use crate::{FileSignatures, Signature};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum SignatureChange {
    #[serde(rename = "added")]
    Added(Signature),
    #[serde(rename = "removed")]
    Removed(Signature),
    #[serde(rename = "modified")]
    Modified { old: Signature, new: Signature },
}

#[derive(Debug, Clone, Serialize)]
pub struct FileDiff {
    pub path: PathBuf,
    pub changes: Vec<SignatureChange>,
}

type MatchKey = (PathBuf, String, String, Option<String>);

fn match_key(s: &Signature) -> MatchKey {
    (
        s.file.clone(),
        s.name.clone(),
        format!("{:?}", s.kind),
        s.parent.clone(),
    )
}

pub fn diff_signatures(old: &[Signature], new: &[Signature]) -> Vec<SignatureChange> {
    let old_map: HashMap<MatchKey, &Signature> = old.iter().map(|s| (match_key(s), s)).collect();
    let new_map: HashMap<MatchKey, &Signature> = new.iter().map(|s| (match_key(s), s)).collect();

    let mut changes = Vec::new();
    for s in new {
        let key = match_key(s);
        match old_map.get(&key) {
            None => changes.push(SignatureChange::Added(s.clone())),
            Some(old_sig) if old_sig.text != s.text => {
                changes.push(SignatureChange::Modified {
                    old: (*old_sig).clone(),
                    new: s.clone(),
                });
            }
            _ => {}
        }
    }
    for s in old {
        if !new_map.contains_key(&match_key(s)) {
            changes.push(SignatureChange::Removed(s.clone()));
        }
    }
    changes
}

pub fn diff_file_signatures(
    old_files: &[FileSignatures],
    new_files: &[FileSignatures],
) -> Vec<FileDiff> {
    let all_old: Vec<Signature> = old_files
        .iter()
        .flat_map(|f| f.signatures.clone())
        .collect();
    let all_new: Vec<Signature> = new_files
        .iter()
        .flat_map(|f| f.signatures.clone())
        .collect();
    let changes = diff_signatures(&all_old, &all_new);

    let mut by_file: HashMap<PathBuf, Vec<SignatureChange>> = HashMap::new();
    for change in changes {
        let path = match &change {
            SignatureChange::Added(s) | SignatureChange::Removed(s) => s.file.clone(),
            SignatureChange::Modified { new, .. } => new.file.clone(),
        };
        by_file.entry(path).or_default().push(change);
    }
    let mut diffs: Vec<FileDiff> = by_file
        .into_iter()
        .map(|(path, changes)| FileDiff { path, changes })
        .collect();
    diffs.sort_by(|a, b| a.path.cmp(&b.path));
    diffs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Signature, SignatureKind, Visibility};

    fn sig(name: &str, text: &str) -> Signature {
        Signature {
            file: PathBuf::from("t.rs"),
            name: name.into(),
            kind: SignatureKind::Function,
            visibility: Visibility::Public,
            text: text.into(),
            line: 1,
            parent: None,
        }
    }

    #[test]
    fn detects_added() {
        let d = diff_signatures(&[], &[sig("hello", "pub fn hello()")]);
        assert_eq!(d.len(), 1);
        assert!(matches!(&d[0], SignatureChange::Added(s) if s.name == "hello"));
    }

    #[test]
    fn detects_removed() {
        let d = diff_signatures(&[sig("hello", "pub fn hello()")], &[]);
        assert_eq!(d.len(), 1);
        assert!(matches!(&d[0], SignatureChange::Removed(s) if s.name == "hello"));
    }

    #[test]
    fn detects_modified() {
        let d = diff_signatures(
            &[sig("hello", "pub fn hello()")],
            &[sig("hello", "pub fn hello(name: &str)")],
        );
        assert_eq!(d.len(), 1);
        assert!(matches!(&d[0], SignatureChange::Modified { .. }));
    }

    #[test]
    fn unchanged_not_in_diff() {
        let d = diff_signatures(
            &[sig("hello", "pub fn hello()")],
            &[sig("hello", "pub fn hello()")],
        );
        assert!(d.is_empty());
    }

    #[test]
    fn empty_both_sides() {
        let d = diff_signatures(&[], &[]);
        assert!(d.is_empty());
    }

    #[test]
    fn multiple_changes_at_once() {
        let old = vec![
            sig("kept", "fn kept()"),
            sig("removed", "fn removed()"),
            sig("changed", "fn changed()"),
        ];
        let new = vec![
            sig("kept", "fn kept()"),
            sig("added", "fn added()"),
            sig("changed", "fn changed(x: i32)"),
        ];
        let d = diff_signatures(&old, &new);
        assert_eq!(d.len(), 3);
        assert!(
            d.iter()
                .any(|c| matches!(c, SignatureChange::Added(s) if s.name == "added"))
        );
        assert!(
            d.iter()
                .any(|c| matches!(c, SignatureChange::Removed(s) if s.name == "removed"))
        );
        assert!(
            d.iter().any(
                |c| matches!(c, SignatureChange::Modified { old, .. } if old.name == "changed")
            )
        );
    }

    #[test]
    fn diff_file_signatures_groups_by_file() {
        let old_files = vec![FileSignatures {
            path: "a.rs".into(),
            language: "rust".into(),
            signatures: vec![Signature {
                file: "a.rs".into(),
                name: "hello".into(),
                kind: SignatureKind::Function,
                visibility: Visibility::Public,
                text: "fn hello()".into(),
                line: 1,
                parent: None,
            }],
        }];
        let new_files = vec![
            FileSignatures {
                path: "a.rs".into(),
                language: "rust".into(),
                signatures: vec![Signature {
                    file: "a.rs".into(),
                    name: "hello".into(),
                    kind: SignatureKind::Function,
                    visibility: Visibility::Public,
                    text: "fn hello(name: &str)".into(),
                    line: 1,
                    parent: None,
                }],
            },
            FileSignatures {
                path: "b.rs".into(),
                language: "rust".into(),
                signatures: vec![Signature {
                    file: "b.rs".into(),
                    name: "world".into(),
                    kind: SignatureKind::Function,
                    visibility: Visibility::Public,
                    text: "fn world()".into(),
                    line: 1,
                    parent: None,
                }],
            },
        ];
        let diffs = diff_file_signatures(&old_files, &new_files);
        // Should have changes in both files
        assert_eq!(diffs.len(), 2);
        // Should be sorted by path
        assert_eq!(diffs[0].path, PathBuf::from("a.rs"));
        assert_eq!(diffs[1].path, PathBuf::from("b.rs"));
        // a.rs should have a Modified change
        assert!(
            diffs[0]
                .changes
                .iter()
                .any(|c| matches!(c, SignatureChange::Modified { .. }))
        );
        // b.rs should have an Added change
        assert!(
            diffs[1]
                .changes
                .iter()
                .any(|c| matches!(c, SignatureChange::Added(_)))
        );
    }

    #[test]
    fn diff_file_signatures_empty() {
        let diffs = diff_file_signatures(&[], &[]);
        assert!(diffs.is_empty());
    }
}
