use actix_web::{delete, get, patch, post, web, HttpRequest, HttpResponse, Responder};
use azure_data_tables::prelude::TableServiceClient;
use validator::Validate;

use crate::{
    db,
    models::{NewUser, UpdateUser},
    routes::{util::created, v1::Paginated},
    util::Result,
};

pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(get_users)
        .service(get_user)
        .service(add_user)
        .service(update_user)
        .service(delete_user);
}

#[get("users")]
pub async fn get_users(
    client: web::Data<TableServiceClient>,
    info: web::Query<Paginated>,
) -> Result<impl Responder> {
    let query = info.into_inner();
    query.validate()?;
    let page = query.page;
    let results = db::get_users(&client, page.unwrap_or(1)).await?;
    match results {
        Some(results) => Ok(HttpResponse::Ok().json(results)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}

#[post("users")]
pub async fn add_user(
    req: HttpRequest,
    client: web::Data<TableServiceClient>,
    body: web::Json<NewUser>,
) -> Result<impl Responder> {
    let user = db::create_new_user(&client, body.into_inner()).await?;
    Ok(created(req, &user.id, &user))
}

#[get("users/{id}")]
pub async fn get_user(
    client: web::Data<TableServiceClient>,
    id: web::Path<String>,
) -> Result<impl Responder> {
    let user = db::get_user(&client, &id).await?;
    Ok(HttpResponse::Ok().json(user))
}

#[patch("users/{id}")]
pub async fn update_user(
    client: web::Data<TableServiceClient>,
    id: web::Path<String>,
    update: web::Json<UpdateUser>,
) -> Result<impl Responder> {
    dbg!(&id, &update);
    db::update_user(&client, &id, update.into_inner()).await?;
    Ok(HttpResponse::NoContent())
}

#[delete("users/{id}")]
pub async fn delete_user(
    client: web::Data<TableServiceClient>,
    id: web::Path<String>,
) -> Result<impl Responder> {
    db::delete_user(&client, &id).await?;
    Ok(HttpResponse::NoContent())
}
