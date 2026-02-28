use std::process::Command;

fn run_git(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(stderr)
    }
}

fn run_git_stdin(args: &[&str], stdin_data: &[u8]) -> Result<String, String> {
    use std::io::Write;
    let mut child = Command::new("git")
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run git: {e}"))?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin_data)
        .map_err(|e| format!("failed to write stdin: {e}"))?;
    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for git: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(stderr)
    }
}

const BRANCH: &str = "litebrite";
const STORE_FILENAME: &str = "store.json";

pub fn branch_exists() -> bool {
    run_git(&["rev-parse", "--verify", &format!("refs/heads/{BRANCH}")])
        .is_ok()
}

pub fn has_remote() -> bool {
    run_git(&["remote", "get-url", "origin"]).is_ok()
}

pub fn remote_branch_exists() -> bool {
    run_git(&["rev-parse", "--verify", &format!("refs/remotes/origin/{BRANCH}")])
        .is_ok()
}

pub fn init_branch(store_json: &str) -> Result<(), String> {
    if branch_exists() {
        return Err("litebrite already initialized".to_string());
    }

    // Check if remote has the branch — if so, set up tracking instead
    // Try fetching first to see if remote exists
    if fetch().is_ok() && remote_branch_exists() {
        run_git(&[
            "branch", BRANCH, &format!("refs/remotes/origin/{BRANCH}"),
        ])?;
        return Ok(());
    }

    // Create orphan branch with empty store
    let blob_hash = run_git_stdin(
        &["hash-object", "-w", "--stdin"],
        store_json.as_bytes(),
    )?;

    let tree_entry = format!("100644 blob {blob_hash}\t{STORE_FILENAME}\n");
    let tree_hash = run_git_stdin(&["mktree"], tree_entry.as_bytes())?;

    let commit_hash = run_git(&[
        "commit-tree", &tree_hash, "-m", "Initialize litebrite",
    ])?;

    run_git(&[
        "update-ref",
        &format!("refs/heads/{BRANCH}"),
        &commit_hash,
    ])?;

    // Push to remote if one is configured
    if has_remote() {
        push()?;
    }

    Ok(())
}

pub fn read_store() -> Result<String, String> {
    run_git(&["show", &format!("{BRANCH}:{STORE_FILENAME}")])
}

pub fn read_store_from_ref(git_ref: &str) -> Result<String, String> {
    run_git(&["show", &format!("{git_ref}:{STORE_FILENAME}")])
}

pub fn write_store(store_json: &str, message: &str) -> Result<(), String> {
    let parent = run_git(&["rev-parse", &format!("refs/heads/{BRANCH}")])?;

    let blob_hash = run_git_stdin(
        &["hash-object", "-w", "--stdin"],
        store_json.as_bytes(),
    )?;

    let tree_entry = format!("100644 blob {blob_hash}\t{STORE_FILENAME}\n");
    let tree_hash = run_git_stdin(&["mktree"], tree_entry.as_bytes())?;

    let commit_hash = run_git(&[
        "commit-tree", &tree_hash, "-p", &parent, "-m", message,
    ])?;

    run_git(&[
        "update-ref",
        &format!("refs/heads/{BRANCH}"),
        &commit_hash,
    ])?;

    Ok(())
}

pub fn fetch() -> Result<(), String> {
    run_git(&[
        "fetch", "origin",
        &format!("{BRANCH}:refs/remotes/origin/{BRANCH}"),
    ])?;
    Ok(())
}

pub fn push() -> Result<(), String> {
    run_git(&["push", "origin", BRANCH])?;
    Ok(())
}

pub fn fast_forward() -> Result<(), String> {
    if !remote_branch_exists() {
        return Ok(());
    }

    let local = run_git(&["rev-parse", &format!("refs/heads/{BRANCH}")])?;
    let remote = run_git(&["rev-parse", &format!("refs/remotes/origin/{BRANCH}")])?;

    if local == remote {
        return Ok(());
    }

    // Check if local is ancestor of remote (we're behind)
    let is_ancestor = run_git(&[
        "merge-base", "--is-ancestor", &local, &remote,
    ]);
    if is_ancestor.is_ok() {
        // Fast-forward local to remote
        run_git(&[
            "update-ref",
            &format!("refs/heads/{BRANCH}"),
            &remote,
        ])?;
    }
    // If remote is ancestor of local, we're ahead — nothing to do
    // If neither, we've diverged — caller handles merge

    Ok(())
}

pub fn merge_base() -> Result<Option<String>, String> {
    if !remote_branch_exists() {
        return Ok(None);
    }
    let local = run_git(&["rev-parse", &format!("refs/heads/{BRANCH}")])?;
    let remote = run_git(&["rev-parse", &format!("refs/remotes/origin/{BRANCH}")])?;
    match run_git(&["merge-base", &local, &remote]) {
        Ok(base) => Ok(Some(base)),
        Err(_) => Ok(None), // no common ancestor
    }
}

pub fn create_merge_commit(
    store_json: &str,
    parent1: &str,
    parent2: &str,
    message: &str,
) -> Result<(), String> {
    let blob_hash = run_git_stdin(
        &["hash-object", "-w", "--stdin"],
        store_json.as_bytes(),
    )?;

    let tree_entry = format!("100644 blob {blob_hash}\t{STORE_FILENAME}\n");
    let tree_hash = run_git_stdin(&["mktree"], tree_entry.as_bytes())?;

    let commit_hash = run_git(&[
        "commit-tree", &tree_hash,
        "-p", parent1,
        "-p", parent2,
        "-m", message,
    ])?;

    run_git(&[
        "update-ref",
        &format!("refs/heads/{BRANCH}"),
        &commit_hash,
    ])?;

    Ok(())
}

pub fn git_user_name() -> Result<String, String> {
    run_git(&["config", "user.name"])
}

pub fn local_ref() -> Result<String, String> {
    run_git(&["rev-parse", &format!("refs/heads/{BRANCH}")])
}

pub fn remote_ref() -> Result<String, String> {
    run_git(&["rev-parse", &format!("refs/remotes/origin/{BRANCH}")])
}
