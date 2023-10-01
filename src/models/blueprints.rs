use serde::{Deserialize, Serialize};

use crate::models::Template;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Blueprint {
    pub id: String,
    pub name: String,
    pub author: String,
    pub templates: Vec<Template>,
}
