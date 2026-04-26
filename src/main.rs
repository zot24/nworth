use anyhow::Result;
use portfolio_tracker::{build_app, config::Config, migrate, AppState};
use sqlx::sqlite::SqlitePoolOptions;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = Config::from_env()?;
    tracing::info!("starting nworth-web on {}", cfg.bind_addr);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&cfg.database_url)
        .await?;
    migrate(&pool).await?;

    let addr: SocketAddr = cfg.bind_addr.parse()?;
    let state = AppState { pool, cfg };
    let app = build_app(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
