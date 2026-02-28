mod git;
mod id;
mod model;
mod store;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use model::{ItemType, Status};

#[derive(Parser)]
#[command(name = "lb", about = "Litebrite — lightweight issue tracker", version)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Initialize litebrite in this git repo
    Init,
    /// Create a new item
    Create {
        title: String,
        #[arg(short = 't', long = "type", default_value = "task")]
        item_type: ItemType,
        #[arg(short, long, default_value_t = 2)]
        priority: u8,
        #[arg(long)]
        parent: Option<String>,
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Show item details
    Show { id: String },
    /// List items
    List {
        /// Show all statuses (default hides closed)
        #[arg(long)]
        all: bool,
        #[arg(short = 't', long = "type")]
        item_type: Option<ItemType>,
        #[arg(short, long)]
        status: Option<Status>,
        /// Display as tree
        #[arg(long)]
        tree: bool,
    },
    /// Update an item
    Update {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        status: Option<Status>,
        #[arg(short = 't', long = "type")]
        item_type: Option<ItemType>,
        #[arg(short, long)]
        priority: Option<u8>,
        #[arg(short, long)]
        description: Option<String>,
        #[arg(long)]
        parent: Option<String>,
    },
    /// Close an item (shorthand for --status closed)
    Close { id: String },
    /// Delete an item and its deps
    Delete { id: String },
    /// Manage dependencies
    Dep {
        #[command(subcommand)]
        action: DepCmd,
    },
    /// Show open + unblocked + unclaimed items sorted by priority
    Ready,
    /// Claim an item (fetch + set claimed_by + push)
    Claim { id: String },
    /// Unclaim an item (fetch + clear claimed_by + push)
    Unclaim { id: String },
    /// Sync local changes with remote (fetch + merge + push)
    Sync,
    /// Output AI-optimized context for Claude Code hooks
    Prime,
    /// Set up integrations
    Setup {
        #[command(subcommand)]
        action: SetupCmd,
    },
    /// Generate shell completions
    #[command(hide = true)]
    Completions { shell: Shell },
}

#[derive(Subcommand)]
enum DepCmd {
    /// Add a blocking dependency
    Add {
        blocker: String,
        #[arg(long)]
        blocks: String,
    },
    /// Remove a dependency
    Rm { from: String, to: String },
    /// List dependencies for an item
    List { id: String },
}

