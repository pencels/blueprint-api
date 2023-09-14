use actix_multipart::form::{tempfile::TempFile, text, MultipartForm};
use actix_web::{
    get,
    http::StatusCode,
    post,
    web::{self, Json},
    HttpRequest, Responder,
};
use azure_data_tables::prelude::TableServiceClient;
use azure_storage_blobs::prelude::BlobServiceClient;
use serde::{Deserialize, Serialize};

use crate::{
    db::{self, Asset},
    models::AssetPack,
    routes::util::{created, download},
    util::Result,
};

use super::Paginated;

pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(upload_asset)
        .service(download_asset)
        .service(get_packs)
        .service(create_pack);
}

#[derive(Debug, MultipartForm)]
struct UploadAsset {
    #[multipart]
    pack_id: text::Text<String>,
    #[multipart(limit = "512000")]
    file: TempFile,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CreatePack {
    name: String,
    description: String,
    tags: Vec<String>,
}

#[get("packs")]
async fn get_packs(
    tables: web::Data<TableServiceClient>,
    query: web::Query<Paginated>,
) -> Result<impl Responder> {
    let page = query.into_inner().page.unwrap_or(1);
    let packs = db::get_packs(&tables, page).await?;
    Ok((Json(packs), StatusCode::OK))
}

#[post("packs")]
async fn create_pack(
    req: HttpRequest,
    tables: web::Data<TableServiceClient>,
    create_pack: web::Json<CreatePack>,
) -> Result<impl Responder> {
    let create_pack = create_pack.into_inner();
    let pack_id = uuid::Uuid::new_v4().to_string();

    let pack = AssetPack {
        id: pack_id.clone(),
        description: create_pack.description,
        name: create_pack.name,
        tags: create_pack.tags,
    };

    let pack = db::create_pack(&tables, pack).await?;

    Ok(created(req, &pack_id, pack))
}

#[get("assets/{asset_id}/blob")]
async fn download_asset(
    tables: web::Data<TableServiceClient>,
    blobs: web::Data<BlobServiceClient>,
    path: web::Path<(String, String)>,
) -> Result<impl Responder> {
    let (pack_id, asset_id) = path.into_inner();

    let response = tables
        .table_client("assets")
        .partition_key_client(&pack_id)
        .entity_client(&asset_id)?
        .get::<Asset>()
        .await?;
    let asset_metadata = response.entity;

    let content = blobs
        .container_client("assets")
        .blob_client(&asset_id)
        .get_content()
        .await?;

    Ok(download(&asset_metadata.file_name, content))
}

#[post("assets")]
async fn upload_asset(
    req: HttpRequest,
    tables: web::Data<TableServiceClient>,
    blobs: web::Data<BlobServiceClient>,
    MultipartForm(form): MultipartForm<UploadAsset>,
) -> Result<impl Responder> {
    let pack_id = form.pack_id.into_inner();
    let asset_id = uuid::Uuid::new_v4().to_string();

    let asset = db::upload_asset(&tables, &blobs, form.file, &pack_id, &asset_id).await?;

    Ok(created(req, &asset_id, asset))
}
