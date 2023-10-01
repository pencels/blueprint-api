use serde::{Deserialize, Serialize};

use crate::models;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Blueprint {
    #[serde(
        rename = "_id",
        with = "bson::serde_helpers::hex_string_as_object_id",
        skip_serializing
    )]
    pub id: String,
    #[serde(with = "bson::serde_helpers::hex_string_as_object_id")]
    pub author: String,
    pub name: String,
    pub templates: String,
}

impl From<models::Blueprint> for Blueprint {
    fn from(value: models::Blueprint) -> Self {
        Self {
            author: value.author,
            id: value.id,
            name: value.name,
            templates: serde_json::to_string(&value.templates).unwrap(),
        }
    }
}

impl From<Blueprint> for models::Blueprint {
    fn from(value: Blueprint) -> Self {
        Self {
            author: value.author,
            id: value.id,
            name: value.name,
            templates: serde_json::from_str(&value.templates).unwrap(),
        }
    }
}
