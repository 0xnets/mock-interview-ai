//! Phase 6: audit_log writer. Every state-changing handler should call one of
//! these. PII discipline: never write raw transcripts/JDs/resumes — only ids
//! and hashes.

use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

use crate::DbError;

pub async fn write(
    pool: &PgPool,
    actor_id: Option<Uuid>,
    session_id: Option<Uuid>,
    action: &str,
    event_id: Option<Uuid>,
    metadata: JsonValue,
) -> Result<(), DbError> {
    sqlx::query(
        r#"
        INSERT INTO audit_log (actor_id, session_id, action, event_id, metadata)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(actor_id)
    .bind(session_id)
    .bind(action)
    .bind(event_id)
    .bind(metadata)
    .execute(pool)
    .await?;
    Ok(())
}
