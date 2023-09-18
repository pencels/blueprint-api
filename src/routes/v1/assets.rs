use actix_multipart::form::{tempfile::TempFile, text, MultipartForm};
use actix_web::{
    delete, get,
    http::StatusCode,
    patch, post,
    web::{self, Json},
    HttpResponse, Responder,
};
use azure_data_tables::prelude::TableServiceClient;
use azure_storage_blobs::prelude::BlobServiceClient;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{
    db::{self, DateTime},
    models::AssetPack,
    routes::util::download,
    util::Result,
};

use super::Paginated;

pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(download_asset)
        .service(get_packs)
        .service(get_pack)
        .service(create_pack)
        .service(update_pack)
        .service(delete_pack);
}

#[derive(Debug, MultipartForm)]
struct UploadPack {
    name: text::Text<String>,
    description: text::Text<String>,
    tags: text::Text<String>,
    version: text::Text<String>,
    #[multipart]
    file: TempFile,
}

#[derive(Debug, Serialize, Deserialize)]
struct UpdatePack {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub last_modified: DateTime,
    pub version: String,
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

#[get("packs/{pack_id}")]
async fn get_pack(
    tables: web::Data<TableServiceClient>,
    slug: web::Path<String>,
) -> Result<impl Responder> {
    let slug = slug.into_inner();

    let response = tables
        .table_client("packs")
        .partition_key_client(&slug)
        .entity_client(&slug)?
        .get::<db::AssetPack>()
        .await;

    match response {
        Ok(response) => {
            let asset: AssetPack = response.entity.into();
            Ok(HttpResponse::Ok().json(asset))
        }
        Err(_) => Ok(HttpResponse::NotFound().finish()),
    }
}

#[post("packs/{pack_id}")]
async fn create_pack(
    tables: web::Data<TableServiceClient>,
    blobs: web::Data<BlobServiceClient>,
    slug: web::Path<String>,
    MultipartForm(form): MultipartForm<UploadPack>,
) -> Result<impl Responder> {
    let pack = AssetPack {
        slug: slug.clone(),
        description: form.description.into_inner(),
        name: form.name.into_inner(),
        tags: form
            .tags
            .into_inner()
            .split_terminator(',')
            .into_iter()
            .map(|s| s.to_owned())
            .collect(),
        last_modified: OffsetDateTime::now_utc().into(),
        version: form.version.into_inner(),
    };

    // Create pack metadata
    db::create_pack(&tables, &blobs, pack).await?;

    // Upload pack data in the bg
    tokio::spawn(async move {
        match db::upload_zipped_pack(&blobs, form.file, &slug).await {
            Ok(_) => {}
            Err(e) => {
                log::error!("error uploading zip file for {}: {}", &slug, e);
                return;
            }
        }
    });

    Ok(HttpResponse::Accepted())
}

#[patch("packs/{pack_id}")]
async fn update_pack(
    tables: web::Data<TableServiceClient>,
    pack_id: web::Path<String>,
    patch: web::Json<UpdatePack>,
) -> Result<impl Responder> {
    let pack_id = pack_id.into_inner();
    let patch = patch.into_inner();

    tables
        .table_client("packs")
        .partition_key_client(&pack_id)
        .entity_client(&pack_id)?
        .merge(patch, azure_data_tables::IfMatchCondition::Any)?
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[delete("packs/{pack_id}")]
async fn delete_pack(
    tables: web::Data<TableServiceClient>,
    blobs: web::Data<BlobServiceClient>,
    pack_id: web::Path<String>,
) -> Result<impl Responder> {
    let pack_id = pack_id.into_inner();

    db::delete_pack(&tables, &blobs, pack_id).await?;

    Ok(HttpResponse::Accepted().finish())
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
