use serde::{Deserialize, Serialize};

use crate::db::DateTime;

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[repr(u32)]
pub enum CompositorRunStatus {
    Pending = 0,
    Running = 1,
    Succeeded = 2,
    Failed = 3,
}

impl TryFrom<u32> for CompositorRunStatus {
    type Error = &'static str;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        use CompositorRunStatus::*;
        Ok(match value {
            0 => Pending,
            1 => Running,
            2 => Succeeded,
            3 => Failed,
            _ => return Err("invalid status"),
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompositorRun {
    pub id: String,
    pub created: DateTime,
    pub status: CompositorRunStatus,
    pub author: String,
}
