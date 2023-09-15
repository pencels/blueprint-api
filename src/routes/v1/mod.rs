mod assets;
mod runs;
mod template;
mod users;
pub use assets::*;
pub use runs::*;
pub use template::*;
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
            .configure(template::config)
            .configure(runs::config),
    );
}
