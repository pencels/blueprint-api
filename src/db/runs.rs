use bson::Bson;
use serde::{Deserialize, Serialize};

use super::DateTime;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompositorRun {
    #[serde(rename = "_id", skip_serializing)]
    pub id: bson::oid::ObjectId,
    pub created: DateTime,
    pub status: CompositorRunStatus,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum CompositorRunStatus {
    Running,
    Succeeded,
    Failed,
}

impl From<CompositorRunStatus> for Bson {
    fn from(value: CompositorRunStatus) -> Self {
        match value {
            CompositorRunStatus::Running => Bson::String("running".to_string()),
            CompositorRunStatus::Succeeded => Bson::String("succeeded".to_string()),
            CompositorRunStatus::Failed => Bson::String("failed".to_string()),
        }
    }
}

impl From<CompositorRun> for crate::models::CompositorRun {
    fn from(value: CompositorRun) -> Self {
        Self {
            id: value.id.to_string(),
            created: value.created,
            status: value.status,
        }
    }
}

impl From<crate::models::CompositorRun> for CompositorRun {
    fn from(value: crate::models::CompositorRun) -> Self {
        Self {
            id: Default::default(),
            created: value.created,
            status: value.status,
        }
    }
}
