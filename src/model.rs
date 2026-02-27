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
