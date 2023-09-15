use serde::{Deserialize, Serialize};

use super::DateTime;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompositorRun {
    #[serde(rename = "PartitionKey")]
    pub partition_key: String,
    #[serde(rename = "RowKey")]
    pub row_key: String,
    #[serde(
        rename = "created@odata.type",
        serialize_with = "super::edm_datetime",
        skip_deserializing
    )]
    pub _created_tag: (),
    pub created: DateTime,
    pub status: CompositorRunStatus,
    pub progress: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum CompositorRunStatus {
    Running,
    Succeeded,
    Failed,
}

impl From<CompositorRun> for crate::models::CompositorRun {
    fn from(value: CompositorRun) -> Self {
        Self {
            id: value.row_key,
            created: value.created,
            progress: value.progress,
            status: value.status,
        }
    }
}

impl From<crate::models::CompositorRun> for CompositorRun {
    fn from(value: crate::models::CompositorRun) -> Self {
        Self {
            partition_key: value.id.clone(),
            row_key: value.id,
            _created_tag: (),
            created: value.created,
            status: value.status,
            progress: value.progress,
        }
    }
}
