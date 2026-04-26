use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;

use crate::{error::AppError, AppState};

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    has_data: bool,
}

pub async fn index(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM snapshots")
        .fetch_one(&state.pool).await?;
    Ok(DashboardTemplate { has_data: count > 0 })
}
