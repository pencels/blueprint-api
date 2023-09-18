use serde::{Deserialize, Serialize};

use crate::db::DateTime;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssetPack {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub last_modified: DateTime,
    pub version: String,
}
