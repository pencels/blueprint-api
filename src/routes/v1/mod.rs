mod assets;
mod blueprints;
mod compositor;
mod runs;
mod users;
use std::pin::Pin;

use actix_web::FromRequest;
pub use assets::*;
pub use blueprints::*;
pub use compositor::*;
use futures::Future;
pub use runs::*;
pub use users::*;

use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Serialize, Deserialize, Validate)]
pub struct Paginated {
    #[validate(range(min = 1, message = "must be an integer >= 1"))]
    page: Option<usize>,
}

pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(
        actix_web::web::scope("v1")
            .configure(users::config)
            .configure(assets::config)
            .configure(blueprints::config)
            .configure(compositor::config)
            .configure(runs::config),
    );
}

pub struct UserSession(String);

impl FromRequest for UserSession {
    type Error = Box<dyn std::error::Error>;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(
        req: &actix_web::HttpRequest,
        _payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        let result = req
            .cookie("next-auth.session-token")
            .map(|c| UserSession(c.value().to_owned()))
            .ok_or("no session token provided in request".into());
        Box::pin(async move { result })
    }
}
