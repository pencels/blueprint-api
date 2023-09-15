use std::collections::HashMap;
use std::sync::Arc;
use std::{iter, slice};

use azure_data_tables::prelude::TableServiceClient;
use azure_storage_blobs::prelude::BlobServiceClient;
use futures::TryStreamExt;
use image::codecs::png::PngEncoder;
use image::imageops::FilterType;
use image::{imageops, RgbaImage};
use imageproc::geometric_transformations::Interpolation;
use itertools::{Either, Itertools, MultiProduct};

use crate::db::Asset;
use crate::models::{Degrees, Scale, Template};
use crate::util::Result;

use super::image_cache::ImageCache;

#[derive(Debug, Clone)]
pub struct Compositor {
    table_client: TableServiceClient,
    blob_client: BlobServiceClient,
}

impl Compositor {
    pub fn new(table_client: TableServiceClient, blob_client: BlobServiceClient) -> Compositor {
        Compositor {
            table_client,
            blob_client,
        }
    }

    pub async fn apply_template(
        &self,
        template: &Template,
        image_cache: &ImageCache,
        aliases: &HashMap<&str, &String>,
    ) -> Result<RgbaImage> {
        let (w, h) = (template.canvas_size.0, template.canvas_size.1);
        let mut canvas = RgbaImage::new(w, h);
        for pixel in canvas.pixels_mut() {
            pixel[3] = 0;
        }

        for layer_spec in &template.layers {
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

    pub async fn run_template(&self, run_id: &str, mut template: Template) -> Result<()> {
        let output = self.blob_client.container_client("template-output");
        for refs in template.aliases.values_mut() {
            *refs = expand_refs(&self.table_client, refs.iter()).await?;
        }

        let (vals, iter) = iter_alias_binds(&template.aliases);
        for tuple in iter {
            let pairs = vals.clone().into_iter().zip(tuple);
            let aliases = HashMap::from_iter(pairs);

            let image_cache = ImageCache::new(Arc::new(self.blob_client.clone()));

            let result = self
                .apply_template(&template, &image_cache, &aliases)
                .await?;

            let mut buf = Vec::new();
            result.write_with_encoder(PngEncoder::new(&mut buf))?;

            let file_name = match aliases.get(&"fg") {
                Some(id) => {
                    let meta = self
                        .blob_client
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

    imageproc::geometric_transformations::rotate_about_center(
        &new_image,
        degrees.0.to_radians(),
        Interpolation::Bicubic,
        image::Rgba([0, 0, 0, 0]),
    )
}

fn scale(image: &RgbaImage, scale: Scale) -> RgbaImage {
    let nw = (image.width() as f32 * scale.0) as u32;
    let nh = (image.height() as f32 * scale.0) as u32;

    imageops::resize(image, nw, nh, FilterType::Lanczos3)
}

/// Returns an iterator over all the possible alias bindings for the given mapping.
pub fn iter_alias_binds(
    aliases: &HashMap<String, Vec<String>>,
) -> (Vec<&str>, MultiProduct<slice::Iter<String>>) {
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
