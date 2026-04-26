//! Thin DB helpers. Most query code lives next to the model/route that owns it.
//! Kept as a module for future shared query utilities (batched upserts, etc.).

#![allow(dead_code)]

use sqlx::SqlitePool;

pub async fn count_snapshots(pool: &SqlitePool) -> sqlx::Result<i64> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM snapshots")
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}