#[derive(Subcommand)]
enum SetupCmd {
    /// Set up Claude Code integration (hooks + permissions)
    Claude,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Cmd::Init => {
            let empty_store = store::to_json(&model::Store::default())?;
            git::init_branch(&empty_store)?;
            println!("initialized litebrite branch");
            Ok(())
        }
        Cmd::Create {
            title,
            item_type,
            priority,
            parent,
            description,
        } => {
            let mut s = load()?;
            let id = store::create_item(&mut s, title, item_type, priority, description, parent)?;
            save(&s, &format!("Create item {id}"))?;
            println!("created {id}");
            Ok(())
        }
        Cmd::Show { id } => {
            let s = load()?;
            let id = store::resolve_id(&s, &id)?;
            let item = s.items.get(&id).ok_or("item not found")?;
            println!("  ID: {}", item.id);
            println!("  Title: {}", item.title);
            println!("  Type: {}", item.item_type);
            println!("  Status: {}", item.status);
            println!("  Priority: P{}", item.priority);
            if let Some(ref who) = item.claimed_by {
                println!("  Claimed by: {who}");
            }
            if let Some(ref desc) = item.description {
                println!("  Description: {desc}");
            }
            println!("  Created: {}", item.created_at.format("%Y-%m-%d %H:%M"));
            println!("  Updated: {}", item.updated_at.format("%Y-%m-%d %H:%M"));

            if let Some(pid) = store::get_parent(&s, &id)
                && let Some(p) = s.items.get(&pid)
            {
                println!("  Parent: {} ({})", pid, p.title);
            }

            let children = store::get_children(&s, &id);
            if !children.is_empty() {
                println!("  Children:");
                for cid in &children {
                    if let Some(c) = s.items.get(cid) {
                        println!("    {} [{}] {}", cid, c.status, c.title);
                    }
                }
            }

            let blockers = store::get_blockers(&s, &id);
            if !blockers.is_empty() {
                println!("  Blocked by:");
                for bid in &blockers {
                    if let Some(b) = s.items.get(bid) {
                        println!("    {} [{}] {}", bid, b.status, b.title);
                    }
                }
            }

            let blocking = store::get_blocking(&s, &id);
            if !blocking.is_empty() {
                println!("  Blocks:");
                for bid in &blocking {
                    if let Some(b) = s.items.get(bid) {
                        println!("    {} [{}] {}", bid, b.status, b.title);
                    }
                }
            }

            Ok(())
        }
        Cmd::List {
            all,
            item_type,
            status,
            tree,
        } => {
            let s = load()?;
            if tree {
                let roots = store::root_items(&s);
                for root in &roots {
                    print_tree_item(&s, &root.id, 0, all, item_type, status);
                }
            } else {
                print_list_header();
                let mut items: Vec<&model::Item> = s.items.values().collect();
                items.sort_by_key(|i| (i.priority, i.id.clone()));
                for item in items {
                    if should_show(item, all, item_type, status) {
                        print_list_row(item);
                    }
                }
            }
            Ok(())
        }
        Cmd::Update {
            id,
            title,
            status,
            item_type,
            priority,
            description,
            parent,
        } => {
            let mut s = load()?;
            let id = store::resolve_id(&s, &id)?;
            {
                let item = s.items.get_mut(&id).ok_or("item not found")?;
                if let Some(t) = title {
                    item.title = t;
                }
                if let Some(st) = status {
                    item.status = st;
                }
                if let Some(it) = item_type {
                    item.item_type = it;
                }
                if let Some(p) = priority {
                    item.priority = p;
                }
                if let Some(d) = description {
                    item.description = if d.is_empty() { None } else { Some(d) };
                }
                item.updated_at = chrono::Utc::now();
            }
            if let Some(pid) = parent {
                store::set_parent(&mut s, &id, &pid)?;
            }
            save(&s, &format!("Update item {id}"))?;
            println!("updated {id}");
            Ok(())
        }
        Cmd::Close { id } => {
            let mut s = load()?;
            let id = store::resolve_id(&s, &id)?;
            store::close_item(&mut s, &id)?;
            save(&s, &format!("Close item {id}"))?;
            println!("closed {id}");
            Ok(())
        }
        Cmd::Delete { id } => {
            let mut s = load()?;
            let resolved = store::resolve_id(&s, &id)?;
            let deleted = store::delete_item(&mut s, &resolved)?;
            save(&s, &format!("Delete item {resolved}"))?;
            for did in &deleted {
                println!("deleted {did}");
            }
            Ok(())
        }
        Cmd::Dep { action } => match action {
            DepCmd::Add { blocker, blocks } => {
                let mut s = load()?;
                store::add_blocking_dep(&mut s, &blocker, &blocks)?;
                let blocker = store::resolve_id(&s, &blocker)?;
                let blocks = store::resolve_id(&s, &blocks)?;
                save(&s, &format!("{blocker} blocks {blocks}"))?;
                println!("{blocker} now blocks {blocks}");
                Ok(())
            }
            DepCmd::Rm { from, to } => {
                let mut s = load()?;
                store::remove_dep(&mut s, &from, &to)?;
                save(&s, "Remove dependency")?;
                println!("removed dependency");
                Ok(())
            }
            DepCmd::List { id } => {
                let s = load()?;
                let id = store::resolve_id(&s, &id)?;

                if let Some(pid) = store::get_parent(&s, &id)
                    && let Some(p) = s.items.get(&pid)
                {
                    println!("parent: {} {}", pid, p.title);
                }

                let children = store::get_children(&s, &id);
                if !children.is_empty() {
                    println!("children:");
                    for cid in &children {
                        if let Some(c) = s.items.get(cid) {
                            println!("  {} {}", cid, c.title);
                        }
                    }
                }

                let blockers = store::get_blockers(&s, &id);
                if !blockers.is_empty() {
                    println!("blocked by:");
                    for bid in &blockers {
                        if let Some(b) = s.items.get(bid) {
                            println!("  {} [{}] {}", bid, b.status, b.title);
                        }
                    }
                }

                let blocking = store::get_blocking(&s, &id);
                if !blocking.is_empty() {
                    println!("blocks:");
                    for bid in &blocking {
                        if let Some(b) = s.items.get(bid) {
                            println!("  {} [{}] {}", bid, b.status, b.title);
                        }
                    }
                }

                Ok(())
            }
        },
        Cmd::Ready => {
            let s = load()?;
            let items = store::ready_items(&s);
            if items.is_empty() {
                println!("no ready items");
            } else {
                print_list_header();
                for item in items {
                    print_list_row(item);
                }
            }
            Ok(())
        }
        Cmd::Claim { id } => {
            let has_remote = sync_from_remote()?;

            let mut s = load()?;
            let id = store::resolve_id(&s, &id)?;
            let item = s.items.get(&id).ok_or("item not found")?;

            if item.status == Status::Closed {
                return Err(format!("item {id} is closed"));
            }
            if let Some(ref who) = item.claimed_by {
                return Err(format!("item {id} already claimed by {who}"));
            }

            let user = git::git_user_name()?;
            let item = s.items.get_mut(&id).ok_or("item not found")?;
            item.claimed_by = Some(user.clone());
            item.updated_at = chrono::Utc::now();
            save(&s, &format!("{user} claims {id}"))?;

            if has_remote {
                // Push — retry once on conflict
                match git::push() {
                    Ok(()) => {}
                    Err(_) => {
                        // Push rejected — fetch and check if someone else claimed it
                        git::fetch().map_err(|e| format!("fetch failed on retry: {e}"))?;
                        let remote_json =
                            git::read_store_from_ref("refs/remotes/origin/litebrite")?;
                        let remote_store = store::from_json(&remote_json)?;
                        if let Some(remote_item) = remote_store.items.get(&id)
                            && let Some(ref who) = remote_item.claimed_by
                        {
                            return Err(format!("item {id} already claimed by {who}"));
                        }

                        // Not a claim conflict — try merge and push
                        let base_commit = git::merge_base()?;
                        let base_store = match base_commit {
                            Some(ref commit) => {
                                let json = git::read_store_from_ref(commit)?;
                                store::from_json(&json)?
                            }
                            None => model::Store::default(),
                        };
                        let merged = store::merge_stores(&base_store, &s, &remote_store)?;
                        let merged_json = store::to_json(&merged)?;

                        let local_ref = git::local_ref()?;
                        let remote_ref = git::remote_ref()?;
                        git::create_merge_commit(
                            &merged_json,
                            &local_ref,
                            &remote_ref,
                            &format!("Merge: {user} claims {id}"),
                        )?;
                        git::push().map_err(|e| format!("push failed after merge: {e}"))?;
                    }
                }
            }

            println!("claimed {id} ({user})");
            Ok(())
        }
        Cmd::Unclaim { id } => {
            let has_remote = sync_from_remote()?;

            let mut s = load()?;
            let id = store::resolve_id(&s, &id)?;
            let item = s.items.get(&id).ok_or("item not found")?;

            if item.claimed_by.is_none() {
                return Err(format!("item {id} is not claimed"));
            }

            let item = s.items.get_mut(&id).ok_or("item not found")?;
            item.claimed_by = None;
            item.updated_at = chrono::Utc::now();
            save(&s, &format!("Unclaim {id}"))?;

            if has_remote {
                // Push — retry once on conflict
                match git::push() {
                    Ok(()) => {}
                    Err(_) => {
                        git::fetch().map_err(|e| format!("fetch failed on retry: {e}"))?;
                        let remote_json =
                            git::read_store_from_ref("refs/remotes/origin/litebrite")?;
                        let remote_store = store::from_json(&remote_json)?;

                        let base_commit = git::merge_base()?;
                        let base_store = match base_commit {
                            Some(ref commit) => {
                                let json = git::read_store_from_ref(commit)?;
                                store::from_json(&json)?
                            }
                            None => model::Store::default(),
                        };
                        let merged = store::merge_stores(&base_store, &s, &remote_store)?;
                        let merged_json = store::to_json(&merged)?;

                        let local_ref = git::local_ref()?;
                        let remote_ref = git::remote_ref()?;
                        git::create_merge_commit(
                            &merged_json,
                            &local_ref,
                            &remote_ref,
                            &format!("Merge: unclaim {id}"),
                        )?;
                        git::push().map_err(|e| format!("push failed after merge: {e}"))?;
                    }
                }
            }

            println!("unclaimed {id}");
            Ok(())
        }
        Cmd::Sync => {
            if !git::has_remote() {
                return Err("no remote configured — nothing to sync".to_string());
            }

            if git::fetch().is_err() || !git::remote_branch_exists() {
                // Remote doesn't have the branch yet — just push
                git::push().map_err(|e| format!("push failed: {e}"))?;
                println!("pushed litebrite branch to remote");
                return Ok(());
            }

            let local_json = git::read_store()?;
            let remote_json = git::read_store_from_ref("refs/remotes/origin/litebrite")?;

            let local_ref = git::local_ref()?;
            let remote_ref = git::remote_ref()?;

            if local_ref == remote_ref {
                println!("already in sync");
                return Ok(());
            }

            // Try fast-forward first
            git::fast_forward()?;
            let new_local_ref = git::local_ref()?;
            if new_local_ref == remote_ref {
                // We were just behind — fast-forwarded
                println!("fast-forwarded to remote");
                return Ok(());
            }

            // We're ahead or diverged — need to merge
            let base_commit = git::merge_base()?;
            let base_store = match base_commit {
                Some(ref commit) => {
                    let json = git::read_store_from_ref(commit)?;
                    store::from_json(&json)?
                }
                None => model::Store::default(),
            };

            let local_store = store::from_json(&local_json)?;
            let remote_store = store::from_json(&remote_json)?;
            let merged = store::merge_stores(&base_store, &local_store, &remote_store)?;
            let merged_json = store::to_json(&merged)?;

            git::create_merge_commit(
                &merged_json,
                &local_ref,
                &remote_ref,
                "Sync litebrite stores",
            )?;
            git::push().map_err(|e| format!("push failed: {e}"))?;
            println!("synced with remote");
            Ok(())
        }
        Cmd::Prime => {
            print_prime_context();
            Ok(())
        }
        Cmd::Setup { action } => match action {
            SetupCmd::Claude => setup_claude(),
        },
        Cmd::Completions { shell } => {
            generate(shell, &mut Cli::command(), "lb", &mut std::io::stdout());
            Ok(())
        }
    }
}

