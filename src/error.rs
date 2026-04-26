use axum::{http::StatusCode, response::{IntoResponse, Response}};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Template(#[from] askama::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("request failed: {:?}", self);
        let msg = match &self {
            AppError::Sqlx(_) => "database error",
            AppError::Template(_) => "template error",
            AppError::Reqwest(_) => "upstream error",
            AppError::Anyhow(_) => "internal error",
        };
        (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
    }
}
