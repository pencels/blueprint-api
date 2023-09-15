use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Template {
    pub aliases: HashMap<String, Vec<String>>,
    pub layers: Vec<Layer>,
    pub canvas_size: (u32, u32),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Layer {
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default)]
    pub transform: Transform,
    #[serde(default)]
    pub blend_mode: BlendMode,
    #[serde(default)]
    pub opacity: Opacity,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(transparent)]
pub struct Opacity(pub f32);

impl Default for Opacity {
    fn default() -> Self {
        Self(1.0)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(transparent)]
pub struct Scale(pub f32);

impl Default for Scale {
    fn default() -> Self {
        Self(1.0)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default)]
#[serde(transparent)]
pub struct Degrees(pub f32);

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default)]
pub struct Transform {
    /// The `(x, y)` offset in pixels.
    #[serde(default)]
    pub offset: (i64, i64),

    /// The scale as a floating point value, where a value of 1 indicates no scaling.
    #[serde(default)]
    pub scale: Scale,

    /// The rotation as degrees clockwise.
    #[serde(default)]
    pub rotate: Degrees,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum BlendMode {
    Normal,
    Multiply,
    Overlay,
}

impl Default for BlendMode {
    fn default() -> Self {
        BlendMode::Normal
    }
}
