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
    #[error("{0}")]
    BadRequest(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("request failed: {:?}", self);
        match &self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()).into_response(),
            AppError::Sqlx(_)     => (StatusCode::INTERNAL_SERVER_ERROR, "database error").into_response(),
            AppError::Template(_) => (StatusCode::INTERNAL_SERVER_ERROR, "template error").into_response(),
            AppError::Reqwest(_)  => (StatusCode::INTERNAL_SERVER_ERROR, "upstream error").into_response(),
            AppError::Anyhow(_)   => (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response(),
        }
    }
}
