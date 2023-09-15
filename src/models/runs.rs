use serde::{Deserialize, Serialize};

use crate::db::{CompositorRunStatus, DateTime};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompositorRun {
    pub id: String,
    pub created: DateTime,
    pub status: CompositorRunStatus,
    pub progress: u32,
}
