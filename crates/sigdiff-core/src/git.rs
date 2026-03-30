use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

pub fn repo_root(path: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path)
        .output()?;
    if !output.status.success() {
        return Err(Error::Git("not a git repository".into()));
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim(),
    ))
}

pub fn list_files(repo_path: &Path) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["ls-files"])
        .current_dir(repo_path)
        .output()?;
    if !output.status.success() {
        return Err(Error::Git("git ls-files failed".into()));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| repo_path.join(l))
        .filter(|p| p.exists())
        .collect())
}

pub fn show_file(repo_path: &Path, commit: &str, file_path: &str) -> Result<Vec<u8>> {
    let spec = format!("{commit}:{file_path}");
    let output = Command::new("git")
        .args(["show", &spec])
        .current_dir(repo_path)
        .output()?;
    if !output.status.success() {
        return Err(Error::Git(format!("git show {spec} failed")));
    }
    Ok(output.stdout)
}

pub fn diff_names(repo_path: &Path, old: &str, new: &str) -> Result<Vec<(FileStatus, PathBuf)>> {
    let output = Command::new("git")
        .args(["diff", "--name-status", old, new])
        .current_dir(repo_path)
        .output()?;
    if !output.status.success() {
        return Err(Error::Git("git diff --name-status failed".into()));
    }
    parse_name_status(&String::from_utf8_lossy(&output.stdout))
}

/// Get changed files in the working tree (both staged and unstaged) relative to HEAD.
pub fn diff_worktree(repo_path: &Path) -> Result<Vec<(FileStatus, PathBuf)>> {
    // Unstaged changes: git diff --name-status
    let unstaged = Command::new("git")
        .args(["diff", "--name-status"])
        .current_dir(repo_path)
        .output()?;
    // Staged changes: git diff --name-status --cached
    let staged = Command::new("git")
        .args(["diff", "--name-status", "--cached"])
        .current_dir(repo_path)
        .output()?;

    let mut result = parse_name_status(&String::from_utf8_lossy(&unstaged.stdout))?;
    let staged_entries = parse_name_status(&String::from_utf8_lossy(&staged.stdout))?;

    // Merge staged entries, avoiding duplicates (unstaged takes precedence)
    let existing: std::collections::HashSet<PathBuf> =
        result.iter().map(|(_, p)| p.clone()).collect();
    for entry in staged_entries {
        if !existing.contains(&entry.1) {
            result.push(entry);
        }
    }
    Ok(result)
}