/// Check remote state and sync if possible. Returns true if remote is available.
/// - No remote configured: returns Ok(false) (local-only operation)
/// - Remote exists, branch on remote: fetches + fast-forwards, returns Ok(true)
/// - Remote exists, no branch on remote: returns Err with instructions
fn sync_from_remote() -> Result<bool, String> {
    if !git::has_remote() {
        return Ok(false);
    }
    match git::fetch() {
        Ok(()) => {
            git::fast_forward()?;
            Ok(true)
        }
        Err(_) => {
            Err("litebrite branch not found on remote — run `lb sync` to push it first".to_string())
        }
    }
}

fn load() -> Result<model::Store, String> {
    let json = git::read_store()?;
    store::from_json(&json)
}

fn save(s: &model::Store, message: &str) -> Result<(), String> {
    let json = store::to_json(s)?;
    git::write_store(&json, message)
}

fn print_prime_context() {
    let s = match load() {
        Ok(s) => s,
        Err(_) => return, // no store — silent exit
    };

    println!("# Litebrite Tracker Active");

    // Claimed section (replaces old In Progress)
    let claimed: Vec<&model::Item> = s
        .items
        .values()
        .filter(|i| i.claimed_by.is_some() && i.status == Status::Open)
        .collect();
    if !claimed.is_empty() {
        println!("\n## Claimed");
        for item in &claimed {
            println!(
                "- {} P{} [{}] {} (by {})",
                item.id,
                item.priority,
                item.item_type,
                item.title,
                item.claimed_by.as_deref().unwrap_or("?")
            );
        }
    }

    // Ready section
    let ready = store::ready_items(&s);
    if !ready.is_empty() {
        println!("\n## Ready (unblocked, unclaimed)");
        for item in &ready {
            println!(
                "- {} P{} [{}] {}",
                item.id, item.priority, item.item_type, item.title
            );
        }
    }

    println!(
        r#"
## Session Protocol
1. `lb ready` — find unblocked, unclaimed work
2. `lb show <id>` — get full context
3. `lb claim <id>` — claim work (syncs with remote)
4. Do the work, commit code
5. `lb close <id>` — mark complete (clears claim)
6. `lb sync` — push changes to remote

## CLI Quick Reference
- `lb create <title>` — new item (-t epic/feature/task, -p <pri>, --parent <id>, -d <desc>)
- `lb show <id>` — item details with deps and children
- `lb list` — all open items (--all, -t <type>, -s <status>, --tree)
- `lb update <id>` — update fields (--title, --status, -t, -p, -d, --parent)
- `lb close <id>` — close item (clears claim)
- `lb delete <id>` — delete item and deps
- `lb dep add <id> --blocks <id>` — add blocking dep
- `lb dep rm <from> <to>` — remove dep
- `lb ready` — open + unblocked + unclaimed by priority
- `lb claim <id>` — claim item (fetch + push)
- `lb unclaim <id>` — release claim (fetch + push)
- `lb sync` — sync with remote (fetch + merge + push)
- IDs: `lb-XXXX`, use any unique prefix"#
    );
}

