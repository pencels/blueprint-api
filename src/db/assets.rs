use std::io::Read;

use crate::util::Result;
use actix_multipart::form::tempfile::TempFile;
use azure_core::request_options::Metadata;
use azure_data_tables::{operations::InsertEntityResponse, prelude::TableServiceClient};
use azure_storage_blobs::prelude::BlobServiceClient;
use serde::{Deserialize, Serialize};

use super::get_entities;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Asset {
    #[serde(rename = "PartitionKey")]
    pub partition_key: String,
    #[serde(rename = "RowKey")]
    pub row_key: String,
    pub file_name: String,
    pub content_type: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AssetPack {
    #[serde(rename = "PartitionKey")]
    pub partition_key: String,
    #[serde(rename = "RowKey")]
    pub row_key: String,
    pub name: String,
    pub description: String,
    pub tags: String,
}

impl From<crate::models::AssetPack> for AssetPack {
    fn from(pack: crate::models::AssetPack) -> Self {
        Self {
            partition_key: pack.id.clone(),
            row_key: pack.id,
            name: pack.name,
            description: pack.description,
            tags: pack.tags.join(","),
        }
    }
}

impl From<AssetPack> for crate::models::AssetPack {
    fn from(value: AssetPack) -> Self {
        Self {
            id: value.row_key,
            name: value.name,
            description: value.description,
            tags: value.tags.split_terminator(",").map(String::from).collect(),
        }
    }
}

impl From<Asset> for crate::models::Asset {
    fn from(value: Asset) -> Self {
        Self {
            id: value.row_key,
            pack_id: value.partition_key,
            file_name: value.file_name,
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
    pack: crate::models::AssetPack,
) -> crate::util::Result<crate::models::AssetPack> {
    let pack: AssetPack = pack.into();

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

pub async fn upload_asset(
    tables: &TableServiceClient,
    blobs: &BlobServiceClient,
    mut file_metadata: TempFile,
    pack_id: &str,
    asset_id: &str,
) -> crate::util::Result<crate::models::Asset> {
    let file_name = file_metadata
        .file_name
        .unwrap_or_else(|| pack_id.to_string());
    let content_type_string = file_metadata
        .content_type
        .as_ref()
        .unwrap_or(&mime::APPLICATION_OCTET_STREAM)
        .to_string();

    let mut data = Vec::new();
    file_metadata.file.read_to_end(&mut data)?;

    let mut metadata = Metadata::new();
    metadata.insert("file_name", file_name.clone());

    blobs
        .container_client("assets")
        .blob_client(asset_id)
        .put_block_blob(data)
        .metadata(metadata)
        .content_type(content_type_string)
        .await?;

    let asset = Asset {
        partition_key: pack_id.to_string(),
        row_key: asset_id.to_string(),
        file_name,
        content_type: file_metadata.content_type.map(|t| t.to_string()),
    };

    tables
        .table_client("assets")
        .partition_key_client(pack_id)
        .entity_client(asset_id)?
        .insert_or_replace(&asset)?
        .await?;

    Ok(asset.into())
}
