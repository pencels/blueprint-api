use std::collections::HashMap;

use futures::Stream;
use image::{
    imageops::{self, resize, FilterType},
    GenericImage, RgbaImage,
};
use imageproc::geometric_transformations::{rotate, rotate_about_center, Interpolation};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

mod db {
    use serde::{Deserialize, Serialize};

    pub struct Template {
        layers: Vec<Layer>,
        canvas_size: (u32, u32),
    }

    pub struct Layer {
        source: Source,
        transform: Transform,
        blend_mode: BlendMode,
        opacity: f32,
    }

    pub enum Source {
        Alias(String),
        Raw(String),
    }

    #[derive(Debug, Serialize, Deserialize, Clone, Copy)]
    #[serde(transparent)]
    pub struct Scale(f32);

    impl Default for Scale {
        fn default() -> Self {
            Self(1.0)
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone, Copy, Default)]
    #[serde(transparent)]
    pub struct Degrees(f32);

    #[derive(Debug, Serialize, Deserialize, Clone, Copy, Default)]
    pub struct Transform {
        /// The `(x, y)` offset in pixels.
        #[serde(default)]
        offset: (i64, i64),

        /// The scale as a floating point value, where a value of 1 indicates no scaling.
        #[serde(default)]
        scale: Scale,

        /// The rotation as degrees clockwise.
        #[serde(default)]
        rotate: Degrees,
    }

    pub enum BlendMode {
        Normal,
        Multiply,
        Overlay,
    }
}

pub struct Template {
    pub layers: Vec<Layer>,
    pub canvas_size: (u32, u32),
}

pub struct Layer {
    pub image: RgbaImage,
    pub transform: Transform,
    pub blend_mode: BlendMode,
    pub opacity: f32,
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

pub enum BlendMode {
    Normal,
    Multiply,
    Overlay,
}

fn copy_to_center(src: &RgbaImage, dest: &mut RgbaImage) {
    let (sx, sy) = (src.width() / 2, src.height() / 2);
    let (dx, dy) = (dest.width() / 2, dest.height() / 2);
    let (dx, dy) = (dx as i64, dy as i64);
    let (sx, sy) = (sx as i64, sy as i64);
    imageops::overlay(dest, src, dx - sx, dy - sy)
}

/// Rotates the image about its center, expanding the image to preserve image data.
fn rot(image: &RgbaImage, degrees: Degrees) -> RgbaImage {
    let longest_dim = (image.width() as f64).hypot(image.height() as f64) as u32;
    let mut new_image = RgbaImage::new(longest_dim, longest_dim);

    copy_to_center(image, &mut new_image);

    rotate_about_center(
        &new_image,
        degrees.0.to_radians(),
        Interpolation::Bicubic,
        image::Rgba([0, 0, 0, 0]),
    )
}

fn scale(image: &RgbaImage, scale: Scale) -> RgbaImage {
    let nw = (image.width() as f32 * scale.0) as u32;
    let nh = (image.height() as f32 * scale.0) as u32;

    resize(image, nw, nh, FilterType::Lanczos3)
}

impl Template {
    pub fn apply(&self) -> Result<RgbaImage, Box<dyn std::error::Error>> {
        let (w, h) = (self.canvas_size.0, self.canvas_size.1);
        let mut canvas = RgbaImage::new(w, h);
        for pixel in canvas.pixels_mut() {
            pixel[3] = 0;
        }

        for layer_spec in &self.layers {
            let layer = scale(&layer_spec.image, layer_spec.transform.scale);
            let mut layer = rot(&layer, layer_spec.transform.rotate);
            for pixel in layer.pixels_mut() {
                pixel[3] = (pixel[3] as f32 * layer_spec.opacity) as u8;
            }
            // Need additional offsets to recenter after rotation happened
            let (lw, lh) = (layer.width() as i64, layer.height() as i64);
            let (cx, cy) = ((w as i64 / 2) - (lw / 2), (h as i64 / 2) - (lh / 2));
            imageops::overlay(
                &mut canvas,
                &layer,
                layer_spec.transform.offset.0 + cx,
                layer_spec.transform.offset.1 + cy,
            );
        }

        Ok(canvas)
    }
}

fn prod(aliases: HashMap<String, Vec<String>>) {
    let (keys, values): (Vec<_>, Vec<Vec<_>>) = aliases.into_iter().unzip();
    let result = values.iter().multi_cartesian_product();

    for instance in result {
        println!("{:?}", keys.iter().zip(instance.iter()).collect::<Vec<_>>());
    }
}

#[cfg(test)]
mod test {
    use image::ImageFormat;

    use super::*;

    #[test]
    fn prod() {
        let mut aliases = HashMap::new();
        aliases.insert(
            String::from("a"),
            vec![String::from("x"), String::from("y")],
        );
        aliases.insert(String::from("b"), vec![String::from("m")]);
        aliases.insert(
            String::from("c"),
            vec![String::from("n"), String::from("o")],
        );
        super::prod(aliases);
    }
}
