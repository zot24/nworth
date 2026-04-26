//! API documentation routes — serves Swagger UI and OpenAPI spec.

use askama::Template;
use askama_axum::IntoResponse;
use axum::response::Response;
use axum::http::{header, StatusCode};

#[derive(Template)]
#[template(path = "swagger.html")]
struct SwaggerTemplate;

/// GET /api/docs — Swagger UI
pub async fn swagger_ui() -> impl IntoResponse {
    SwaggerTemplate
}

/// GET /api/docs/openapi.yaml — raw OpenAPI spec
pub async fn openapi_spec() -> Response {
    let yaml = include_str!("../../openapi.yaml");
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/yaml")
        .body(yaml.into())
        .unwrap()
}
