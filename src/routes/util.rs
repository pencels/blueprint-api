use actix_web::{http::header, HttpRequest, HttpResponse};
use serde::Serialize;

pub fn created(req: HttpRequest, id: &str, body: impl Serialize) -> HttpResponse {
    HttpResponse::Created()
        .append_header((header::LOCATION, req.uri().to_string() + "/" + id))
        .json(body)
}

pub fn download(file_name: &str, data: impl Into<bytes::Bytes>) -> HttpResponse {
    HttpResponse::Ok()
        .append_header(header::ContentDisposition::attachment(
            file_name.to_string(),
        ))
        .body(data.into())
}
