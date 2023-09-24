use serde::{Deserialize, Serialize};

use crate::models::Template;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Blueprint {
    pub templates: Vec<Template>,
}
