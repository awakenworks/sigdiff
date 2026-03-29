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
