use std::collections::HashMap;

use crate::{
    db,
    models::{CompositorRunStatus, Layer, Template},
    util::Result,
};
use actix_web::{post, web, HttpResponse, Responder};
use async_channel::Sender;
use bson::doc;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::UserSession;

pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(run_template);
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TemplateRun {
    run_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TemplateRequest {
    aliases: HashMap<String, Vec<String>>,
    canvas_size: (u32, u32),
    layers: Vec<Layer>,
}

impl From<TemplateRequest> for Template {
    fn from(value: TemplateRequest) -> Self {
        Self {
            aliases: value.aliases,
            canvas_size: value.canvas_size,
            layers: value.layers,
        }
    }
}

#[post("compositor")]
async fn run_template(
    UserSession(session_token): UserSession,
    db: web::Data<mongodb::Client>,
    queue: web::Data<Sender<(String, Template)>>,
    template: web::Json<TemplateRequest>,
) -> Result<impl Responder> {
    let template: Template = template.into_inner().into();

    let session = db
        .database("userdata")
        .collection::<db::Session>("sessions")
        .find_one(doc! { "sessionToken": session_token }, None)
        .await?;

    let Some(session) = session else {
        return Ok(HttpResponse::BadRequest().body("no session found for given session token"));
    };

    let run = db::CompositorRun {
        created: OffsetDateTime::now_utc().into(),
        id: Default::default(),
        status: CompositorRunStatus::Pending as u32,
        author: session.user_id,
    };

    let result = db
        .default_database()
        .unwrap()
        .collection::<db::CompositorRun>("runs")
        .insert_one(&run, None)
        .await?;

    let run_id = result.inserted_id.as_object_id().unwrap().to_hex();

    queue.send((run_id.clone(), template)).await?;

    Ok(HttpResponse::Accepted().json(TemplateRun { run_id }))
}
