use actix_web::{delete, get, patch, post, web, HttpRequest, HttpResponse, Responder};
use bson::doc;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};

use crate::{
    db,
    routes::{util::created, v1::UserSession},
    util::Result,
};

pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(create_blueprint)
        .service(list_blueprints)
        .service(get_blueprint)
        .service(update_blueprint)
        .service(delete_blueprint);
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CreateBlueprint {
    name: String,
    templates: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct BlueprintResponse {
    id: String,
    name: String,
    author: String,
    templates: String,
}

impl From<db::Blueprint> for BlueprintResponse {
    fn from(value: db::Blueprint) -> Self {
        Self {
            id: value.id,
            name: value.name,
            author: value.author,
            templates: value.templates,
        }
    }
}

#[post("blueprints")]
pub async fn create_blueprint(
    req: HttpRequest,
    UserSession(session_token): UserSession,
    db: web::Data<mongodb::Client>,
    body: web::Json<CreateBlueprint>,
) -> Result<impl Responder> {
    let body = body.into_inner();

    let Some(session) = db
        .database("userdata")
        .collection::<db::Session>("sessions")
        .find_one(doc! { "sessionToken": session_token }, None)
        .await?
    else {
        return Ok(HttpResponse::BadRequest().body("no session found for given session token"));
    };

    let mut blueprint = db::Blueprint {
        id: Default::default(),
        author: session.user_id,
        name: body.name,
        templates: body.templates,
    };

    let inserted = db
        .default_database()
        .unwrap()
        .collection::<db::Blueprint>("blueprints")
        .insert_one(&blueprint, None)
        .await?;
    let id = inserted.inserted_id.as_object_id().unwrap().to_hex();

    blueprint.id = id.clone();
    let blueprint: BlueprintResponse = blueprint.into();
    Ok(created(req, &id, blueprint))
}

#[get("blueprints")]
pub async fn list_blueprints(db: web::Data<mongodb::Client>) -> Result<impl Responder> {
    let cursor = db
        .default_database()
        .unwrap()
        .collection::<db::Blueprint>("blueprints")
        .find(doc! {}, None)
        .await?;
    let blueprints: Vec<BlueprintResponse> = cursor.map_ok(|b| b.into()).try_collect().await?;
    Ok(HttpResponse::Ok().json(blueprints))
}

#[get("blueprints/{id}")]
pub async fn get_blueprint(
    db: web::Data<mongodb::Client>,
    id: web::Path<String>,
) -> Result<impl Responder> {
    let id = id.into_inner();

    let Some(blueprint) = db
        .default_database()
        .unwrap()
        .collection::<db::Blueprint>("blueprints")
        .find_one(doc! { "_id": id }, None)
        .await?
    else {
        return Ok(HttpResponse::NotFound().finish());
    };

    let blueprint: BlueprintResponse = blueprint.into();
    Ok(HttpResponse::Ok().json(blueprint))
}

#[patch("blueprints/{id}")]
pub async fn update_blueprint(
    db: web::Data<mongodb::Client>,
    id: web::Path<String>,
    body: web::Json<CreateBlueprint>,
) -> Result<impl Responder> {
    let id = id.into_inner();
    let body = body.into_inner();

    let update = doc! {
        "name": body.name,
        "templates": body.templates,
    };

    db.default_database()
        .unwrap()
        .collection::<db::Blueprint>("blueprints")
        .update_one(doc! { "_id": id }, update, None)
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[delete("blueprints/{id}")]
pub async fn delete_blueprint(
    db: web::Data<mongodb::Client>,
    id: web::Path<String>,
) -> Result<impl Responder> {
    let id = id.into_inner();
    db.default_database()
        .unwrap()
        .collection::<db::Blueprint>("blueprints")
        .delete_one(doc! { "_id": id }, None)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}
