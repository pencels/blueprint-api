use actix_web::{delete, get, patch, post, web, HttpRequest, HttpResponse, Responder};
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
    client: web::Data<mongodb::Client>,
    info: web::Query<Paginated>,
) -> Result<impl Responder> {
    let query = info.into_inner();
    query.validate()?;
    let page = query.page;
    let results = db::get_users(&client, page.unwrap_or(1)).await?;
    Ok(HttpResponse::Ok().json(results))
}

#[post("users")]
pub async fn add_user(
    req: HttpRequest,
    client: web::Data<mongodb::Client>,
    body: web::Json<NewUser>,
) -> Result<impl Responder> {
    let body = body.into_inner();
    let username = body.username.clone();
    let user = db::create_new_user(&client, body).await?;
    Ok(created(req, &username, &user))
}

#[get("users/{id}")]
pub async fn get_user(
    client: web::Data<mongodb::Client>,
    id: web::Path<String>,
) -> Result<impl Responder> {
    let user = db::get_user(&client, &id).await?;
    Ok(HttpResponse::Ok().json(user))
}

#[patch("users/{id}")]
pub async fn update_user(
    client: web::Data<mongodb::Client>,
    id: web::Path<String>,
    update: web::Json<UpdateUser>,
) -> Result<impl Responder> {
    dbg!(&id, &update);
    db::update_user(&client, &id, update.into_inner()).await?;
    Ok(HttpResponse::NoContent())
}

#[delete("users/{id}")]
pub async fn delete_user(
    client: web::Data<mongodb::Client>,
    id: web::Path<String>,
) -> Result<impl Responder> {
    db::delete_user(&client, &id).await?;
    Ok(HttpResponse::NoContent())
}
