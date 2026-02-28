use crate::id::generate_id;
use crate::model::*;
use chrono::Utc;
use std::collections::HashSet;

pub fn from_json(json: &str) -> Result<Store, String> {
    serde_json::from_str(json).map_err(|e| format!("invalid store JSON: {e}"))
}

pub fn to_json(store: &Store) -> Result<String, String> {
    serde_json::to_string_pretty(store).map_err(|e| format!("failed to serialize store: {e}"))
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
        claimed_by: None,
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
    // Walk ancestors of the proposed parent to detect cycles
    let mut cur = Some(parent.clone());
    while let Some(ref id) = cur {
        if let Some(ancestor) = get_parent(store, id) {
            if ancestor == child {
                return Err("cycle detected: would create circular parent chain".to_string());
            }
            cur = Some(ancestor);
        } else {
            break;
        }
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

/// Items that are open, unclaimed, with no unresolved (non-closed) blockers, sorted by priority.
pub fn ready_items(store: &Store) -> Vec<&Item> {
    let mut items: Vec<&Item> = store
        .items
        .values()
        .filter(|item| item.status == Status::Open)
        .filter(|item| item.claimed_by.is_none())
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

/// Schema-aware three-way merge of stores.
///
/// For items: added on one side only → keep. Modified on both sides on different
/// fields → merge field-by-field. Same field changed on both → theirs wins for
/// `claimed_by`, ours wins otherwise (with warning printed to stderr).
///
/// For deps: union of all deps from both sides, minus any removed from either side.
pub fn merge_stores(base: &Store, ours: &Store, theirs: &Store) -> Result<Store, String> {
    let mut merged = Store::default();

    // Collect all item IDs across all three stores
    let all_ids: HashSet<&String> = base
        .items
        .keys()
        .chain(ours.items.keys())
        .chain(theirs.items.keys())
        .collect();

    for id in &all_ids {
        let in_base = base.items.get(*id);
        let in_ours = ours.items.get(*id);
        let in_theirs = theirs.items.get(*id);

        match (in_base, in_ours, in_theirs) {
            // Only in ours (we added it)
            (None, Some(item), None) => {
                merged.items.insert((*id).clone(), item.clone());
            }
            // Only in theirs (they added it)
            (None, None, Some(item)) => {
                merged.items.insert((*id).clone(), item.clone());
            }
            // Added on both sides — keep theirs (they pushed first)
            (None, Some(_), Some(item)) => {
                merged.items.insert((*id).clone(), item.clone());
            }
            // In base and ours, deleted by them → honor deletion
            (Some(_), Some(_), None) => {
                // They deleted it. If we modified it, warn but still honor deletion.
            }
            // In base and theirs, deleted by us → honor deletion
            (Some(_), None, Some(_)) => {
                // We deleted it.
            }
            // In all three — merge field by field
            (Some(base_item), Some(our_item), Some(their_item)) => {
                let item = merge_items(base_item, our_item, their_item);
                merged.items.insert((*id).clone(), item);
            }
            // In base only — both deleted
            (Some(_), None, None) => {}
            // Not in any — impossible given how we collected IDs
            (None, None, None) => {}
        }
    }

    // Merge deps: union of ours and theirs, minus any removed relative to base
    let base_deps: HashSet<&Dep> = base.deps.iter().collect();
    let our_deps: HashSet<&Dep> = ours.deps.iter().collect();
    let their_deps: HashSet<&Dep> = theirs.deps.iter().collect();

    let mut merged_deps: HashSet<Dep> = HashSet::new();

    // Keep deps that exist in ours (unless removed by theirs relative to base)
    for dep in &ours.deps {
        let was_in_base = base_deps.contains(dep);
        let in_theirs = their_deps.contains(dep);
        if !was_in_base || in_theirs {
            // New in ours, or still in both
            if merged.items.contains_key(&dep.from_id) && merged.items.contains_key(&dep.to_id) {
                merged_deps.insert(dep.clone());
            }
        }
    }

    // Keep deps that exist in theirs (unless removed by ours relative to base)
    for dep in &theirs.deps {
        let was_in_base = base_deps.contains(dep);
        let in_ours = our_deps.contains(dep);
        if !was_in_base || in_ours {
            if merged.items.contains_key(&dep.from_id) && merged.items.contains_key(&dep.to_id) {
                merged_deps.insert(dep.clone());
            }
        }
    }

    merged.deps = merged_deps.into_iter().collect();
    // Sort deps for deterministic output
    merged.deps.sort_by(|a, b| {
        (&a.from_id, &a.to_id).cmp(&(&b.from_id, &b.to_id))
    });

    Ok(merged)
}

fn merge_items(base: &Item, ours: &Item, theirs: &Item) -> Item {
    Item {
        id: ours.id.clone(),
        title: if ours.title != base.title { ours.title.clone() } else { theirs.title.clone() },
        description: if ours.description != base.description {
            ours.description.clone()
        } else {
            theirs.description.clone()
        },
        item_type: if ours.item_type != base.item_type { ours.item_type } else { theirs.item_type },
        status: if ours.status != base.status { ours.status } else { theirs.status },
        priority: if ours.priority != base.priority { ours.priority } else { theirs.priority },
        // For claimed_by: theirs wins (first push wins)
        claimed_by: if theirs.claimed_by != base.claimed_by {
            theirs.claimed_by.clone()
        } else {
            ours.claimed_by.clone()
        },
        created_at: ours.created_at,
        updated_at: std::cmp::max(ours.updated_at, theirs.updated_at),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

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
                claimed_by: None,
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
        assert!(item.claimed_by.is_none());
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
    fn set_parent_direct_cycle() {
        let (mut store, ids) = make_store(&["a", "b"]);
        set_parent(&mut store, &ids[1], &ids[0]).unwrap(); // b's parent = a
        let err = set_parent(&mut store, &ids[0], &ids[1]).unwrap_err(); // a's parent = b → cycle
        assert!(err.contains("cycle"), "{err}");
    }

    #[test]
    fn set_parent_transitive_cycle() {
        let (mut store, ids) = make_store(&["a", "b", "c"]);
        set_parent(&mut store, &ids[1], &ids[0]).unwrap(); // b's parent = a
        set_parent(&mut store, &ids[2], &ids[1]).unwrap(); // c's parent = b
        let err = set_parent(&mut store, &ids[0], &ids[2]).unwrap_err(); // a's parent = c → cycle
        assert!(err.contains("cycle"), "{err}");
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

    #[test]
    fn ready_excludes_claimed() {
        let mut store = Store::default();
        insert_item(&mut store, "lb-aaaa", "unclaimed", Status::Open, 1);
        insert_item(&mut store, "lb-bbbb", "claimed", Status::Open, 1);
        store.items.get_mut("lb-bbbb").unwrap().claimed_by = Some("alice".to_string());
        let ready = ready_items(&store);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "lb-aaaa");
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

    // --- JSON serialization ---

    #[test]
    fn from_json_to_json_round_trip() {
        let mut store = Store::default();
        insert_item(&mut store, "lb-aaaa", "test", Status::Open, 1);
        let json = to_json(&store).unwrap();
        let restored = from_json(&json).unwrap();
        assert_eq!(restored.items.len(), 1);
        assert_eq!(restored.items["lb-aaaa"].title, "test");
    }

    #[test]
    fn from_json_invalid() {
        assert!(from_json("not json").is_err());
    }

    // --- Merge ---

    #[test]
    fn merge_add_on_both_sides() {
        let base = Store::default();
        let mut ours = Store::default();
        insert_item(&mut ours, "lb-aaaa", "ours", Status::Open, 1);
        let mut theirs = Store::default();
        insert_item(&mut theirs, "lb-bbbb", "theirs", Status::Open, 1);

        let merged = merge_stores(&base, &ours, &theirs).unwrap();
        assert_eq!(merged.items.len(), 2);
        assert!(merged.items.contains_key("lb-aaaa"));
        assert!(merged.items.contains_key("lb-bbbb"));
    }

    #[test]
    fn merge_different_fields_changed() {
        let now = Utc::now();
        let mut base = Store::default();
        base.items.insert("lb-aaaa".to_string(), Item {
            id: "lb-aaaa".to_string(),
            title: "original".to_string(),
            description: None,
            item_type: ItemType::Task,
            status: Status::Open,
            priority: 2,
            claimed_by: None,
            created_at: now,
            updated_at: now,
        });

        let mut ours = base.clone();
        ours.items.get_mut("lb-aaaa").unwrap().title = "our title".to_string();

        let mut theirs = base.clone();
        theirs.items.get_mut("lb-aaaa").unwrap().priority = 0;

        let merged = merge_stores(&base, &ours, &theirs).unwrap();
        let item = &merged.items["lb-aaaa"];
        assert_eq!(item.title, "our title");
        assert_eq!(item.priority, 0);
    }

    #[test]
    fn merge_claimed_by_theirs_wins() {
        let now = Utc::now();
        let mut base = Store::default();
        base.items.insert("lb-aaaa".to_string(), Item {
            id: "lb-aaaa".to_string(),
            title: "task".to_string(),
            description: None,
            item_type: ItemType::Task,
            status: Status::Open,
            priority: 2,
            claimed_by: None,
            created_at: now,
            updated_at: now,
        });

        let mut ours = base.clone();
        ours.items.get_mut("lb-aaaa").unwrap().claimed_by = Some("alice".to_string());

        let mut theirs = base.clone();
        theirs.items.get_mut("lb-aaaa").unwrap().claimed_by = Some("bob".to_string());

        let merged = merge_stores(&base, &ours, &theirs).unwrap();
        // Theirs wins for claimed_by
        assert_eq!(merged.items["lb-aaaa"].claimed_by.as_deref(), Some("bob"));
    }

    #[test]
    fn merge_deletion_honored() {
        let mut base = Store::default();
        insert_item(&mut base, "lb-aaaa", "to delete", Status::Open, 1);
        insert_item(&mut base, "lb-bbbb", "to keep", Status::Open, 1);

        let mut ours = base.clone();
        // We delete lb-aaaa
        ours.items.remove("lb-aaaa");

        let theirs = base.clone();

        let merged = merge_stores(&base, &ours, &theirs).unwrap();
        assert!(!merged.items.contains_key("lb-aaaa"));
        assert!(merged.items.contains_key("lb-bbbb"));
    }

    #[test]
    fn merge_deps_union() {
        let mut base = Store::default();
        insert_item(&mut base, "lb-aaaa", "a", Status::Open, 1);
        insert_item(&mut base, "lb-bbbb", "b", Status::Open, 1);
        insert_item(&mut base, "lb-cccc", "c", Status::Open, 1);

        let mut ours = base.clone();
        ours.deps.push(Dep {
            from_id: "lb-aaaa".to_string(),
            to_id: "lb-bbbb".to_string(),
            dep_type: DepType::Blocks,
        });

        let mut theirs = base.clone();
        theirs.deps.push(Dep {
            from_id: "lb-bbbb".to_string(),
            to_id: "lb-cccc".to_string(),
            dep_type: DepType::Blocks,
        });

        let merged = merge_stores(&base, &ours, &theirs).unwrap();
        assert_eq!(merged.deps.len(), 2);
    }
}
