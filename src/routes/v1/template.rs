use std::collections::HashMap;

use crate::{
    models::{Layer, Template},
    util::Result,
};
use actix_web::{post, web, HttpResponse, Responder};
use async_channel::Sender;
use serde::{Deserialize, Serialize};

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
    queue: web::Data<Sender<(String, Template)>>,
    template: web::Json<TemplateRequest>,
) -> Result<impl Responder> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let template: Template = template.into_inner().into();

    queue.send((run_id.clone(), template)).await?;

    Ok(HttpResponse::Accepted().json(TemplateRun { run_id }))
}