fn setup_claude() -> Result<(), String> {
    setup_claude_in(std::path::Path::new("."))
}

fn setup_claude_in(base: &std::path::Path) -> Result<(), String> {
    let claude_dir = base.join(".claude");
    std::fs::create_dir_all(&claude_dir).map_err(|e| format!("create dirs: {e}"))?;

    // Merge settings.local.json
    let settings_path = claude_dir.join("settings.local.json");
    let mut settings: serde_json::Value = if settings_path.exists() {
        let data = std::fs::read_to_string(&settings_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| format!("parse settings: {e}"))?
    } else {
        serde_json::json!({})
    };

    // Ensure permissions.allow exists and merge lb permissions
    let allow = settings
        .pointer_mut("/permissions/allow")
        .and_then(|v| v.as_array_mut());
    let lb_perms = vec!["Bash(lb:*)"];
    if let Some(arr) = allow {
        for perm in &lb_perms {
            let val = serde_json::Value::String(perm.to_string());
            if !arr.contains(&val) {
                arr.push(val);
            }
        }
    } else {
        settings["permissions"]["allow"] = serde_json::json!(lb_perms);
    }

    // Ensure hooks (new matcher-based format)
    let matcher_group = |cmd: &str| {
        serde_json::json!({
            "matcher": "*",
            "hooks": [{ "type": "command", "command": cmd }]
        })
    };
    let session_group = matcher_group("lb prime");
    let compact_group = matcher_group("lb prime");
    let hooks = serde_json::json!({
        "SessionStart": [session_group],
        "PreCompact": [compact_group]
    });
    if let Some(existing_hooks) = settings.get_mut("hooks") {
        for key in ["SessionStart", "PreCompact"] {
            let group = matcher_group("lb prime");
            if let Some(arr) = existing_hooks.get_mut(key).and_then(|v| v.as_array_mut()) {
                let has_lb_prime = arr.iter().any(|g| {
                    g.get("hooks")
                        .and_then(|h| h.as_array())
                        .is_some_and(|hooks| {
                            hooks.iter().any(|h| {
                                h.get("command").and_then(|c| c.as_str()) == Some("lb prime")
                            })
                        })
                });
                if !has_lb_prime {
                    arr.push(group);
                }
            } else {
                existing_hooks[key] = serde_json::json!([group]);
            }
        }
    } else {
        settings["hooks"] = hooks;
    }

    let settings_json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&settings_path, settings_json).map_err(|e| e.to_string())?;
    println!("wrote .claude/settings.local.json (hooks + permissions)");

    Ok(())
}

