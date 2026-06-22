//! Phase 5: at-least-once event outbox.
//!
//! Producers `INSERT` into `event_outbox` in the same transaction as the
//! state change they describe (a report row, an uploaded PDF, …). A polling
//! dispatcher claims rows under a short lease, runs the handler, and either
//! marks the row dispatched or bumps the attempt counter + locked_until.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Postgres, Transaction};

use crate::DbError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxRow {
    pub id: i64,
    pub topic: String,
    pub payload: JsonValue,
    pub attempts: i32,
    pub created_at: DateTime<Utc>,
}

pub async fn enqueue_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    topic: &str,
    payload: &JsonValue,
) -> Result<i64, DbError> {
    let row: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO event_outbox (topic, payload)
        VALUES ($1, $2)
        RETURNING id
        "#,
    )
    .bind(topic)
    .bind(payload)
    .fetch_one(&mut **tx)
    .await?;
    Ok(row.0)
}

pub async fn enqueue(pool: &PgPool, topic: &str, payload: &JsonValue) -> Result<i64, DbError> {
    let mut tx = pool.begin().await?;
    let id = enqueue_in_tx(&mut tx, topic, payload).await?;
    tx.commit().await?;
    Ok(id)
}

/// Claim up to `limit` rows by extending their `locked_until` to now + lease.
/// Rows whose `locked_until` is in the future are skipped so a parallel
/// dispatcher doesn't pull them. Uses `SKIP LOCKED` so we don't queue behind
/// a slow consumer.
pub async fn claim_batch(
    pool: &PgPool,
    limit: i64,
    lease: ChronoDuration,
) -> Result<Vec<OutboxRow>, DbError> {
    let until = Utc::now() + lease;
    let rows: Vec<(i64, String, JsonValue, i32, DateTime<Utc>)> = sqlx::query_as(
        r#"
        WITH claimed AS (
            SELECT id
            FROM event_outbox
            WHERE dispatched_at IS NULL
              AND (locked_until IS NULL OR locked_until < now())
            ORDER BY id
            FOR UPDATE SKIP LOCKED
            LIMIT $1
        )
        UPDATE event_outbox o
        SET locked_until = $2,
            attempts = o.attempts + 1
        FROM claimed
        WHERE o.id = claimed.id
        RETURNING o.id, o.topic, o.payload, o.attempts, o.created_at
        "#,
    )
    .bind(limit)
    .bind(until)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, topic, payload, attempts, created_at)| OutboxRow {
            id,
            topic,
            payload,
            attempts,
            created_at,
        })
        .collect())
}

pub async fn mark_dispatched(pool: &PgPool, id: i64) -> Result<(), DbError> {
    sqlx::query(
        r#"
        UPDATE event_outbox
        SET dispatched_at = now(),
            locked_until = NULL,
            last_error = NULL
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_failed(
    pool: &PgPool,
    id: i64,
    err: &str,
    backoff: ChronoDuration,
) -> Result<(), DbError> {
    let until = Utc::now() + backoff;
    sqlx::query(
        r#"
        UPDATE event_outbox
        SET locked_until = $2,
            last_error = $3
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(until)
    .bind(err)
    .execute(pool)
    .await?;
    Ok(())
}
