use crate::{FileDiff, FileSignatures, refs::FileRefs};

pub fn render_map_json(files: &[FileSignatures]) -> crate::Result<String> {
    serde_json::to_string_pretty(files).map_err(|e| crate::Error::Other(e.to_string()))
}

pub fn render_diff_json(diffs: &[FileDiff]) -> crate::Result<String> {
    serde_json::to_string_pretty(diffs).map_err(|e| crate::Error::Other(e.to_string()))
}

pub fn render_refs_json(refs: &FileRefs) -> crate::Result<String> {
    serde_json::to_string_pretty(refs).map_err(|e| crate::Error::Other(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FileDiff, FileSignatures, Signature, SignatureChange, SignatureKind, Visibility,
        refs::RefLink,
    };
    use std::path::PathBuf;

    fn sig(name: &str, text: &str) -> Signature {
        Signature {
            file: "t.rs".into(),
            name: name.into(),
            kind: SignatureKind::Function,
            visibility: Visibility::Public,
            text: text.into(),
            line: 1,
            parent: None,
        }
    }

    #[test]
    fn render_map_json_produces_valid_json() {
        let files = vec![FileSignatures {
            path: "src/main.rs".into(),
            language: "rust".into(),
            signatures: vec![sig("main", "fn main()")],
        }];
        let json = render_map_json(&files).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed[0]["path"], "src/main.rs");
        assert_eq!(parsed[0]["signatures"][0]["name"], "main");
    }

    #[test]
    fn render_map_json_empty_input() {
        let json = render_map_json(&[]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 0);
    }

    #[test]
    fn render_diff_json_produces_valid_json() {
        let diffs = vec![FileDiff {
            path: "t.rs".into(),
            changes: vec![
                SignatureChange::Added(sig("hello", "pub fn hello()")),
                SignatureChange::Removed(sig("bye", "pub fn bye()")),
                SignatureChange::Modified {
                    old: sig("update", "fn update()"),
                    new: sig("update", "fn update(x: i32)"),
                },
            ],
        }];
        let json = render_diff_json(&diffs).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array());
        let changes = parsed[0]["changes"].as_array().unwrap();
        assert_eq!(changes.len(), 3);
        assert_eq!(changes[0]["type"], "added");
        assert_eq!(changes[1]["type"], "removed");
        assert_eq!(changes[2]["type"], "modified");
    }

    #[test]
    fn render_diff_json_empty_input() {
        let json = render_diff_json(&[]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 0);
    }

    #[test]
    fn render_refs_json_produces_valid_json() {
        let file_refs = FileRefs {
            path: PathBuf::from("a.rs"),
            signatures: vec![sig("hello", "pub fn hello()")],
            uses: vec![RefLink {
                identifier: "world".into(),
                file: "b.rs".into(),
                kind: SignatureKind::Function,
            }],
            used_by: vec![RefLink {
                identifier: "hello".into(),
                file: "c.rs".into(),
                kind: SignatureKind::Function,
            }],
        };
        let json = render_refs_json(&file_refs).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["path"], "a.rs");
        assert_eq!(parsed["uses"][0]["identifier"], "world");
        assert_eq!(parsed["used_by"][0]["identifier"], "hello");
    }

    #[test]
    fn render_refs_json_empty_refs() {
        let file_refs = FileRefs {
            path: PathBuf::from("a.rs"),
            signatures: vec![],
            uses: vec![],
            used_by: vec![],
        };
        let json = render_refs_json(&file_refs).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["uses"].as_array().unwrap().len(), 0);
        assert_eq!(parsed["used_by"].as_array().unwrap().len(), 0);
    }
}
