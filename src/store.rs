use crate::id::generate_id;
use crate::model::*;
use chrono::Utc;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const STORE_DIR: &str = ".litebrite";
const STORE_FILE: &str = ".litebrite/store.json";

pub fn store_path() -> PathBuf {
    PathBuf::from(STORE_FILE)
}

pub fn init() -> io::Result<()> {
    let dir = Path::new(STORE_DIR);
    if dir.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            ".litebrite/ already exists",
        ));
    }
    fs::create_dir(dir)?;
    save(&Store::default())
}

pub fn load() -> io::Result<Store> {
    let path = store_path();
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no .litebrite/store.json — run `lb init` first",
        ));
    }
    let data = fs::read_to_string(&path)?;
    serde_json::from_str(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn save(store: &Store) -> io::Result<()> {
    let path = store_path();
    let tmp = path.with_extension("tmp");
    let data = serde_json::to_string_pretty(store)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(&tmp, data)?;
    fs::rename(&tmp, &path)
}

/// Resolve a prefix like "lb-a3" to a full ID. Errors if ambiguous or not found.
pub fn resolve_id(store: &Store, prefix: &str) -> Result<String, String> {
    // Exact match first
    if store.items.contains_key(prefix) {
        return Ok(prefix.to_string());
    }
    let matches: Vec<&String> = store
        .items
        .keys()
        .filter(|id| id.starts_with(prefix))
        .collect();
    match matches.len() {
        0 => Err(format!("no item matching '{prefix}'")),
        1 => Ok(matches[0].clone()),
        n => Err(format!(
            "ambiguous prefix '{prefix}' matches {n} items: {}",
            matches
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

pub fn create_item(
    store: &mut Store,
    title: String,
    item_type: ItemType,
    priority: u8,
    description: Option<String>,
    parent_id: Option<String>,
) -> Result<String, String> {
    if let Some(ref pid) = parent_id {
        let resolved = resolve_id(store, pid)?;
        if !store.items.contains_key(&resolved) {
            return Err(format!("parent '{resolved}' not found"));
        }
    }

    let existing: Vec<&str> = store.items.keys().map(|s| s.as_str()).collect();
    let id = generate_id(&title, &existing);
    let now = Utc::now();
    let item = Item {
        id: id.clone(),
        title,
        description,
        item_type,
        status: Status::Open,
        priority,
        created_at: now,
        updated_at: now,
    };
    store.items.insert(id.clone(), item);

    if let Some(pid) = parent_id {
        let resolved = resolve_id(store, &pid)?;
        store.deps.push(Dep {
            from_id: id.clone(),
            to_id: resolved,
            dep_type: DepType::Parent,
        });
    }

    Ok(id)
}

pub fn delete_item(store: &mut Store, id: &str) -> Result<(), String> {
    let id = resolve_id(store, id)?;
    store
        .items
        .remove(&id)
        .ok_or_else(|| format!("item '{id}' not found"))?;
    store.deps.retain(|d| d.from_id != id && d.to_id != id);
    Ok(())
}

pub fn get_children(store: &Store, id: &str) -> Vec<String> {
    store
        .deps
        .iter()
        .filter(|d| d.dep_type == DepType::Parent && d.to_id == id)
        .map(|d| d.from_id.clone())
        .collect()
}

pub fn get_parent(store: &Store, id: &str) -> Option<String> {
    store
        .deps
        .iter()
        .find(|d| d.dep_type == DepType::Parent && d.from_id == id)
        .map(|d| d.to_id.clone())
}

pub fn get_blockers(store: &Store, id: &str) -> Vec<String> {
    store
        .deps
        .iter()
        .filter(|d| d.dep_type == DepType::Blocks && d.to_id == id)
        .map(|d| d.from_id.clone())
        .collect()
}

pub fn get_blocking(store: &Store, id: &str) -> Vec<String> {
    store
        .deps
        .iter()
        .filter(|d| d.dep_type == DepType::Blocks && d.from_id == id)
        .map(|d| d.to_id.clone())
        .collect()
}

pub fn add_blocking_dep(store: &mut Store, blocker: &str, blocked: &str) -> Result<(), String> {
    let blocker = resolve_id(store, blocker)?;
    let blocked = resolve_id(store, blocked)?;
    if blocker == blocked {
        return Err("item cannot block itself".to_string());
    }
    let dep = Dep {
        from_id: blocker,
        to_id: blocked,
        dep_type: DepType::Blocks,
    };
    if store.deps.contains(&dep) {
        return Err("dependency already exists".to_string());
    }
    store.deps.push(dep);
    Ok(())
}

pub fn remove_dep(store: &mut Store, from: &str, to: &str) -> Result<(), String> {
    let from = resolve_id(store, from)?;
    let to = resolve_id(store, to)?;
    let before = store.deps.len();
    store
        .deps
        .retain(|d| !(d.from_id == from && d.to_id == to));
    if store.deps.len() == before {
        return Err(format!("no dependency from '{from}' to '{to}'"));
    }
    Ok(())
}

pub fn set_parent(store: &mut Store, child: &str, parent: &str) -> Result<(), String> {
    let child = resolve_id(store, child)?;
    let parent = resolve_id(store, parent)?;
    if child == parent {
        return Err("item cannot be its own parent".to_string());
    }
    // Remove existing parent dep
    store
        .deps
        .retain(|d| !(d.from_id == child && d.dep_type == DepType::Parent));
    store.deps.push(Dep {
        from_id: child,
        to_id: parent,
        dep_type: DepType::Parent,
    });
    Ok(())
}

/// Items that are open with no unresolved (non-closed) blockers, sorted by priority.
pub fn ready_items(store: &Store) -> Vec<&Item> {
    let mut items: Vec<&Item> = store
        .items
        .values()
        .filter(|item| item.status == Status::Open)
        .filter(|item| {
            let blockers = get_blockers(store, &item.id);
            blockers.iter().all(|bid| {
                store
                    .items
                    .get(bid)
                    .is_some_and(|b| b.status == Status::Closed)
            })
        })
        .collect();
    items.sort_by_key(|i| i.priority);
    items
}

/// Get root items (no parent) for tree display.
pub fn root_items(store: &Store) -> Vec<&Item> {
    store
        .items
        .values()
        .filter(|item| get_parent(store, &item.id).is_none())
        .collect()
}
