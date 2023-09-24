use std::sync::Arc;

use actix_cors::Cors;
use actix_web::{
    get,
    middleware::{self, Logger},
    web, App, HttpServer, Responder,
};
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

const STORAGE_ACCOUNT_NAME: &str = "blueprintstore";
const STORAGE_ACCOUNT_KEY_NAME: &str = "blueprintstore-key";
const DB_CONN_STRING_NAME: &str = "blueprintdb-connstring";
const KEYVAULT_URI: &str = "https://blueprint-kv.vault.azure.net/";
const NUM_TEMPLATE_WORKERS: usize = 10;

#[get("/")]
async fn index() -> impl Responder {
    "hi"
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
    let db_conn_string = kv_client
        .secret_client()
        .get(DB_CONN_STRING_NAME)
        .await
        .unwrap()
        .value;

    let cred = StorageCredentials::access_key(STORAGE_ACCOUNT_NAME, key.value);
    let blob_service = BlobServiceClient::new(STORAGE_ACCOUNT_NAME, cred.clone());
    let mut db_client_options = mongodb::options::ClientOptions::parse(db_conn_string)
        .await
        .unwrap();
    db_client_options.default_database = Some(String::from("db"));
    let db_client = mongodb::Client::with_options(db_client_options).unwrap();

    let compositor = Compositor::new(db_client.clone(), blob_service.clone());

    // Template processing
    let (template_queue, recv) = async_channel::unbounded::<Template>();
    for _ in 0..NUM_TEMPLATE_WORKERS {
        let compositor = compositor.clone();
        let recv = recv.clone();
        tokio::spawn(async move {
            loop {
                let template = recv.recv().await.expect("channel closed unexpectedly");
                match compositor.run_template(template).await.unwrap() {
                    (run_id, Ok(_)) => log::info!("template run {} succeeded", &run_id),
                    (run_id, Err(e)) => log::error!("template run {} failed: {}", &run_id, e),
                };
            }
        });
    }

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://localhost:4321")
            .supports_credentials()
            .allowed_methods(vec!["GET", "POST", "DELETE"]);
        App::new()
            .wrap(Logger::default())
            .wrap(cors)
            .wrap(middleware::Compress::default())
            .app_data(web::Data::new(template_queue.clone()))
            .app_data(web::Data::new(blob_service.clone()))
            .app_data(web::Data::new(db_client.clone()))
            .app_data(
                actix_multipart::form::MultipartFormConfig::default()
                    .total_limit(1024 * 1024 * 200),
            )
            .configure(routes::v1::config)
            .service(index)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await?;

    println!("bye!");
    Ok(())
}
