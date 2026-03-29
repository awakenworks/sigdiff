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
    let mut result = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
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
}
