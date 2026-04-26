use askama::Template;
use askama_axum::IntoResponse;

use crate::error::AppError;

#[derive(Template)]
#[template(path = "guide.html")]
struct GuideTemplate;

pub async fn index() -> Result<impl IntoResponse, AppError> {
    Ok(GuideTemplate)
}