fn should_show(
    item: &model::Item,
    all: bool,
    item_type: Option<ItemType>,
    status: Option<Status>,
) -> bool {
    if !all && status.is_none() && item.status == Status::Closed {
        return false;
    }
    if let Some(t) = item_type
        && item.item_type != t
    {
        return false;
    }
    if let Some(s) = status
        && item.status != s
    {
        return false;
    }
    true
}

fn print_list_header() {
    println!(
        "{:<10} {:<8} {:<14} {:<4} TITLE",
        "ID", "TYPE", "STATUS", "PRI"
    );
    println!("{}", "-".repeat(60));
}

fn print_list_row(item: &model::Item) {
    let status_str = if item.claimed_by.is_some() {
        "open (claimed)".to_string()
    } else {
        item.status.to_string()
    };
    println!(
        "{:<10} {:<8} {:<14} {:<4} {}",
        item.id,
        item.item_type,
        status_str,
        format!("P{}", item.priority),
        item.title
    );
}

fn print_tree_item(
    store: &model::Store,
    id: &str,
    depth: usize,
    all: bool,
    item_type: Option<ItemType>,
    status: Option<Status>,
) {
    if let Some(item) = store.items.get(id) {
        let visible = should_show(item, all, item_type, status);
        let child_depth = if visible {
            let claimed = if item.claimed_by.is_some() {
                " *claimed*"
            } else {
                ""
            };
            let indent = "  ".repeat(depth);
            println!(
                "{}{} [{}] P{} {} ({}){claimed}",
                indent, item.id, item.status, item.priority, item.title, item.item_type
            );
            depth + 1
        } else {
            depth
        };
        let children = store::get_children(store, id);
        for cid in &children {
            print_tree_item(store, cid, child_depth, all, item_type, status);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::process::Command;

    fn make_item(status: Status, item_type: ItemType) -> model::Item {
        let now = Utc::now();
        model::Item {
            id: "lb-test".to_string(),
            title: "test".to_string(),
            description: None,
            item_type,
            status,
            priority: 2,
            claimed_by: None,
            created_at: now,
            updated_at: now,
        }
    }

    // --- should_show ---

    #[test]
    fn hides_closed_by_default() {
        let item = make_item(Status::Closed, ItemType::Task);
        assert!(!should_show(&item, false, None, None));
    }

    #[test]
    fn shows_closed_with_all() {
        let item = make_item(Status::Closed, ItemType::Task);
        assert!(should_show(&item, true, None, None));
    }

    #[test]
    fn filters_by_item_type() {
        let item = make_item(Status::Open, ItemType::Epic);
        assert!(!should_show(&item, false, Some(ItemType::Task), None));
        assert!(should_show(&item, false, Some(ItemType::Epic), None));
    }

    #[test]
    fn filters_by_status() {
        let item = make_item(Status::Open, ItemType::Task);
        assert!(should_show(&item, false, None, Some(Status::Open)));
        assert!(!should_show(&item, false, None, Some(Status::Closed)));
    }

    #[test]
    fn status_filter_overrides_closed_hiding() {
        let item = make_item(Status::Closed, ItemType::Task);
        // With status filter for Closed, should show even without --all
        assert!(should_show(&item, false, None, Some(Status::Closed)));
    }

    // --- CLI integration ---

    fn lb_bin() -> std::path::PathBuf {
        let mut path = std::env::current_exe().unwrap();
        path.pop(); // remove test binary name
        path.pop(); // remove "deps"
        path.push("lb");
        if !path.exists() {
            // Newer Rust versions may not place the binary in target/debug/;
            // fall back to building it explicitly.
            let status = Command::new("cargo")
                .args(["build", "--bin", "lb"])
                .status()
                .expect("failed to run cargo build");
            assert!(status.success(), "cargo build --bin lb failed");
        }
        path
    }

    fn lb_cmd(dir: &std::path::Path) -> Command {
        let mut cmd = Command::new(lb_bin());
        cmd.current_dir(dir);
        cmd
    }

    /// Set up a temp dir with a git repo for CLI tests.
    fn setup_git_dir() -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        tmp
    }

    #[test]
    fn cli_init_create_list() {
        let tmp = setup_git_dir();

        // init
        let out = lb_cmd(tmp.path()).arg("init").output().unwrap();
        assert!(
            out.status.success(),
            "init failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("initialized"), "{stdout}");

        // Verify branch exists
        let out = Command::new("git")
            .args(["branch", "--list", "litebrite"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("litebrite"), "branch not created: {stdout}");

        // create
        let out = lb_cmd(tmp.path())
            .args(["create", "My first task"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "create failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.starts_with("created lb-"), "{stdout}");

        // list
        let out = lb_cmd(tmp.path()).arg("list").output().unwrap();
        assert!(out.status.success());
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("My first task"), "{stdout}");
    }

    #[test]
    fn cli_dep_add_and_ready() {
        let tmp = setup_git_dir();
        lb_cmd(tmp.path()).arg("init").output().unwrap();

        // Create two items
        let out = lb_cmd(tmp.path())
            .args(["create", "blocker"])
            .output()
            .unwrap();
        let blocker_id = String::from_utf8_lossy(&out.stdout)
            .trim()
            .strip_prefix("created ")
            .unwrap()
            .to_string();

        let out = lb_cmd(tmp.path())
            .args(["create", "blocked"])
            .output()
            .unwrap();
        let blocked_id = String::from_utf8_lossy(&out.stdout)
            .trim()
            .strip_prefix("created ")
            .unwrap()
            .to_string();

        // Add dep
        let out = lb_cmd(tmp.path())
            .args(["dep", "add", &blocker_id, "--blocks", &blocked_id])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "dep add failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        // Ready should only show the blocker
        let out = lb_cmd(tmp.path()).arg("ready").output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("blocker"), "{stdout}");
        assert!(
            !stdout.contains("blocked"),
            "blocked item should not be ready: {stdout}"
        );
    }

    #[test]
    fn cli_unknown_command_exits_nonzero() {
        let out = Command::new(lb_bin()).arg("nonexistent").output().unwrap();
        assert!(!out.status.success());
    }

    // --- prime ---

    #[test]
    fn cli_prime_no_store_silent_exit() {
        let tmp = tempfile::TempDir::new().unwrap();
        let out = lb_cmd(tmp.path()).arg("prime").output().unwrap();
        assert!(out.status.success(), "prime should exit 0 without store");
        assert!(
            out.stdout.is_empty(),
            "prime should produce no output without store"
        );
    }

    #[test]
    fn cli_prime_with_items() {
        let tmp = setup_git_dir();
        lb_cmd(tmp.path()).arg("init").output().unwrap();

        // Create a ready item
        lb_cmd(tmp.path())
            .args(["create", "Ready task", "-p", "0"])
            .output()
            .unwrap();

        let out = lb_cmd(tmp.path()).arg("prime").output().unwrap();
        assert!(out.status.success());
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("# Litebrite Tracker Active"), "{stdout}");
        assert!(stdout.contains("## Ready"), "{stdout}");
        assert!(stdout.contains("Ready task"), "{stdout}");
        assert!(stdout.contains("## Session Protocol"), "{stdout}");
        assert!(stdout.contains("## CLI Quick Reference"), "{stdout}");
    }

    #[test]
    fn cli_prime_empty_store() {
        let tmp = setup_git_dir();
        lb_cmd(tmp.path()).arg("init").output().unwrap();

        let out = lb_cmd(tmp.path()).arg("prime").output().unwrap();
        assert!(out.status.success());
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("# Litebrite Tracker Active"), "{stdout}");
        // No Claimed or Ready sections when empty
        assert!(!stdout.contains("## Claimed"), "{stdout}");
        assert!(!stdout.contains("## Ready"), "{stdout}");
        assert!(stdout.contains("## Session Protocol"), "{stdout}");
    }

    // --- setup claude ---

    #[test]
    fn cli_setup_claude_writes_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let out = lb_cmd(tmp.path())
            .args(["setup", "claude"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "setup claude failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("settings.local.json"), "{stdout}");

        // Verify settings file exists and has correct content
        assert!(tmp.path().join(".claude/settings.local.json").exists());
        let settings: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join(".claude/settings.local.json")).unwrap(),
        )
        .unwrap();
        assert!(
            settings["permissions"]["allow"]
                .as_array()
                .unwrap()
                .contains(&serde_json::Value::String("Bash(lb:*)".to_string()))
        );
        assert!(settings["hooks"]["SessionStart"].is_array());
        assert!(settings["hooks"]["PreCompact"].is_array());
    }

    #[test]
    fn cli_setup_claude_merges_existing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.local.json"),
            r#"{"permissions":{"allow":["Bash(git:*)"]}}"#,
        )
        .unwrap();

        let out = lb_cmd(tmp.path())
            .args(["setup", "claude"])
            .output()
            .unwrap();
        assert!(out.status.success());

        let settings: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(claude_dir.join("settings.local.json")).unwrap(),
        )
        .unwrap();
        let allow = settings["permissions"]["allow"].as_array().unwrap();
        // Should have both the existing and new permissions
        assert!(allow.contains(&serde_json::Value::String("Bash(git:*)".to_string())));
        assert!(allow.contains(&serde_json::Value::String("Bash(lb:*)".to_string())));
    }

    #[test]
    fn cli_setup_claude_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();

        // Run twice
        lb_cmd(tmp.path())
            .args(["setup", "claude"])
            .output()
            .unwrap();
        lb_cmd(tmp.path())
            .args(["setup", "claude"])
            .output()
            .unwrap();

        let settings: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join(".claude/settings.local.json")).unwrap(),
        )
        .unwrap();
        // Should not duplicate permissions or hooks
        let allow = settings["permissions"]["allow"].as_array().unwrap();
        let lb_count = allow
            .iter()
            .filter(|v| v.as_str() == Some("Bash(lb:*)"))
            .count();
        assert_eq!(lb_count, 1, "permission duplicated: {allow:?}");

        let session_hooks = settings["hooks"]["SessionStart"].as_array().unwrap();
        let prime_count = session_hooks
            .iter()
            .filter(|g| {
                g.get("hooks")
                    .and_then(|h| h.as_array())
                    .map_or(false, |hooks| {
                        hooks
                            .iter()
                            .any(|h| h.get("command").and_then(|c| c.as_str()) == Some("lb prime"))
                    })
            })
            .count();
        assert_eq!(prime_count, 1, "hook duplicated: {session_hooks:?}");
    }

    // --- close clears claim ---

    #[test]
    fn cli_close_clears_claim() {
        let tmp = setup_git_dir();
        lb_cmd(tmp.path()).arg("init").output().unwrap();

        let out = lb_cmd(tmp.path())
            .args(["create", "claimable"])
            .output()
            .unwrap();
        let id = String::from_utf8_lossy(&out.stdout)
            .trim()
            .strip_prefix("created ")
            .unwrap()
            .to_string();

        // Verify the store is on the branch
        let out = Command::new("git")
            .args(["show", "litebrite:store.json"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        assert!(out.status.success(), "store.json not on branch");

        // Close it
        let out = lb_cmd(tmp.path()).args(["close", &id]).output().unwrap();
        assert!(
            out.status.success(),
            "close failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        // Show should show closed status
        let out = lb_cmd(tmp.path()).args(["show", &id]).output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("closed"), "{stdout}");
    }

    // --- init already initialized ---

    #[test]
    fn cli_init_already_initialized() {
        let tmp = setup_git_dir();
        lb_cmd(tmp.path()).arg("init").output().unwrap();

        let out = lb_cmd(tmp.path()).arg("init").output().unwrap();
        assert!(!out.status.success(), "second init should fail");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("already initialized"), "{stderr}");
    }

    // --- claim/unclaim without remote ---

    #[test]
    fn cli_claim_no_remote() {
        let tmp = setup_git_dir();
        lb_cmd(tmp.path()).arg("init").output().unwrap();

        let out = lb_cmd(tmp.path())
            .args(["create", "claimable"])
            .output()
            .unwrap();
        let id = String::from_utf8_lossy(&out.stdout)
            .trim()
            .strip_prefix("created ")
            .unwrap()
            .to_string();

        let out = lb_cmd(tmp.path()).args(["claim", &id]).output().unwrap();
        assert!(
            out.status.success(),
            "claim without remote should succeed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("claimed"), "{stdout}");

        // Show should reflect the claim
        let out = lb_cmd(tmp.path()).args(["show", &id]).output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("Claimed by:"), "{stdout}");
    }

    #[test]
    fn cli_unclaim_no_remote() {
        let tmp = setup_git_dir();
        lb_cmd(tmp.path()).arg("init").output().unwrap();

        let out = lb_cmd(tmp.path())
            .args(["create", "claimable"])
            .output()
            .unwrap();
        let id = String::from_utf8_lossy(&out.stdout)
            .trim()
            .strip_prefix("created ")
            .unwrap()
            .to_string();

        lb_cmd(tmp.path()).args(["claim", &id]).output().unwrap();

        let out = lb_cmd(tmp.path()).args(["unclaim", &id]).output().unwrap();
        assert!(
            out.status.success(),
            "unclaim without remote should succeed: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        // Show should no longer have a claim
        let out = lb_cmd(tmp.path()).args(["show", &id]).output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(!stdout.contains("Claimed by:"), "{stdout}");
    }

    // --- sync without remote ---

    #[test]
    fn cli_sync_no_remote() {
        let tmp = setup_git_dir();
        lb_cmd(tmp.path()).arg("init").output().unwrap();

        let out = lb_cmd(tmp.path()).arg("sync").output().unwrap();
        assert!(!out.status.success(), "sync without remote should fail");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("no remote configured"), "{stderr}");
    }

    // --- with bare remote ---

    /// Set up a local repo with a bare remote for network tests.
    fn setup_git_dir_with_remote() -> (tempfile::TempDir, tempfile::TempDir) {
        let bare = tempfile::TempDir::new().unwrap();
        Command::new("git")
            .args(["init", "--bare"])
            .current_dir(bare.path())
            .output()
            .unwrap();

        let work = setup_git_dir();
        Command::new("git")
            .args(["remote", "add", "origin", bare.path().to_str().unwrap()])
            .current_dir(work.path())
            .output()
            .unwrap();

        (work, bare)
    }

    #[test]
    fn cli_init_pushes_to_remote() {
        let (work, bare) = setup_git_dir_with_remote();

        let out = lb_cmd(work.path()).arg("init").output().unwrap();
        assert!(
            out.status.success(),
            "init failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        // Verify branch exists on remote
        let out = Command::new("git")
            .args(["branch", "--list", "litebrite"])
            .current_dir(bare.path())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("litebrite"),
            "init should push to remote: {stdout}"
        );
    }

    #[test]
    fn cli_sync_pushes_new_branch() {
        let (work, _bare) = setup_git_dir_with_remote();

        // Init without remote push (simulate old init by creating branch directly)
        Command::new("git")
            .args(["hash-object", "-w", "--stdin"])
            .stdin(std::process::Stdio::piped())
            .current_dir(work.path())
            .output()
            .unwrap();

        // Use lb init, but since init now pushes, we need to verify sync works
        // when already pushed
        lb_cmd(work.path()).arg("init").output().unwrap();
        lb_cmd(work.path())
            .args(["create", "sync test"])
            .output()
            .unwrap();

        let out = lb_cmd(work.path()).arg("sync").output().unwrap();
        assert!(
            out.status.success(),
            "sync failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn cli_claim_with_remote() {
        let (work, _bare) = setup_git_dir_with_remote();
        lb_cmd(work.path()).arg("init").output().unwrap();

        let out = lb_cmd(work.path())
            .args(["create", "remote claimable"])
            .output()
            .unwrap();
        let id = String::from_utf8_lossy(&out.stdout)
            .trim()
            .strip_prefix("created ")
            .unwrap()
            .to_string();

        // Sync so the branch is on the remote
        lb_cmd(work.path()).arg("sync").output().unwrap();

        let out = lb_cmd(work.path()).args(["claim", &id]).output().unwrap();
        assert!(
            out.status.success(),
            "claim with remote failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("claimed"), "{stdout}");
    }

    #[test]
    fn cli_unclaim_with_remote() {
        let (work, _bare) = setup_git_dir_with_remote();
        lb_cmd(work.path()).arg("init").output().unwrap();

        let out = lb_cmd(work.path())
            .args(["create", "remote claimable"])
            .output()
            .unwrap();
        let id = String::from_utf8_lossy(&out.stdout)
            .trim()
            .strip_prefix("created ")
            .unwrap()
            .to_string();

        lb_cmd(work.path()).arg("sync").output().unwrap();
        lb_cmd(work.path()).args(["claim", &id]).output().unwrap();

        let out = lb_cmd(work.path()).args(["unclaim", &id]).output().unwrap();
        assert!(
            out.status.success(),
            "unclaim with remote failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // --- git show verifies storage ---

    #[test]
    fn git_show_reflects_creates() {
        let tmp = setup_git_dir();
        lb_cmd(tmp.path()).arg("init").output().unwrap();

        lb_cmd(tmp.path())
            .args(["create", "git-visible task"])
            .output()
            .unwrap();

        let out = Command::new("git")
            .args(["show", "litebrite:store.json"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("git-visible task"),
            "store.json on branch should contain the item: {stdout}"
        );
    }
}
