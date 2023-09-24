use std::io::Read;

use crate::util::Result;
use actix_multipart::form::tempfile::TempFile;
use azure_storage_blobs::prelude::BlobServiceClient;
use mongodb::bson::doc;
use serde::{Deserialize, Serialize};

use super::{get_entities, DateTime};

#[derive(Serialize, Deserialize, Clone)]
pub struct AssetPack {
    #[serde(rename = "_id")]
    pub slug: String,
    pub last_modified: DateTime,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub version: String,
    #[serde(with = "bson::serde_helpers::hex_string_as_object_id")]
    pub author: String,
    pub manifest: Vec<String>,
}

impl From<crate::models::AssetPack> for AssetPack {
    fn from(pack: crate::models::AssetPack) -> Self {
        Self {
            slug: pack.slug,
            name: pack.name,
            description: pack.description,
            tags: pack.tags,
            last_modified: pack.last_modified,
            version: pack.version,
            author: pack.author,
            manifest: pack.manifest,
        }
    }
}

impl From<AssetPack> for crate::models::AssetPack {
    fn from(value: AssetPack) -> Self {
        Self {
            name: value.name,
            description: value.description,
            tags: value.tags,
            slug: value.slug,
            last_modified: value.last_modified,
            version: value.version,
            author: value.author,
            manifest: value.manifest,
        }
    }
}

pub async fn get_packs(
    client: &mongodb::Client,
    page: usize,
) -> Result<Vec<crate::models::AssetPack>> {
    get_entities::<AssetPack, crate::models::AssetPack>(client, "packs", page).await
}

pub async fn create_pack(
    db: &mongodb::Client,
    blobs: &BlobServiceClient,
    pack: crate::models::AssetPack,
) -> crate::util::Result<()> {
    let pack: AssetPack = pack.into();

    let packs_coll = db
        .default_database()
        .unwrap()
        .collection::<AssetPack>("packs");

    let existing_pack = packs_coll
        .find_one(doc! { "_id": &pack.slug }, None)
        .await?;
    if existing_pack.is_some() {
        Err(format!(
            "Cannot create '{}' as it already exists",
            pack.slug
        ))?
    }

    packs_coll.insert_one(&pack, None).await?;

    let cont = blobs.container_client(format!("pack-{}", pack.slug));
    if !cont.exists().await? {
        cont.create().await?;
    }

    Ok(())
}

pub async fn upload_zipped_pack(
    db: &mongodb::Client,
    blobs: &BlobServiceClient,
    file_metadata: TempFile,
    pack_slug: &str,
) -> crate::util::Result<()> {
    let mut zip = zip::ZipArchive::new(file_metadata.file)?;

    for index in 0..zip.len() {
        let mut buf = Vec::new();
        let name = {
            let mut file = zip.by_index(index)?;
            if file.is_dir() {
                continue; // Skip directories
            }
            file.read_to_end(&mut buf)?;
            let name = file
                .enclosed_name()
                .ok_or_else(|| format!("zip file escapes archive: {}", file.name()))?;
            name.as_os_str().to_string_lossy().to_string()
        };

        let update = doc! {
            "$push": {
                "manifest": name.clone(),
            }
        };

        log::info!("updating blah");

        db.default_database()
            .unwrap()
            .collection::<AssetPack>("packs")
            .update_one(doc! { "_id": pack_slug }, update, None)
            .await?;

        blobs
            .container_client(format!("pack-{}", pack_slug))
            .blob_client(name)
            .put_block_blob(buf)
            .await?;
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GenericRowKeyEntity {
    #[serde(rename = "RowKey")]
    row_key: String,
}

pub async fn delete_pack(
    db: &mongodb::Client,
    blobs: &BlobServiceClient,
    pack_id: String,
) -> Result<()> {
    blobs
        .container_client(format!("pack-{}", &pack_id))
        .delete()
        .await?;

    db.default_database()
        .unwrap()
        .collection::<AssetPack>("packs")
        .delete_one(
            doc! {
                "_id": pack_id
            },
            None,
        )
        .await?;

    Ok(())
}
