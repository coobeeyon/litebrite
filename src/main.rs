mod id;
mod model;
mod store;

use clap::{Parser, Subcommand};
use model::{ItemType, Status};

#[derive(Parser)]
#[command(name = "lb", about = "Litebrite — lightweight issue tracker")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create .litebrite/ in the current directory
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
    /// Show open + unblocked items sorted by priority
    Ready,
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
            store::init().map_err(|e| e.to_string())?;
            println!("initialized .litebrite/");
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
            save(&s)?;
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
            if let Some(ref desc) = item.description {
                println!("  Description: {desc}");
            }
            println!("  Created: {}", item.created_at.format("%Y-%m-%d %H:%M"));
            println!("  Updated: {}", item.updated_at.format("%Y-%m-%d %H:%M"));

            if let Some(pid) = store::get_parent(&s, &id) {
                if let Some(p) = s.items.get(&pid) {
                    println!("  Parent: {} ({})", pid, p.title);
                }
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
                    if !should_show(root, all, item_type, status) {
                        continue;
                    }
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
                    item.description = Some(d);
                }
                item.updated_at = chrono::Utc::now();
            }
            if let Some(pid) = parent {
                store::set_parent(&mut s, &id, &pid)?;
            }
            save(&s)?;
            println!("updated {id}");
            Ok(())
        }
        Cmd::Close { id } => {
            let mut s = load()?;
            let id = store::resolve_id(&s, &id)?;
            let item = s.items.get_mut(&id).ok_or("item not found")?;
            item.status = Status::Closed;
            item.updated_at = chrono::Utc::now();
            save(&s)?;
            println!("closed {id}");
            Ok(())
        }
        Cmd::Delete { id } => {
            let mut s = load()?;
            let resolved = store::resolve_id(&s, &id)?;
            store::delete_item(&mut s, &resolved)?;
            save(&s)?;
            println!("deleted {resolved}");
            Ok(())
        }
        Cmd::Dep { action } => match action {
            DepCmd::Add { blocker, blocks } => {
                let mut s = load()?;
                store::add_blocking_dep(&mut s, &blocker, &blocks)?;
                save(&s)?;
                let blocker = store::resolve_id(&s, &blocker)?;
                let blocks = store::resolve_id(&s, &blocks)?;
                println!("{blocker} now blocks {blocks}");
                Ok(())
            }
            DepCmd::Rm { from, to } => {
                let mut s = load()?;
                store::remove_dep(&mut s, &from, &to)?;
                save(&s)?;
                println!("removed dependency");
                Ok(())
            }
            DepCmd::List { id } => {
                let s = load()?;
                let id = store::resolve_id(&s, &id)?;

                if let Some(pid) = store::get_parent(&s, &id) {
                    if let Some(p) = s.items.get(&pid) {
                        println!("parent: {} {}", pid, p.title);
                    }
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
    }
}

fn load() -> Result<model::Store, String> {
    store::load().map_err(|e| e.to_string())
}

fn save(s: &model::Store) -> Result<(), String> {
    store::save(s).map_err(|e| e.to_string())
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
    if let Some(t) = item_type {
        if item.item_type != t {
            return false;
        }
    }
    if let Some(s) = status {
        if item.status != s {
            return false;
        }
    }
    true
}

fn print_list_header() {
    println!(
        "{:<10} {:<8} {:<12} {:<4} {}",
        "ID", "TYPE", "STATUS", "PRI", "TITLE"
    );
    println!("{}", "-".repeat(60));
}

fn print_list_row(item: &model::Item) {
    println!(
        "{:<10} {:<8} {:<12} P{}   {}",
        item.id, item.item_type, item.status, item.priority, item.title
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
        let indent = "  ".repeat(depth);
        println!(
            "{}{} [{}] P{} {} ({})",
            indent, item.id, item.status, item.priority, item.title, item.item_type
        );
        let children = store::get_children(store, id);
        for cid in &children {
            if let Some(child) = store.items.get(cid) {
                if should_show(child, all, item_type, status) {
                    print_tree_item(store, cid, depth + 1, all, item_type, status);
                }
            }
        }
    }
}
