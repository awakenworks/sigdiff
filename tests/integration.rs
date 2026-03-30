use std::process::Command;

fn sigdiff(args: &[&str]) -> (String, String, bool) {
    let output = Command::new(env!("CARGO_BIN_EXE_sigdiff"))
        .args(args)
        .output()
        .expect("failed to run sigdiff");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

#[test]
fn langs_lists_languages() {
    let (stdout, _, success) = sigdiff(&["langs"]);
    assert!(success);
    assert!(stdout.contains("rust"));
    assert!(stdout.contains("python"));
    assert!(stdout.contains("go"));
    assert!(stdout.contains("java"));
}

#[test]
fn map_outputs_signatures() {
    let (stdout, _, success) = sigdiff(&["map", "--no-color", "."]);
    assert!(success);
    // Should find sigdiff's own Rust source signatures
    assert!(stdout.contains("main"));
}

#[test]
fn map_json_is_valid() {
    let (stdout, _, success) = sigdiff(&["map", "--format", "json", "."]);
    assert!(success);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert!(parsed.is_array());
}

#[test]
fn map_with_lang_filter() {
    let (stdout, _, success) = sigdiff(&["map", "--no-color", "--lang", "rust", "."]);
    assert!(success);
    // Should only contain Rust files, output should not be empty for this Rust project
    assert!(!stdout.is_empty());
}

#[test]
fn map_with_public_only() {
    let (stdout, _, success) = sigdiff(&["map", "--no-color", "--public-only", "."]);
    assert!(success);
    assert!(!stdout.is_empty());
}

#[test]
fn map_with_kind_filter() {
    let (stdout, _, success) = sigdiff(&["map", "--no-color", "--kind", "struct", "."]);
    assert!(success);
    // This Rust project has structs
    assert!(!stdout.is_empty());
}

#[test]
fn map_with_grep_filter() {
    let (stdout, _, success) = sigdiff(&["map", "--no-color", "--grep", "Provider", "."]);
    assert!(success);
    // Should find language Provider structs
    assert!(stdout.contains("Provider"));
}

#[test]
fn map_with_max_tokens() {
    let (stdout, _, success) = sigdiff(&["map", "--no-color", "--max-tokens", "10", "."]);
    assert!(success);
    // Should be truncated with a message
    assert!(stdout.contains("truncated"));
}

#[test]
fn diff_no_changes_on_clean_repo() {
    let (stdout, _, success) = sigdiff(&["diff", "--no-color"]);
    assert!(success);
    // On a clean repo, diff should produce empty or minimal output
    let _ = stdout; // Just verify it doesn't crash
}

#[test]
fn diff_json_format() {
    let (stdout, _, success) = sigdiff(&["diff", "--format", "json"]);
    assert!(success);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert!(parsed.is_array());
}

#[test]
fn refs_for_existing_file() {
    let (stdout, _, success) = sigdiff(&["refs", "--no-color", "src/main.rs"]);
    assert!(success);
    assert!(stdout.contains("src/main.rs"));
}

#[test]
fn refs_json_format() {
    let (stdout, _, success) = sigdiff(&["refs", "--format", "json", "src/main.rs"]);
    assert!(success);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert!(parsed.is_object());
}

#[test]
fn refs_direction_uses() {
    let (stdout, _, success) =
        sigdiff(&["refs", "--no-color", "--direction", "uses", "src/main.rs"]);
    assert!(success);
    // Should not contain "used by" section
    assert!(!stdout.contains("← used by:"));
}

#[test]
fn refs_direction_used_by() {
    let (stdout, _, success) = sigdiff(&[
        "refs",
        "--no-color",
        "--direction",
        "used-by",
        "src/main.rs",
    ]);
    assert!(success);
    // Should not contain "uses" section
    assert!(!stdout.contains("→ uses:"));
}

#[test]
fn map_with_path_filter() {
    let (stdout, _, success) = sigdiff(&["map", "--no-color", "crates/"]);
    assert!(success);
    // Should only show files under crates/
    assert!(!stdout.is_empty());
}