fn parse_name_status(output: &str) -> Result<Vec<(FileStatus, PathBuf)>> {
    let mut result = Vec::new();
    for line in output.lines() {
        let mut parts = line.split('\t');
        let status = match parts.next() {
            Some(s) if s.starts_with('A') => FileStatus::Added,
            Some(s) if s.starts_with('M') => FileStatus::Modified,
            Some(s) if s.starts_with('D') => FileStatus::Deleted,
            Some(s) if s.starts_with('R') => FileStatus::Renamed,
            _ => continue,
        };
        let path = if status == FileStatus::Renamed {
            parts.nth(1)
        } else {
            parts.next()
        };
        if let Some(p) = path {
            result.push((status, PathBuf::from(p)));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn setup_test_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        Command::new("git")
            .args(["init"])
            .current_dir(d)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "t@t.com"])
            .current_dir(d)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "T"])
            .current_dir(d)
            .output()
            .unwrap();
        std::fs::write(d.join("hello.rs"), "fn hello() {}").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(d)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(d)
            .output()
            .unwrap();
        dir
    }

    #[test]
    fn list_files_returns_tracked_files() {
        let dir = setup_test_repo();
        let files = list_files(dir.path()).unwrap();
        assert!(files.iter().any(|f| f.ends_with("hello.rs")));
    }

    #[test]
    fn show_file_returns_content() {
        let dir = setup_test_repo();
        let content = show_file(dir.path(), "HEAD", "hello.rs").unwrap();
        assert_eq!(content, b"fn hello() {}");
    }

    #[test]
    fn diff_names_detects_changes() {
        let dir = setup_test_repo();
        let d = dir.path();
        std::fs::write(d.join("hello.rs"), "fn hello() { greet(); }").unwrap();
        std::fs::write(d.join("world.rs"), "fn world() {}").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(d)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "change"])
            .current_dir(d)
            .output()
            .unwrap();
        let changes = diff_names(d, "HEAD~1", "HEAD").unwrap();
        assert!(
            changes
                .iter()
                .any(|(s, p)| p.ends_with("hello.rs") && *s == FileStatus::Modified)
        );
        assert!(
            changes
                .iter()
                .any(|(s, p)| p.ends_with("world.rs") && *s == FileStatus::Added)
        );
    }

    #[test]
    fn parse_name_status_added() {
        let result = parse_name_status("A\tnew_file.rs\n").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, FileStatus::Added);
        assert_eq!(result[0].1, PathBuf::from("new_file.rs"));
    }

    #[test]
    fn parse_name_status_modified() {
        let result = parse_name_status("M\texisting.rs\n").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, FileStatus::Modified);
    }

    #[test]
    fn parse_name_status_deleted() {
        let result = parse_name_status("D\tremoved.rs\n").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, FileStatus::Deleted);
    }

    #[test]
    fn parse_name_status_renamed() {
        // Git rename format: R100\told_name\tnew_name
        let result = parse_name_status("R100\told.rs\tnew.rs\n").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, FileStatus::Renamed);
        assert_eq!(result[0].1, PathBuf::from("new.rs"));
    }

    #[test]
    fn parse_name_status_multiple() {
        let input = "A\ta.rs\nM\tb.rs\nD\tc.rs\n";
        let result = parse_name_status(input).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, FileStatus::Added);
        assert_eq!(result[1].0, FileStatus::Modified);
        assert_eq!(result[2].0, FileStatus::Deleted);
    }

    #[test]
    fn parse_name_status_empty_input() {
        let result = parse_name_status("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_name_status_unknown_status_skipped() {
        let result = parse_name_status("X\tunknown.rs\n").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn repo_root_returns_correct_path() {
        let dir = setup_test_repo();
        let root = repo_root(dir.path()).unwrap();
        // Canonicalize both to handle symlinks (e.g., /tmp -> /private/tmp on macOS)
        let expected = std::fs::canonicalize(dir.path()).unwrap();
        let actual = std::fs::canonicalize(root).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn repo_root_fails_for_non_repo() {
        let dir = tempfile::tempdir().unwrap();
        let result = repo_root(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn diff_worktree_detects_unstaged_changes() {
        let dir = setup_test_repo();
        let d = dir.path();
        // Modify a tracked file without staging
        std::fs::write(d.join("hello.rs"), "fn hello() { changed(); }").unwrap();
        let changes = diff_worktree(d).unwrap();
        assert!(
            changes
                .iter()
                .any(|(s, p)| p.ends_with("hello.rs") && *s == FileStatus::Modified)
        );
    }

    #[test]
    fn diff_worktree_detects_staged_changes() {
        let dir = setup_test_repo();
        let d = dir.path();
        std::fs::write(d.join("new.rs"), "fn new_fn() {}").unwrap();
        Command::new("git")
            .args(["add", "new.rs"])
            .current_dir(d)
            .output()
            .unwrap();
        let changes = diff_worktree(d).unwrap();
        assert!(
            changes
                .iter()
                .any(|(s, p)| p.ends_with("new.rs") && *s == FileStatus::Added)
        );
    }

    #[test]
    fn diff_worktree_empty_when_clean() {
        let dir = setup_test_repo();
        let changes = diff_worktree(dir.path()).unwrap();
        assert!(changes.is_empty());
    }

    #[test]
    fn show_file_fails_for_nonexistent_path() {
        let dir = setup_test_repo();
        let result = show_file(dir.path(), "HEAD", "nonexistent.rs");
        assert!(result.is_err());
    }
}
