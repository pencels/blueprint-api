use std::io::Read;

use crate::util::Result;
use actix_multipart::form::tempfile::TempFile;
use azure_data_tables::{operations::InsertEntityResponse, prelude::TableServiceClient};
use azure_storage_blobs::prelude::BlobServiceClient;
use serde::{Deserialize, Serialize};

use super::{get_entities, DateTime};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Asset {
    #[serde(rename = "PartitionKey")]
    pub partition_key: String,
    #[serde(rename = "RowKey")]
    pub row_key: String,
    pub slug: String,
    pub file_name: String,
    pub content_type: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AssetPack {
    #[serde(rename = "PartitionKey")]
    pub partition_key: String,
    #[serde(rename = "RowKey")]
    pub row_key: String,
    #[serde(rename = "Timestamp", skip_serializing)]
    pub last_modified: DateTime,
    pub name: String,
    pub description: String,
    pub tags: String,
    pub version: String,
}

impl From<crate::models::AssetPack> for AssetPack {
    fn from(pack: crate::models::AssetPack) -> Self {
        Self {
            partition_key: pack.slug.to_string(),
            row_key: pack.slug,
            name: pack.name,
            description: pack.description,
            tags: pack.tags.join(","),
            last_modified: pack.last_modified,
            version: pack.version,
        }
    }
}

impl From<AssetPack> for crate::models::AssetPack {
    fn from(value: AssetPack) -> Self {
        Self {
            name: value.name,
            description: value.description,
            tags: value.tags.split_terminator(",").map(String::from).collect(),
            slug: value.row_key,
            last_modified: value.last_modified,
            version: value.version,
        }
    }
}

pub async fn get_packs(
    client: &TableServiceClient,
    page: usize,
) -> Result<Option<Vec<crate::models::AssetPack>>> {
    get_entities::<AssetPack, crate::models::AssetPack>(client, "packs", page).await
}

pub async fn create_pack(
    tables: &TableServiceClient,
    blobs: &BlobServiceClient,
    pack: crate::models::AssetPack,
) -> crate::util::Result<crate::models::AssetPack> {
    let pack: AssetPack = pack.into();

    let get_result = tables
        .table_client("packs")
        .partition_key_client(&pack.partition_key)
        .entity_client(&pack.row_key)?
        .get::<GenericRowKeyEntity>()
        .await;

    if get_result.is_ok() {
        Err(format!(
            "Cannot create '{}' as it already exists",
            pack.row_key
        ))?
    }

    let cont = blobs.container_client(format!("pack-{}", pack.row_key));
    if !cont.exists().await? {
        cont.create().await?;
    }

    let response: InsertEntityResponse<AssetPack> = tables
        .table_client("packs")
        .insert(&pack)?
        .return_entity(true)
        .await?;

    response
        .entity_with_metadata
        .map(|e| e.entity.into())
        .ok_or("Pack create failed".into())
}

pub async fn upload_zipped_pack(
    blobs: &BlobServiceClient,
    file_metadata: TempFile,
    pack_slug: &str,
) -> crate::util::Result<()> {
    let mut zip = zip::ZipArchive::new(file_metadata.file)?;

    for index in 0..zip.len() {
        let mut buf = Vec::new();
        let name = {
            let mut file = zip.by_index(index)?;
            file.read_to_end(&mut buf)?;
            let name = file
                .enclosed_name()
                .ok_or_else(|| format!("zip file escapes archive: {}", file.name()))?;
            name.as_os_str().to_string_lossy().to_string()
        };

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
    tables: &TableServiceClient,
    blobs: &BlobServiceClient,
    pack_id: String,
) -> Result<()> {
    blobs
        .container_client(format!("pack-{}", &pack_id))
        .delete()
        .await?;
    tables
        .table_client("packs")
        .partition_key_client(&pack_id)
        .entity_client(&pack_id)?
        .delete()
        .await?;

    Ok(())
}
