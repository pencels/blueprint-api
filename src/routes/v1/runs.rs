use std::io::{Cursor, Write};

use crate::{db, models::CompositorRun, util::Result};
use actix_web::{get, http::header, web, HttpResponse, Responder};
use azure_data_tables::prelude::TableServiceClient;
use azure_storage_blobs::prelude::BlobServiceClient;
use futures::TryStreamExt;
use zip::write::FileOptions;

pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(get_run).service(get_run_results_zip);
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

#[get("runs/{run_id}/zip")]
pub async fn get_run_results_zip(
    blob: web::Data<BlobServiceClient>,
    run_id: web::Path<String>,
) -> Result<impl Responder> {
    let run_id = run_id.into_inner();
    let container = blob.container_client("template-output");
    let pages: Vec<_> = container
        .list_blobs()
        .prefix(run_id.clone())
        .into_stream()
        .try_collect()
        .await?;

    let blobs = pages.iter().flat_map(|page| page.blobs.blobs());

    let mut buf = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(Cursor::new(&mut buf));
        let options = FileOptions::default();
        for blob in blobs {
            let content = container.blob_client(&blob.name).get_content().await?;
            let file_name = blob.name.split_once('/').unwrap().1;
            writer.start_file(file_name, options)?;
            writer.write(&content)?;
        }
        writer.finish()?;
    }

    Ok(HttpResponse::Ok()
        .append_header(header::ContentDisposition::attachment(format!(
            "{}.zip",
            &run_id
        )))
        .body(buf))
}
