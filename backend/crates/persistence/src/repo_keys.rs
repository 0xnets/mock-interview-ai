//! Phase 6: catalog of transcript-signing public keys for rotation.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::DbError;

#[derive(Debug, Clone)]
pub struct KeyVersion {
    pub key_id: String,
    pub public_key: Vec<u8>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub retired_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

pub async fn upsert_active(
    pool: &PgPool,
    key_id: &str,
    public_key: &[u8],
    notes: Option<&str>,
) -> Result<(), DbError> {
    sqlx::query(
        r#"
        INSERT INTO key_versions (key_id, public_key, status, notes)
        VALUES ($1, $2, 'active', $3)
        ON CONFLICT (key_id) DO UPDATE
          SET public_key = EXCLUDED.public_key,
              notes      = COALESCE(EXCLUDED.notes, key_versions.notes),
              status     = 'active'
        "#,
    )
    .bind(key_id)
    .bind(public_key)
    .bind(notes)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn retire(pool: &PgPool, key_id: &str) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE key_versions SET status='retired', retired_at = now() WHERE key_id = $1 AND status='active'",
    )
    .bind(key_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_all(pool: &PgPool) -> Result<Vec<KeyVersion>, DbError> {
    let rows: Vec<(String, Vec<u8>, String, DateTime<Utc>, Option<DateTime<Utc>>, Option<String>)> =
        sqlx::query_as(
            r#"
            SELECT key_id, public_key, status, created_at, retired_at, notes
            FROM key_versions
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(key_id, public_key, status, created_at, retired_at, notes)| KeyVersion {
            key_id,
            public_key,
            status,
            created_at,
            retired_at,
            notes,
        })
        .collect())
}
