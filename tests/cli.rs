use std::process::Command;
use tempfile::TempDir;

/// Set up an isolated git repo with litebrite initialized.
fn setup_git_dir() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");
    let run = |args: &[&str]| {
        let out = Command::new(args[0])
            .args(&args[1..])
            .current_dir(dir.path())
            .output()
            .unwrap_or_else(|e| panic!("failed to run {:?}: {e}", args));
        assert!(
            out.status.success(),
            "command {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    };
    run(&["git", "init"]);
    run(&["git", "config", "user.name", "Test"]);
    run(&["git", "config", "user.email", "test@test.com"]);
    run(&["git", "commit", "--allow-empty", "-m", "init"]);
    lb(&dir, &["init"]);
    dir
}

/// Run `lb` with args in the given dir, returning (stdout, stderr, success).
fn lb(dir: &TempDir, args: &[&str]) -> (String, String, bool) {
    let bin = env!("CARGO_BIN_EXE_lb");
    let out = Command::new(bin)
        .args(args)
        .current_dir(dir.path())
        .output()
        .expect("failed to run lb");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.success(),
    )
}

#[test]
fn close_with_open_children_fails() {
    let dir = setup_git_dir();

    // Create an epic with two children
    let (stdout, _, ok) = lb(&dir, &["create", "my-epic", "-t", "epic"]);
    assert!(ok, "create epic failed");
    let epic_id = stdout.trim().split_whitespace().last().unwrap().to_string();

    let (stdout, _, ok) = lb(&dir, &["create", "child1", "--parent", &epic_id]);
    assert!(ok, "create child1 failed");
    let child1_id = stdout.trim().split_whitespace().last().unwrap().to_string();

    let (stdout, _, ok) = lb(&dir, &["create", "child2", "--parent", &epic_id]);
    assert!(ok, "create child2 failed");
    let _child2_id = stdout.trim().split_whitespace().last().unwrap().to_string();

    // Closing the epic should fail
    let (_, stderr, ok) = lb(&dir, &["close", &epic_id]);
    assert!(!ok, "close should have failed");
    assert!(
        stderr.contains("open children"),
        "expected 'open children' in stderr: {stderr}"
    );

    // Close one child — epic should still fail
    let (_, _, ok) = lb(&dir, &["close", &child1_id]);
    assert!(ok, "close child1 failed");

    let (_, stderr, ok) = lb(&dir, &["close", &epic_id]);
    assert!(!ok, "close should still fail with one open child");
    assert!(stderr.contains("open children"), "{stderr}");

    // Close the other child — now epic should succeed
    let (_, _, ok) = lb(&dir, &["close", &_child2_id]);
    assert!(ok, "close child2 failed");

    let (stdout, _, ok) = lb(&dir, &["close", &epic_id]);
    assert!(ok, "close epic should succeed now: {stdout}");
}
