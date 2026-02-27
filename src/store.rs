use crate::id::generate_id;
use crate::model::*;
use chrono::Utc;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const STORE_DIR: &str = ".litebrite";
const STORE_FILE: &str = ".litebrite/store.json";

fn dir_path_in(base: &Path) -> PathBuf {
    base.join(STORE_DIR)
}

fn store_path_in(base: &Path) -> PathBuf {
    base.join(STORE_FILE)
}

pub fn init() -> io::Result<()> {
    init_in(Path::new("."))
}

pub fn init_in(base: &Path) -> io::Result<()> {
    let dir = dir_path_in(base);
    if dir.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            ".litebrite/ already exists",
        ));
    }
    fs::create_dir(&dir)?;
    save_in(base, &Store::default())
}

pub fn load() -> io::Result<Store> {
    load_in(Path::new("."))
}

pub fn load_in(base: &Path) -> io::Result<Store> {
    let path = store_path_in(base);
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
    save_in(Path::new("."), store)
}

pub fn save_in(base: &Path, store: &Store) -> io::Result<()> {
    let path = store_path_in(base);
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::TempDir;

    /// Helper: build a Store with items by short name, returning the generated IDs.
    fn make_store(titles: &[&str]) -> (Store, Vec<String>) {
        let mut store = Store::default();
        let mut ids = Vec::new();
        for title in titles {
            let id = create_item(
                &mut store,
                title.to_string(),
                ItemType::Task,
                2,
                None,
                None,
            )
            .unwrap();
            ids.push(id);
        }
        (store, ids)
    }

    fn insert_item(store: &mut Store, id: &str, title: &str, status: Status, priority: u8) {
        let now = Utc::now();
        store.items.insert(
            id.to_string(),
            Item {
                id: id.to_string(),
                title: title.to_string(),
                description: None,
                item_type: ItemType::Task,
                status,
                priority,
                created_at: now,
                updated_at: now,
            },
        );
    }

    // --- Prefix resolution ---

    #[test]
    fn resolve_exact_match() {
        let (store, ids) = make_store(&["alpha"]);
        assert_eq!(resolve_id(&store, &ids[0]).unwrap(), ids[0]);
    }

    #[test]
    fn resolve_unique_prefix() {
        let mut store = Store::default();
        insert_item(&mut store, "lb-aaaa", "A", Status::Open, 1);
        insert_item(&mut store, "lb-bbbb", "B", Status::Open, 1);
        assert_eq!(resolve_id(&store, "lb-a").unwrap(), "lb-aaaa");
    }

    #[test]
    fn resolve_ambiguous_prefix() {
        let mut store = Store::default();
        insert_item(&mut store, "lb-ab01", "A", Status::Open, 1);
        insert_item(&mut store, "lb-ab02", "B", Status::Open, 1);
        let err = resolve_id(&store, "lb-ab").unwrap_err();
        assert!(err.contains("ambiguous"), "{err}");
    }

    #[test]
    fn resolve_no_match() {
        let store = Store::default();
        let err = resolve_id(&store, "lb-zzzz").unwrap_err();
        assert!(err.contains("no item"), "{err}");
    }

    // --- CRUD ---

    #[test]
    fn create_item_basic() {
        let mut store = Store::default();
        let id = create_item(
            &mut store,
            "My task".to_string(),
            ItemType::Task,
            1,
            Some("desc".to_string()),
            None,
        )
        .unwrap();
        assert!(id.starts_with("lb-"));
        let item = &store.items[&id];
        assert_eq!(item.title, "My task");
        assert_eq!(item.item_type, ItemType::Task);
        assert_eq!(item.status, Status::Open);
        assert_eq!(item.priority, 1);
        assert_eq!(item.description.as_deref(), Some("desc"));
    }

    #[test]
    fn create_item_with_parent() {
        let (mut store, ids) = make_store(&["parent"]);
        let child_id = create_item(
            &mut store,
            "child".to_string(),
            ItemType::Task,
            2,
            None,
            Some(ids[0].clone()),
        )
        .unwrap();
        assert_eq!(get_parent(&store, &child_id), Some(ids[0].clone()));
    }

    #[test]
    fn create_item_nonexistent_parent() {
        let mut store = Store::default();
        let err = create_item(
            &mut store,
            "orphan".to_string(),
            ItemType::Task,
            2,
            None,
            Some("lb-nope".to_string()),
        )
        .unwrap_err();
        assert!(err.contains("no item"), "{err}");
    }

    #[test]
    fn delete_item_basic() {
        let (mut store, ids) = make_store(&["doomed"]);
        delete_item(&mut store, &ids[0]).unwrap();
        assert!(store.items.is_empty());
    }

    #[test]
    fn delete_item_removes_deps() {
        let (mut store, ids) = make_store(&["a", "b"]);
        add_blocking_dep(&mut store, &ids[0], &ids[1]).unwrap();
        assert_eq!(store.deps.len(), 1);
        delete_item(&mut store, &ids[0]).unwrap();
        assert!(store.deps.is_empty());
    }

    #[test]
    fn delete_nonexistent() {
        let store = &mut Store::default();
        assert!(delete_item(store, "lb-nope").is_err());
    }

    // --- Parent/child ---

    #[test]
    fn set_parent_basic() {
        let (mut store, ids) = make_store(&["parent", "child"]);
        set_parent(&mut store, &ids[1], &ids[0]).unwrap();
        assert_eq!(get_parent(&store, &ids[1]), Some(ids[0].clone()));
    }

    #[test]
    fn set_parent_replaces_existing() {
        let (mut store, ids) = make_store(&["p1", "p2", "child"]);
        set_parent(&mut store, &ids[2], &ids[0]).unwrap();
        assert_eq!(get_parent(&store, &ids[2]), Some(ids[0].clone()));

        set_parent(&mut store, &ids[2], &ids[1]).unwrap();
        assert_eq!(get_parent(&store, &ids[2]), Some(ids[1].clone()));
        // Only one parent dep should remain for the child
        let parent_deps: Vec<_> = store
            .deps
            .iter()
            .filter(|d| d.from_id == ids[2] && d.dep_type == DepType::Parent)
            .collect();
        assert_eq!(parent_deps.len(), 1);
    }

    #[test]
    fn set_parent_self_reference() {
        let (mut store, ids) = make_store(&["lonely"]);
        let err = set_parent(&mut store, &ids[0], &ids[0]).unwrap_err();
        assert!(err.contains("own parent"), "{err}");
    }

    #[test]
    fn get_parent_none() {
        let (store, ids) = make_store(&["orphan"]);
        assert_eq!(get_parent(&store, &ids[0]), None);
    }

    #[test]
    fn get_children_basic() {
        let (mut store, ids) = make_store(&["parent", "c1", "c2"]);
        set_parent(&mut store, &ids[1], &ids[0]).unwrap();
        set_parent(&mut store, &ids[2], &ids[0]).unwrap();
        let mut children = get_children(&store, &ids[0]);
        children.sort();
        let mut expected = vec![ids[1].clone(), ids[2].clone()];
        expected.sort();
        assert_eq!(children, expected);
    }

    // --- Blocking deps ---

    #[test]
    fn add_blocking_dep_basic() {
        let (mut store, ids) = make_store(&["blocker", "blocked"]);
        add_blocking_dep(&mut store, &ids[0], &ids[1]).unwrap();
        assert_eq!(get_blockers(&store, &ids[1]), vec![ids[0].clone()]);
        assert_eq!(get_blocking(&store, &ids[0]), vec![ids[1].clone()]);
    }

    #[test]
    fn add_blocking_dep_self_block() {
        let (mut store, ids) = make_store(&["self"]);
        let err = add_blocking_dep(&mut store, &ids[0], &ids[0]).unwrap_err();
        assert!(err.contains("itself"), "{err}");
    }

    #[test]
    fn add_blocking_dep_duplicate() {
        let (mut store, ids) = make_store(&["a", "b"]);
        add_blocking_dep(&mut store, &ids[0], &ids[1]).unwrap();
        let err = add_blocking_dep(&mut store, &ids[0], &ids[1]).unwrap_err();
        assert!(err.contains("already exists"), "{err}");
    }

    #[test]
    fn remove_dep_basic() {
        let (mut store, ids) = make_store(&["a", "b"]);
        add_blocking_dep(&mut store, &ids[0], &ids[1]).unwrap();
        remove_dep(&mut store, &ids[0], &ids[1]).unwrap();
        assert!(get_blockers(&store, &ids[1]).is_empty());
    }

    #[test]
    fn remove_dep_nonexistent() {
        let (mut store, ids) = make_store(&["a", "b"]);
        let err = remove_dep(&mut store, &ids[0], &ids[1]).unwrap_err();
        assert!(err.contains("no dependency"), "{err}");
    }

    #[test]
    fn get_blockers_and_blocking() {
        let (mut store, ids) = make_store(&["a", "b", "c"]);
        add_blocking_dep(&mut store, &ids[0], &ids[2]).unwrap();
        add_blocking_dep(&mut store, &ids[1], &ids[2]).unwrap();

        let mut blockers = get_blockers(&store, &ids[2]);
        blockers.sort();
        let mut expected = vec![ids[0].clone(), ids[1].clone()];
        expected.sort();
        assert_eq!(blockers, expected);

        assert_eq!(get_blocking(&store, &ids[0]), vec![ids[2].clone()]);
    }

    // --- Ready items ---

    #[test]
    fn ready_open_no_blockers() {
        let mut store = Store::default();
        insert_item(&mut store, "lb-aaaa", "ready", Status::Open, 1);
        let ready = ready_items(&store);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "lb-aaaa");
    }

    #[test]
    fn ready_open_with_unclosed_blocker() {
        let mut store = Store::default();
        insert_item(&mut store, "lb-aaaa", "blocker", Status::Open, 1);
        insert_item(&mut store, "lb-bbbb", "blocked", Status::Open, 1);
        store.deps.push(Dep {
            from_id: "lb-aaaa".to_string(),
            to_id: "lb-bbbb".to_string(),
            dep_type: DepType::Blocks,
        });
        let ready = ready_items(&store);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "lb-aaaa");
    }

    #[test]
    fn ready_open_with_closed_blocker() {
        let mut store = Store::default();
        insert_item(&mut store, "lb-aaaa", "blocker", Status::Closed, 1);
        insert_item(&mut store, "lb-bbbb", "blocked", Status::Open, 1);
        store.deps.push(Dep {
            from_id: "lb-aaaa".to_string(),
            to_id: "lb-bbbb".to_string(),
            dep_type: DepType::Blocks,
        });
        let ready = ready_items(&store);
        // lb-bbbb is ready (blocker is closed); lb-aaaa is closed so not ready
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "lb-bbbb");
    }

    #[test]
    fn ready_closed_never_ready() {
        let mut store = Store::default();
        insert_item(&mut store, "lb-aaaa", "done", Status::Closed, 1);
        assert!(ready_items(&store).is_empty());
    }

    #[test]
    fn ready_sorted_by_priority() {
        let mut store = Store::default();
        insert_item(&mut store, "lb-cccc", "low", Status::Open, 3);
        insert_item(&mut store, "lb-aaaa", "high", Status::Open, 0);
        insert_item(&mut store, "lb-bbbb", "mid", Status::Open, 1);
        let ready = ready_items(&store);
        let priorities: Vec<u8> = ready.iter().map(|i| i.priority).collect();
        assert_eq!(priorities, vec![0, 1, 3]);
    }

    // --- Root items ---

    #[test]
    fn root_items_no_parent() {
        let (store, ids) = make_store(&["root1", "root2"]);
        let roots: Vec<&str> = root_items(&store).iter().map(|i| i.id.as_str()).collect();
        assert!(roots.contains(&ids[0].as_str()));
        assert!(roots.contains(&ids[1].as_str()));
    }

    #[test]
    fn root_items_excludes_children() {
        let (mut store, ids) = make_store(&["parent", "child"]);
        set_parent(&mut store, &ids[1], &ids[0]).unwrap();
        let roots: Vec<&str> = root_items(&store).iter().map(|i| i.id.as_str()).collect();
        assert!(roots.contains(&ids[0].as_str()));
        assert!(!roots.contains(&ids[1].as_str()));
    }

    // --- File I/O ---

    #[test]
    fn init_creates_store() {
        let tmp = TempDir::new().unwrap();
        init_in(tmp.path()).unwrap();

        let path = tmp.path().join(".litebrite/store.json");
        assert!(path.exists());
        let data = std::fs::read_to_string(&path).unwrap();
        let store: Store = serde_json::from_str(&data).unwrap();
        assert!(store.items.is_empty());
        assert!(store.deps.is_empty());
    }

    #[test]
    fn init_existing_dir_errors() {
        let tmp = TempDir::new().unwrap();
        init_in(tmp.path()).unwrap();
        let err = init_in(tmp.path()).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
    }

    #[test]
    fn save_then_load_round_trip() {
        let tmp = TempDir::new().unwrap();
        init_in(tmp.path()).unwrap();

        let mut store = load_in(tmp.path()).unwrap();
        let id = create_item(
            &mut store,
            "persist me".to_string(),
            ItemType::Feature,
            1,
            Some("desc".to_string()),
            None,
        )
        .unwrap();
        save_in(tmp.path(), &store).unwrap();

        let loaded = load_in(tmp.path()).unwrap();
        assert_eq!(loaded.items.len(), 1);
        let item = &loaded.items[&id];
        assert_eq!(item.title, "persist me");
        assert_eq!(item.item_type, ItemType::Feature);
        assert_eq!(item.description.as_deref(), Some("desc"));
    }

    #[test]
    fn load_missing_file_errors() {
        let tmp = TempDir::new().unwrap();
        let err = load_in(tmp.path()).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }
}
