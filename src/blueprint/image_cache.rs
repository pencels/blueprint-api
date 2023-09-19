use std::{io::Cursor, sync::Arc};

use azure_storage_blobs::prelude::BlobServiceClient;
use cache_loader_async::{
    backing::LruCacheBacking,
    cache_api::{CacheEntry, LoadingCache},
};
use image::RgbaImage;

const POOL_SIZE: usize = 20;

pub struct ImageCache {
    pub inner: LoadingCache<
        (String, String),
        RgbaImage,
        CacheError,
        LruCacheBacking<(String, String), CacheEntry<RgbaImage, CacheError>>,
    >,
}

#[derive(Debug, Clone)]
pub struct CacheError {
    message: String,
}

impl<E: std::error::Error> From<E> for CacheError {
    fn from(value: E) -> Self {
        CacheError {
            message: value.to_string(),
        }
    }
}

impl ImageCache {
    pub fn new(blobs: Arc<BlobServiceClient>) -> ImageCache {
        let inner = LoadingCache::with_backing(
            LruCacheBacking::new(POOL_SIZE),
            move |(pack, path): (String, String)| {
                let blobs = blobs.clone();
                async move {
                    let content = blobs
                        .container_client(format!("pack-{}", pack))
                        .blob_client(path)
                        .get_content()
                        .await?;

                    let image = image::io::Reader::new(Cursor::new(content))
                        .with_guessed_format()?
                        .decode()?
                        .into_rgba8();

                    Ok(image)
                }
            },
        );
        ImageCache { inner }
    }
}
