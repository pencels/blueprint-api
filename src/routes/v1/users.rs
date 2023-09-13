use actix_web::{get, http::header, post, web, HttpRequest, HttpResponse, Responder};
use azure_data_tables::prelude::TableServiceClient;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{
    db,
    models::users::{NewUser, User},
};

pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(get_users).service(add_user);
}

#[derive(Serialize, Deserialize, Validate)]
pub struct Paginated {
    #[validate(range(min = 1, message = "must be an integer >= 1"))]
    page: Option<usize>,
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[get("users")]
pub async fn get_users(
    tables: web::Data<TableServiceClient>,
    info: web::Query<Paginated>,
) -> Result<impl Responder> {
    let query = info.into_inner();
    query.validate()?;
    let page = query.page;

    let page = tables
        .table_client("users")
        .query()
        .into_stream::<db::users::User>()
        .skip(page.unwrap_or(1) - 1)
        .next()
        .await;

    Ok(match page {
        Some(r) => {
            let r = r?;
            let users: Vec<User> = r.entities.into_iter().map(|u| u.into()).collect();
            HttpResponse::Ok().json(users)
        }
        None => HttpResponse::NotFound()
            .reason("Page does not exist.")
            .finish(),
    })
}

#[post("users")]
pub async fn add_user(
    req: HttpRequest,
    client: web::Data<TableServiceClient>,
    body: web::Json<NewUser>,
) -> Result<impl Responder> {
    let user = db::create_new_user(&client, body.into_inner()).await?;
    let id = serde_json::to_value(&user.id)?;
    Ok(HttpResponse::Created()
        .append_header((
            header::LOCATION,
            req.uri().to_string() + "/" + id.as_str().unwrap(),
        ))
        .json(user))
}
