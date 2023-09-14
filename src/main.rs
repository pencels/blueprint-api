use std::sync::Arc;

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

mod blueprint;
mod db;
mod models;
mod routes;
mod util;

pub const STORAGE_ACCOUNT_NAME: &str = "blueprintstore";
pub const STORAGE_ACCOUNT_KEY_NAME: &str = "blueprintstore-key";
pub const KEYVAULT_URI: &str = "https://blueprint-kv.vault.azure.net/";

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

    let cred = StorageCredentials::access_key(STORAGE_ACCOUNT_NAME, key.value);
    let blob_service = BlobServiceClient::new(STORAGE_ACCOUNT_NAME, cred.clone());
    let table_service = TableServiceClient::new(STORAGE_ACCOUNT_NAME, cred);

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(middleware::Compress::default())
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
