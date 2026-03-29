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
        assert!(output.contains("+"));
        assert!(output.contains("-"));
    }
}
