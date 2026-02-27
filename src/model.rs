use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemType {
    Epic,
    Feature,
    Task,
}

impl fmt::Display for ItemType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ItemType::Epic => write!(f, "epic"),
            ItemType::Feature => write!(f, "feature"),
            ItemType::Task => write!(f, "task"),
        }
    }
}

impl std::str::FromStr for ItemType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "epic" => Ok(ItemType::Epic),
            "feature" => Ok(ItemType::Feature),
            "task" => Ok(ItemType::Task),
            _ => Err(format!("unknown item type: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Open,
    InProgress,
    Blocked,
    Deferred,
    Closed,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Open => write!(f, "open"),
            Status::InProgress => write!(f, "in_progress"),
            Status::Blocked => write!(f, "blocked"),
            Status::Deferred => write!(f, "deferred"),
            Status::Closed => write!(f, "closed"),
        }
    }
}

impl std::str::FromStr for Status {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(Status::Open),
            "in_progress" => Ok(Status::InProgress),
            "blocked" => Ok(Status::Blocked),
            "deferred" => Ok(Status::Deferred),
            "closed" => Ok(Status::Closed),
            _ => Err(format!("unknown status: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepType {
    Parent,
    Blocks,
}

impl fmt::Display for DepType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DepType::Parent => write!(f, "parent"),
            DepType::Blocks => write!(f, "blocks"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub item_type: ItemType,
    pub status: Status,
    pub priority: u8,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Dep {
    pub from_id: String,
    pub to_id: String,
    pub dep_type: DepType,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Store {
    pub items: BTreeMap<String, Item>,
    pub deps: Vec<Dep>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_type_from_str_valid() {
        assert_eq!("epic".parse::<ItemType>().unwrap(), ItemType::Epic);
        assert_eq!("feature".parse::<ItemType>().unwrap(), ItemType::Feature);
        assert_eq!("task".parse::<ItemType>().unwrap(), ItemType::Task);
    }

    #[test]
    fn item_type_from_str_case_insensitive() {
        assert_eq!("EPIC".parse::<ItemType>().unwrap(), ItemType::Epic);
        assert_eq!("Feature".parse::<ItemType>().unwrap(), ItemType::Feature);
        assert_eq!("tAsK".parse::<ItemType>().unwrap(), ItemType::Task);
    }

    #[test]
    fn item_type_from_str_invalid() {
        assert!("bug".parse::<ItemType>().is_err());
        assert!("".parse::<ItemType>().is_err());
    }

    #[test]
    fn status_from_str_valid() {
        assert_eq!("open".parse::<Status>().unwrap(), Status::Open);
        assert_eq!("in_progress".parse::<Status>().unwrap(), Status::InProgress);
        assert_eq!("blocked".parse::<Status>().unwrap(), Status::Blocked);
        assert_eq!("deferred".parse::<Status>().unwrap(), Status::Deferred);
        assert_eq!("closed".parse::<Status>().unwrap(), Status::Closed);
    }

    #[test]
    fn status_from_str_case_insensitive() {
        assert_eq!("OPEN".parse::<Status>().unwrap(), Status::Open);
        assert_eq!("IN_PROGRESS".parse::<Status>().unwrap(), Status::InProgress);
        assert_eq!("Closed".parse::<Status>().unwrap(), Status::Closed);
    }

    #[test]
    fn status_from_str_invalid() {
        assert!("done".parse::<Status>().is_err());
        assert!("".parse::<Status>().is_err());
    }

    #[test]
    fn display_round_trip_item_type() {
        for variant in [ItemType::Epic, ItemType::Feature, ItemType::Task] {
            let s = variant.to_string();
            assert_eq!(s.parse::<ItemType>().unwrap(), variant);
        }
    }

    #[test]
    fn display_round_trip_status() {
        for variant in [
            Status::Open,
            Status::InProgress,
            Status::Blocked,
            Status::Deferred,
            Status::Closed,
        ] {
            let s = variant.to_string();
            assert_eq!(s.parse::<Status>().unwrap(), variant);
        }
    }

    #[test]
    fn store_serde_round_trip() {
        let now = Utc::now();
        let mut store = Store::default();
        store.items.insert(
            "lb-abc1".to_string(),
            Item {
                id: "lb-abc1".to_string(),
                title: "Test item".to_string(),
                description: Some("A description".to_string()),
                item_type: ItemType::Task,
                status: Status::Open,
                priority: 1,
                created_at: now,
                updated_at: now,
            },
        );
        store.deps.push(Dep {
            from_id: "lb-abc1".to_string(),
            to_id: "lb-xyz2".to_string(),
            dep_type: DepType::Parent,
        });

        let json = serde_json::to_string(&store).unwrap();
        let restored: Store = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.items.len(), 1);
        assert_eq!(restored.items["lb-abc1"].title, "Test item");
        assert_eq!(restored.deps.len(), 1);
        assert_eq!(restored.deps[0].dep_type, DepType::Parent);
    }

    #[test]
    fn item_description_none_skipped_in_json() {
        let now = Utc::now();
        let item = Item {
            id: "lb-test".to_string(),
            title: "No desc".to_string(),
            description: None,
            item_type: ItemType::Task,
            status: Status::Open,
            priority: 2,
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(!json.contains("description"));
    }
}
