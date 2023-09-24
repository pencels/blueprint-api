use serde::{Deserialize, Serialize};

use super::DateTime;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompositorRun {
    #[serde(
        rename = "_id",
        with = "bson::serde_helpers::hex_string_as_object_id",
        skip_serializing
    )]
    pub id: String,
    pub created: DateTime,
    pub status: u32,
    #[serde(with = "bson::serde_helpers::hex_string_as_object_id")]
    pub author: String,
}

impl From<CompositorRun> for crate::models::CompositorRun {
    fn from(value: CompositorRun) -> Self {
        Self {
            id: value.id,
            created: value.created,
            status: value.status.try_into().unwrap(),
            author: value.author,
        }
    }
}

impl From<crate::models::CompositorRun> for CompositorRun {
    fn from(value: crate::models::CompositorRun) -> Self {
        Self {
            id: Default::default(),
            created: value.created,
            status: value.status as u32,
            author: value.author,
        }
    }
}
