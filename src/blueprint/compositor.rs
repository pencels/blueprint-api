use std::borrow::Borrow;
use std::collections::HashMap;
use std::slice;
use std::sync::Arc;

use azure_storage_blobs::prelude::BlobServiceClient;
use futures::TryStreamExt;
use image::codecs::png::PngEncoder;
use image::imageops::FilterType;
use image::{imageops, RgbaImage};
use imageproc::geometric_transformations::Interpolation;
use itertools::{Itertools, MultiProduct};
use mongodb::bson::doc;

use crate::db::CompositorRun;
use crate::models::{CompositorRunStatus, Degrees, Scale, Template};
use crate::util::Result;

use super::image_cache::ImageCache;

#[derive(Debug, Clone)]
pub struct Compositor {
    db: mongodb::Client,
    blob_client: BlobServiceClient,
}

impl Compositor {
    pub fn new(db: mongodb::Client, blob_client: BlobServiceClient) -> Compositor {
        Compositor { db, blob_client }
    }

    pub async fn apply_template_instance(
        &self,
        template: &Template,
        image_cache: &ImageCache,
        aliases: &HashMap<&String, &(&str, String)>,
    ) -> Result<RgbaImage> {
        let (w, h) = (template.canvas_size.0, template.canvas_size.1);
        let mut canvas = RgbaImage::new(w, h);
        for pixel in canvas.pixels_mut() {
            pixel[3] = 0;
        }

        for layer_spec in &template.layers {
            let (pack, path) = aliases.get(&&layer_spec.reference).unwrap();
            let layer = image_cache
                .inner
                .get((pack.to_string(), path.to_string()))
                .await?;
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

    pub async fn run_template(&self, run_id: &String, mut template: Template) -> Result<()> {
        let runs_coll = self
            .db
            .default_database()
            .unwrap()
            .collection::<CompositorRun>("runs");
        let output = self.blob_client.container_client("template-output");

        template.normalize_use_refs();
        let mut expanded_refs = HashMap::new();
        for (alias, refs) in template.aliases.iter() {
            expanded_refs.insert(alias, self.expand_refs(refs.iter()).await?);
        }

        let (vals, iter) = iter_alias_binds(&expanded_refs);
        for tuple in iter {
            let pairs = vals.iter().zip(tuple).map(|(k, v)| (*k, v));
            let aliases = HashMap::from_iter(pairs);

            let image_cache = ImageCache::new(Arc::new(self.blob_client.clone()));

            let result = self
                .apply_template_instance(&template, &image_cache, &aliases)
                .await?;

            let mut buf = Vec::new();
            result.write_with_encoder(PngEncoder::new(&mut buf))?;

            let file_name = match aliases.get(&"$_fg".to_string()) {
                Some((_, path)) => path,
                None => panic!("erm"),
            };
            let relative_blob_name = run_id.to_string() + "/" + &file_name;
            output
                .blob_client(relative_blob_name)
                .put_block_blob(buf)
                .await?;

            let modifications = doc! {
                "$set": {
                    "status": CompositorRunStatus::Running as u32,
                }
            };

            runs_coll
                .update_one(doc! { "_id": &run_id }, modifications, None)
                .await?;
        }

        let modifications = doc! {
            "$set": {
                "status": CompositorRunStatus::Succeeded as u32,
            }
        };
        runs_coll
            .update_one(doc! { "_id": &run_id }, modifications, None)
            .await?;
        Ok(())
    }

    async fn match_paths_to_glob<'a>(
        &self,
        pack_id: &'a str,
        glob: &str,
    ) -> Result<impl IntoIterator<Item = (&'a str, String)>> {
        let matcher = globset::Glob::new(glob)?.compile_matcher();
        let mut blobs = self
            .blob_client
            .container_client(format!("pack-{}", pack_id))
            .list_blobs()
            .into_stream();

        let mut results = Vec::new();
        while let Some(page) = blobs.try_next().await? {
            for blob in page.blobs.blobs() {
                if matcher.is_match(&blob.name) {
                    results.push((pack_id, blob.name.to_string()));
                }
            }
        }

        Ok(results)
    }

    async fn expand_ref<'a, S>(
        &self,
        item: &'a S,
    ) -> Result<impl IntoIterator<Item = (&'a str, String)>>
    where
        S: Borrow<str> + 'a,
    {
        let iter = match item.borrow().split_once(':') {
            Some((slug, glob)) => self.match_paths_to_glob(slug, glob).await?,
            None => Err(format!("reference is missing pack slug: {}", item.borrow()))?,
        };
        Ok(iter)
    }

    async fn expand_refs<'a, S>(
        &self,
        refs: impl Iterator<Item = &'a S>,
    ) -> Result<Vec<(&'a str, String)>>
    where
        S: Borrow<str> + 'a,
    {
        let futs = refs.map(|item| self.expand_ref(item));

        let mut result = Vec::new();
        for fut in futs {
            let vals = fut.await?;
            result.extend(vals);
        }
        Ok(result)
    }
}

/// Returns an iterator over all the possible alias bindings for the given mapping.
pub fn iter_alias_binds<'a, 'b>(
    aliases: &'b HashMap<&'a String, Vec<(&'a str, String)>>,
) -> (
    Vec<&'a String>,
    MultiProduct<slice::Iter<'b, (&'a str, String)>>,
) {
    let (keys, values): (Vec<&'a String>, Vec<_>) = aliases.iter().unzip();
    let result = values.iter().map(|v| *v).multi_cartesian_product();
    (keys, result)
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
