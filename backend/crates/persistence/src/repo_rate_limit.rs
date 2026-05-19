//! Phase 6: per-account/route rate-limit overrides backing the limiter.

use sqlx::PgPool;
use uuid::Uuid;

use crate::DbError;

#[derive(Debug, Clone)]
pub struct OverrideRow {
    pub account_id: Option<Uuid>,
    pub route: Option<String>,
    pub limit_count: i32,
    pub window_secs: i32,
}

pub async fn list_for_account(
    pool: &PgPool,
    account_id: Option<Uuid>,
) -> Result<Vec<OverrideRow>, DbError> {
    let rows: Vec<(Option<Uuid>, Option<String>, i32, i32)> = sqlx::query_as(
        r#"
        SELECT account_id, route, limit_count, window_secs
        FROM rate_limit_overrides
        WHERE account_id = $1 OR account_id IS NULL
        ORDER BY account_id NULLS LAST, route NULLS LAST
        "#,
    )
    .bind(account_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(account_id, route, limit_count, window_secs)| OverrideRow {
            account_id,
            route,
            limit_count,
            window_secs,
        })
        .collect())
}
