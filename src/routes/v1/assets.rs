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
    db,
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
    blobs: web::Data<BlobServiceClient>,
    asset_id: web::Path<String>,
) -> Result<impl Responder> {
    let asset_id = asset_id.into_inner();

    let blob_client = blobs.container_client("assets").blob_client(&asset_id);
    let tags_response = blob_client.get_tags().await?;
    let content = blob_client.get_content().await?;

    let tags = tags_response.tags;
    let file_name = tags
        .into_iter()
        .filter(|(k, _)| k == "file_name")
        .nth(0)
        .map(|(_, v)| v)
        .unwrap_or(asset_id);

    Ok(download(&file_name, content))
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
