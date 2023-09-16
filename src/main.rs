use std::sync::Arc;

use actix_cors::Cors;
use actix_web::{
    get,
    middleware::{self, Logger},
    web, App, HttpServer, Responder,
};
use azure_data_tables::prelude::TableServiceClient;
use azure_identity::DefaultAzureCredential;
use azure_security_keyvault::KeyvaultClient;
use azure_storage::StorageCredentials;
use azure_storage_blobs::prelude::BlobServiceClient;

use crate::{blueprint::compositor::Compositor, models::Template};

mod blueprint;
mod db;
mod models;
mod routes;
mod util;

pub const STORAGE_ACCOUNT_NAME: &str = "blueprintstore";
pub const STORAGE_ACCOUNT_KEY_NAME: &str = "blueprintstore-key";
pub const KEYVAULT_URI: &str = "https://blueprint-kv.vault.azure.net/";
const NUM_TEMPLATE_WORKERS: usize = 10;
const NUM_GENERIC_WORKERS: usize = 10;

#[get("/")]
async fn index() -> impl Responder {
    "hi"
}

pub enum Order {
    DeletePack(String),
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let cred = DefaultAzureCredential::default();
    let kv_client = KeyvaultClient::new(KEYVAULT_URI, Arc::new(cred)).unwrap();
    let key = kv_client
        .secret_client()
        .get(STORAGE_ACCOUNT_KEY_NAME)
        .await
        .unwrap();

    let cred = StorageCredentials::access_key(STORAGE_ACCOUNT_NAME, key.value);
    let blob_service = BlobServiceClient::new(STORAGE_ACCOUNT_NAME, cred.clone());
    let table_service = TableServiceClient::new(STORAGE_ACCOUNT_NAME, cred.clone());

    let compositor = Compositor::new(table_service.clone(), blob_service.clone());

    // Template processing
    let (template_queue, recv) = async_channel::unbounded::<(String, Template)>();
    for _ in 0..NUM_TEMPLATE_WORKERS {
        let compositor = compositor.clone();
        let recv = recv.clone();
        tokio::spawn(async move {
            loop {
                let (run_id, template) = recv.recv().await.expect("channel closed unexpectedly");
                match compositor.run_template(&run_id, template).await {
                    Ok(_) => log::info!("template run {} succeeded", &run_id),
                    Err(e) => log::error!("template run {} failed: {}", &run_id, e),
                };
            }
        });
    }

    let (worker_queue, recv) = async_channel::unbounded::<Order>();
    for _ in 0..NUM_GENERIC_WORKERS {
        let table_service = table_service.clone();
        let blob_service = blob_service.clone();
        let recv = recv.clone();
        tokio::spawn(async move {
            loop {
                let order = recv.recv().await.expect("channel closed unexpectedly");
                match order {
                    Order::DeletePack(pack_id) => {
                        db::delete_pack(&table_service, &blob_service, pack_id).await;
                    }
                }
            }
        });
    }

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://localhost:4321")
            .allowed_methods(vec!["GET", "POST"]);
        App::new()
            .wrap(Logger::default())
            .wrap(cors)
            .wrap(middleware::Compress::default())
            .app_data(web::Data::new(template_queue.clone()))
            .app_data(web::Data::new(worker_queue.clone()))
            .app_data(web::Data::new(table_service.clone()))
            .app_data(web::Data::new(blob_service.clone()))
            .configure(routes::v1::config)
            .service(index)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await?;

    println!("bye!");
    Ok(())
}
