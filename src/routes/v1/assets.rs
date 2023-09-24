use actix_multipart::form::{tempfile::TempFile, text, MultipartForm};
use actix_web::{
    delete, get,
    http::StatusCode,
    patch, post,
    web::{self, Json},
    HttpRequest, HttpResponse, Responder,
};
use azure_storage_blobs::prelude::BlobServiceClient;
use bson::doc;
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
        .service(get_pack_manifest)
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
    db: web::Data<mongodb::Client>,
    query: web::Query<Paginated>,
) -> Result<impl Responder> {
    let page = query.into_inner().page.unwrap_or(1);
    let packs = db::get_packs(&db, page).await?;
    Ok((Json(packs), StatusCode::OK))
}

#[get("packs/{pack_id}")]
async fn get_pack(
    db: web::Data<mongodb::Client>,
    slug: web::Path<String>,
) -> Result<impl Responder> {
    let slug = slug.into_inner();

    let response = db
        .default_database()
        .unwrap()
        .collection::<db::AssetPack>("packs")
        .find_one(doc! { "_id": &slug }, None)
        .await?;

    match response {
        Some(pack) => Ok(HttpResponse::Ok().json(AssetPack::from(pack))),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}

#[get("packs/{pack_id}/manifest")]
async fn get_pack_manifest(
    db: web::Data<mongodb::Client>,
    slug: web::Path<String>,
) -> Result<impl Responder> {
    let slug = slug.into_inner();

    let result = db
        .default_database()
        .unwrap()
        .collection::<db::AssetPack>("packs")
        .find_one(doc! { "_id": slug }, None)
        .await?;

    match result {
        Some(pack) => Ok(HttpResponse::Ok().json(pack.manifest)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}

#[post("packs/{pack_id}")]
async fn create_pack(
    req: HttpRequest,
    db: web::Data<mongodb::Client>,
    blobs: web::Data<BlobServiceClient>,
    slug: web::Path<String>,
    MultipartForm(form): MultipartForm<UploadPack>,
) -> Result<impl Responder> {
    let session_token = req.cookie("next-auth.session-token").unwrap();

    let session = db
        .database("userdata")
        .collection::<db::Session>("sessions")
        .find_one(doc! { "sessionToken": session_token.value() }, None)
        .await?;

    let Some(session) = session else {
        return Ok(HttpResponse::BadRequest().body("missing session token for this request"));
    };

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
        author: session.user_id,
        manifest: Default::default(),
    };

    // Create pack metadata
    db::create_pack(&db, &blobs, pack).await?;

    // Upload pack data in the bg
    tokio::spawn(async move {
        match db::upload_zipped_pack(&db, &blobs, form.file, &slug).await {
            Ok(_) => {}
            Err(e) => {
                log::error!("error uploading zip file for {}: {}", &slug, e);
                return;
            }
        }
    });

    Ok(HttpResponse::Accepted().finish())
}

#[patch("packs/{pack_id}")]
async fn update_pack(
    db: web::Data<mongodb::Client>,
    pack_id: web::Path<String>,
    patch: web::Json<UpdatePack>,
) -> Result<impl Responder> {
    let pack_id = pack_id.into_inner();
    let patch = patch.into_inner();

    let modifications = doc! {
        "$set": {
            "slug": &patch.slug,
            "name": &patch.name,
            "description": &patch.description,
            "tags": &patch.tags,
            "last_modified": "$currentDate",
            "version": &patch.version,
        }
    };

    db.default_database()
        .unwrap()
        .collection::<AssetPack>("packs")
        .update_one(
            doc! {
               "_id": &pack_id
            },
            modifications,
            None,
        )
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[delete("packs/{pack_id}")]
async fn delete_pack(
    db: web::Data<mongodb::Client>,
    blobs: web::Data<BlobServiceClient>,
    pack_id: web::Path<String>,
) -> Result<impl Responder> {
    let pack_id = pack_id.into_inner();

    db::delete_pack(&db, &blobs, pack_id).await?;

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
