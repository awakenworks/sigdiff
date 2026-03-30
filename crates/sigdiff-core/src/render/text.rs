use crate::{FileDiff, FileSignatures, diff::SignatureChange, refs::FileRefs};

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";

pub fn render_map(files: &[FileSignatures], color: bool) -> String {
    let mut out = String::new();
    for file in files {
        if file.signatures.is_empty() {
            continue;
        }
        if color {
            out.push_str(&format!("{CYAN}{}:{RESET}\n", file.path.display()));
        } else {
            out.push_str(&format!("{}:\n", file.path.display()));
        }
        for sig in &file.signatures {
            out.push_str(&format!("    {}\n", sig.text));
        }
        out.push('\n');
    }
    out
}

pub fn render_diff(diffs: &[FileDiff], color: bool) -> String {
    let mut out = String::new();
    for diff in diffs {
        if diff.changes.is_empty() {
            continue;
        }
        if color {
            out.push_str(&format!("{CYAN}{}:{RESET}\n", diff.path.display()));
        } else {
            out.push_str(&format!("{}:\n", diff.path.display()));
        }
        for change in &diff.changes {
            match change {
                SignatureChange::Added(s) => {
                    if color {
                        out.push_str(&format!("{GREEN}+   {}{RESET}\n", s.text));
                    } else {
                        out.push_str(&format!("+   {}\n", s.text));
                    }
                }
                SignatureChange::Removed(s) => {
                    if color {
                        out.push_str(&format!("{RED}-   {}{RESET}\n", s.text));
                    } else {
                        out.push_str(&format!("-   {}\n", s.text));
                    }
                }
                SignatureChange::Modified { old, new } => {
                    if color {
                        out.push_str(&format!("{YELLOW}~   {}{RESET}\n", old.text));
                        out.push_str(&format!("    {DIM}→{RESET}  {GREEN}{}{RESET}\n", new.text));
                    } else {
                        out.push_str(&format!("~   {}\n", old.text));
                        out.push_str(&format!("    →  {}\n", new.text));
                    }
                }
            }
        }
        out.push('\n');
    }
    out
}

pub fn render_refs(file_refs: &FileRefs, color: bool) -> String {
    let mut out = String::new();
    if color {
        out.push_str(&format!("{CYAN}{}:{RESET}\n", file_refs.path.display()));
    } else {
        out.push_str(&format!("{}:\n", file_refs.path.display()));
    }
    for sig in &file_refs.signatures {
        out.push_str(&format!("    {}\n", sig.text));
    }
    if !file_refs.uses.is_empty() {
        if color {
            out.push_str(&format!("    {DIM}→ uses:{RESET}"));
        } else {
            out.push_str("    → uses:");
        }
        for u in &file_refs.uses {
            out.push_str(&format!(" {} ({})", u.identifier, u.file.display()));
        }
        out.push('\n');
    }
    if !file_refs.used_by.is_empty() {
        if color {
            out.push_str(&format!("    {DIM}← used by:{RESET}"));
        } else {
            out.push_str("    ← used by:");
        }
        for u in &file_refs.used_by {
            out.push_str(&format!(" {} ({})", u.identifier, u.file.display()));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileDiff, FileSignatures, Signature, SignatureChange, SignatureKind, Visibility};

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
    fn renders_map() {
        let sigs = vec![FileSignatures {
            path: "src/main.rs".into(),
            language: "rust".into(),
            signatures: vec![sig("main", "fn main()")],
        }];
        let output = render_map(&sigs, false);
        assert!(output.contains("src/main.rs:"));
        assert!(output.contains("fn main()"));
    }

    #[test]
    fn renders_diff() {
        let diffs = vec![FileDiff {
            path: "t.rs".into(),
            changes: vec![
                SignatureChange::Added(sig("hello", "pub fn hello()")),
                SignatureChange::Removed(sig("bye", "pub fn bye()")),
            ],
        }];
        let output = render_diff(&diffs, false);
        assert!(output.contains("+   pub fn hello()"));
        assert!(output.contains("-   pub fn bye()"));
    }

    #[test]
    fn renders_diff_modified() {
        let diffs = vec![FileDiff {
            path: "t.rs".into(),
            changes: vec![SignatureChange::Modified {
                old: sig("update", "fn update()"),
                new: sig("update", "fn update(x: i32)"),
            }],
        }];
        let output = render_diff(&diffs, false);
        assert!(output.contains("~   fn update()"));
        assert!(output.contains("→  fn update(x: i32)"));
    }

    #[test]
    fn renders_diff_with_color() {
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
        let output = render_diff(&diffs, true);
        assert!(output.contains(GREEN));
        assert!(output.contains(RED));
        assert!(output.contains(YELLOW));
        assert!(output.contains(RESET));
    }

    #[test]
    fn renders_map_with_color() {
        let sigs = vec![FileSignatures {
            path: "src/main.rs".into(),
            language: "rust".into(),
            signatures: vec![sig("main", "fn main()")],
        }];
        let output = render_map(&sigs, true);
        assert!(output.contains(CYAN));
        assert!(output.contains(RESET));
    }

    #[test]
    fn renders_map_skips_empty_files() {
        let sigs = vec![
            FileSignatures {
                path: "src/empty.rs".into(),
                language: "rust".into(),
                signatures: vec![],
            },
            FileSignatures {
                path: "src/main.rs".into(),
                language: "rust".into(),
                signatures: vec![sig("main", "fn main()")],
            },
        ];
        let output = render_map(&sigs, false);
        assert!(!output.contains("empty.rs"));
        assert!(output.contains("src/main.rs:"));
    }

    #[test]
    fn renders_diff_skips_empty_changes() {
        let diffs = vec![
            FileDiff {
                path: "empty.rs".into(),
                changes: vec![],
            },
            FileDiff {
                path: "t.rs".into(),
                changes: vec![SignatureChange::Added(sig("hello", "pub fn hello()"))],
            },
        ];
        let output = render_diff(&diffs, false);
        assert!(!output.contains("empty.rs"));
        assert!(output.contains("t.rs:"));
    }

    #[test]
    fn renders_refs() {
        use crate::refs::{FileRefs, RefLink};
        let file_refs = FileRefs {
            path: "a.rs".into(),
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
        let output = render_refs(&file_refs, false);
        assert!(output.contains("a.rs:"));
        assert!(output.contains("pub fn hello()"));
        assert!(output.contains("→ uses:"));
        assert!(output.contains("world (b.rs)"));
        assert!(output.contains("← used by:"));
        assert!(output.contains("hello (c.rs)"));
    }

    #[test]
    fn renders_refs_with_color() {
        use crate::refs::{FileRefs, RefLink};
        let file_refs = FileRefs {
            path: "a.rs".into(),
            signatures: vec![sig("hello", "pub fn hello()")],
            uses: vec![RefLink {
                identifier: "world".into(),
                file: "b.rs".into(),
                kind: SignatureKind::Function,
            }],
            used_by: vec![],
        };
        let output = render_refs(&file_refs, true);
        assert!(output.contains(CYAN));
        assert!(output.contains(DIM));
    }

    #[test]
    fn renders_refs_no_uses_or_used_by() {
        use crate::refs::FileRefs;
        let file_refs = FileRefs {
            path: "a.rs".into(),
            signatures: vec![sig("hello", "pub fn hello()")],
            uses: vec![],
            used_by: vec![],
        };
        let output = render_refs(&file_refs, false);
        assert!(output.contains("a.rs:"));
        assert!(!output.contains("→ uses:"));
        assert!(!output.contains("← used by:"));
    }
}
