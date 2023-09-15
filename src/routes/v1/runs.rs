use crate::{db, models::CompositorRun, util::Result};
use actix_web::{get, web, HttpResponse, Responder};
use azure_data_tables::prelude::TableServiceClient;

pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(get_run);
}

#[get("runs/{run_id}")]
pub async fn get_run(
    table: web::Data<TableServiceClient>,
    run_id: web::Path<String>,
) -> Result<impl Responder> {
    let run_id = run_id.into_inner();
    let response = table
        .table_client("runs")
        .partition_key_client(&run_id)
        .entity_client(&run_id)?
        .get::<db::CompositorRun>()
        .await?;

    let run: CompositorRun = response.entity.into();

    Ok(HttpResponse::Ok().json(run))
}
