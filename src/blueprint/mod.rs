pub mod image_cache;

use std::{collections::HashMap, iter, slice::Iter, sync::Arc};

use crate::{db::Asset, util::Result};
use azure_data_tables::prelude::TableServiceClient;
use azure_storage_blobs::prelude::BlobServiceClient;
use futures::TryStreamExt;
use image::{
    codecs::png::PngEncoder,
    imageops::{self, resize, FilterType},
    RgbaImage,
};
use imageproc::geometric_transformations::{rotate_about_center, Interpolation};
use itertools::{Either, Itertools, MultiProduct};
use serde::{Deserialize, Serialize};

use self::image_cache::ImageCache;

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
    pub async fn apply(
        &self,
        image_cache: &ImageCache,
        aliases: &HashMap<&str, &String>,
    ) -> Result<RgbaImage> {
        let (w, h) = (self.canvas_size.0, self.canvas_size.1);
        let mut canvas = RgbaImage::new(w, h);
        for pixel in canvas.pixels_mut() {
            pixel[3] = 0;
        }

        for layer_spec in &self.layers {
            let asset_id = aliases.get(&layer_spec.reference.as_str()).unwrap();
            let layer = image_cache.inner.get(asset_id.to_string()).await?;
            let layer = scale(&layer, layer_spec.transform.scale);
            let mut layer = rot(&layer, layer_spec.transform.rotate);
            for pixel in layer.pixels_mut() {
                pixel[3] = (pixel[3] as f32 * layer_spec.opacity.0) as u8;
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

/// Returns an iterator over all the possible alias bindings for the given mapping.
pub fn iter_alias_binds(
    aliases: &HashMap<String, Vec<String>>,
) -> (Vec<&str>, MultiProduct<Iter<String>>) {
    let (keys, values): (Vec<_>, Vec<_>) = aliases.iter().unzip();
    let result = values.iter().map(|v| *v).multi_cartesian_product();
    (keys.iter().map(|s| s.as_str()).collect(), result)
}

async fn get_pack_asset_ids(
    tables: &TableServiceClient,
    pack_id: &str,
) -> Result<impl Iterator<Item = String>> {
    let resp: Vec<_> = tables
        .table_client("assets")
        .query()
        .filter(format!("PartitionKey eq '{}'", pack_id))
        .into_stream::<Asset>()
        .try_collect()
        .await?;

    let asset_ids = resp
        .into_iter()
        .flat_map(|res| res.entities)
        .map(|asset| asset.row_key);

    Ok(asset_ids)
}

async fn expand_ref<S>(tables: &TableServiceClient, item: S) -> Result<impl Iterator<Item = String>>
where
    S: AsRef<str>,
{
    let item = item.as_ref();
    let iter = match item.split_once(':') {
        Some(("pack", id)) => Either::Left(get_pack_asset_ids(&tables, id).await?),
        Some((ref_type, _)) => Err(format!("unrecognized reference type: {}", ref_type))?,
        None => Either::Right(iter::once(item.to_string())),
    };
    Ok(iter)
}

async fn expand_refs<S>(
    tables: &TableServiceClient,
    refs: impl Iterator<Item = S>,
) -> Result<Vec<String>>
where
    S: AsRef<str>,
{
    let futs = refs.map(|item| expand_ref(tables, item));

    let mut result = Vec::new();
    for fut in futs {
        let vals = fut.await?;
        result.extend(vals);
    }
    Ok(result)
}

pub async fn run_template(
    tables: &TableServiceClient,
    blobs: &BlobServiceClient,
    run_id: &str,
    mut template: Template,
) -> Result<()> {
    let output = blobs.container_client("template-output");
    for refs in template.aliases.values_mut() {
        *refs = expand_refs(&tables, refs.iter()).await?;
    }

    let (vals, iter) = iter_alias_binds(&template.aliases);
    for tuple in iter {
        let pairs = vals.clone().into_iter().zip(tuple);
        let aliases = HashMap::from_iter(pairs);

        let image_cache = ImageCache::new(Arc::new(blobs.clone()));

        let result = template.apply(&image_cache, &aliases).await?;

        let mut buf = Vec::new();
        result.write_with_encoder(PngEncoder::new(&mut buf))?;

        let file_name = match aliases.get(&"fg") {
            Some(id) => {
                let meta = blobs
                    .container_client("assets")
                    .blob_client(*id)
                    .get_metadata()
                    .await?;
                meta.metadata
                    .get("file_name")
                    .and_then(|n| String::from_utf8(n.into()).ok())
                    .unwrap_or_else(|| format!("{}.png", id))
            }
            None => panic!("erm"),
        };
        let relative_blob_name = run_id.to_string() + "/" + &file_name;
        output
            .blob_client(relative_blob_name)
            .put_block_blob(buf)
            .await?;
    }

    Ok(())
}
