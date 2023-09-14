use crate::{
    blueprint::{BlendMode, Degrees, Layer, Scale, Template, Transform},
    routes::util::download,
    util::Result,
};
use actix_web::{get, http::StatusCode, post, web, HttpResponse, Responder};
use azure_data_tables::prelude::TableServiceClient;
use azure_storage_blobs::prelude::BlobServiceClient;
use image::codecs::png::PngEncoder;

pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(run_template);
}

#[get("templates/lol")]
async fn run_template(
    tables: web::Data<TableServiceClient>,
    blobs: web::Data<BlobServiceClient>,
) -> Result<impl Responder> {
    let template = Template {
        canvas_size: (256, 256),
        layers: vec![
            Layer {
                blend_mode: BlendMode::Normal,
                image: image::io::Reader::open("assets/baseg.webp")?
                    .decode()?
                    .into_rgba8(),
                opacity: 1.0,
                transform: Transform {
                    offset: (0, 0),
                    scale: Scale(1.3),
                    rotate: Degrees(45.0),
                },
            },
            Layer {
                blend_mode: BlendMode::Normal,
                image: image::io::Reader::open("assets/dh.png")?
                    .decode()?
                    .into_rgba8(),
                opacity: 1.0,
                transform: Transform {
                    offset: (0, 0),
                    scale: Scale(1.0),
                    rotate: Degrees(0.0),
                },
            },
        ],
    };

    let result = template.apply()?;

    let mut buf = Vec::new();
    result.write_with_encoder(PngEncoder::new(&mut buf))?;

    Ok(HttpResponse::Ok().body(buf))
}
