//! Phase 5: tamper-evident transcript log.
//!
//! Every `utterance` WS frame writes one row to `transcript_chunks` inside a
//! transaction. Each row's `row_hash = SHA256(prev_hash || canonical(row))`.
//! Checkpoints (separate table) sign a row hash + the last seq with the
//! service Ed25519 key so an offline verifier only needs the public key and
//! the chunk list to confirm tamper-freeness.
//!
//! The canonical-row encoding is intentionally simple text — see
//! [`canonical_row_bytes`]. The CLI verifier replicates this byte-for-byte.

use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::DbError;

pub const HASH_LEN: usize = 32;
pub type Hash = [u8; HASH_LEN];

/// Canonical encoding: pipe-delimited, terminated with `\n`, all integers in
/// base-10. We never store untrusted JSON in the hash input so re-encoding
/// drift can't break verification.
///
/// Format:  `seq|question_id|is_final|client_ts_ms|text\n`
///
/// `text` is appended raw (it's the candidate's own bytes); the trailing `\n`
/// is the unambiguous terminator.
pub fn canonical_row_bytes(
    seq: i32,
    question_id: Uuid,
    is_final: bool,
    client_ts_ms: i64,
    text: &str,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(text.len() + 96);
    buf.extend_from_slice(seq.to_string().as_bytes());
    buf.push(b'|');
    buf.extend_from_slice(question_id.to_string().as_bytes());
    buf.push(b'|');
    buf.push(if is_final { b'1' } else { b'0' });
    buf.push(b'|');
    buf.extend_from_slice(client_ts_ms.to_string().as_bytes());
    buf.push(b'|');
    buf.extend_from_slice(text.as_bytes());
    buf.push(b'\n');
    buf
}

pub fn next_hash(prev: &Hash, canonical: &[u8]) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(prev);
    hasher.update(canonical);
    let digest = hasher.finalize();
    let mut out = [0u8; HASH_LEN];
    out.copy_from_slice(&digest);
    out
}

#[derive(Debug, Clone)]
pub struct AppendedChunk {
    pub id: i64,
    pub seq: i32,
    pub row_hash: Hash,
}

/// Append one chunk for `session_id`. Computes `seq = last_seq + 1` and
/// `row_hash = sha256(prev_hash || canonical_row)` inside the same TX so two
/// concurrent appenders can't fork the chain. Idempotent on retry only if the
/// caller picks a deterministic identity (we don't — the WS actor is the sole
/// writer per session).
pub async fn append_chunk(
    pool: &PgPool,
    session_id: Uuid,
    question_id: Uuid,
    is_final: bool,
    client_ts_ms: i64,
    text: &str,
) -> Result<AppendedChunk, DbError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;

    let prev: Option<(i32, Vec<u8>)> = sqlx::query_as(
        r#"
        SELECT seq, row_hash
        FROM transcript_chunks
        WHERE session_id = $1
        ORDER BY seq DESC
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(session_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (next_seq, prev_hash) = match prev {
        Some((s, h)) => {
            let mut arr = [0u8; HASH_LEN];
            if h.len() != HASH_LEN {
                return Err(DbError::Sqlx(sqlx::Error::Decode(
                    "transcript prev_hash has unexpected length".into(),
                )));
            }
            arr.copy_from_slice(&h);
            (s + 1, arr)
        }
        None => (1, [0u8; HASH_LEN]),
    };

    let canonical = canonical_row_bytes(next_seq, question_id, is_final, client_ts_ms, text);
    let row_hash = next_hash(&prev_hash, &canonical);

    let inserted: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO transcript_chunks (
            session_id, question_id, seq, text, is_final,
            client_ts_ms, prev_hash, row_hash
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#,
    )
    .bind(session_id)
    .bind(question_id)
    .bind(next_seq)
    .bind(text)
    .bind(is_final)
    .bind(client_ts_ms)
    .bind(prev_hash.as_slice())
    .bind(row_hash.as_slice())
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(AppendedChunk {
        id: inserted.0,
        seq: next_seq,
        row_hash,
    })
}

#[derive(Debug, Clone)]
pub struct ChunkRow {
    pub id: i64,
    pub session_id: Uuid,
    pub question_id: Uuid,
    pub seq: i32,
    pub text: String,
    pub is_final: bool,
    pub client_ts_ms: i64,
    pub server_ts: chrono::DateTime<chrono::Utc>,
    pub prev_hash: Vec<u8>,
    pub row_hash: Vec<u8>,
}

pub async fn list_chunks(pool: &PgPool, session_id: Uuid) -> Result<Vec<ChunkRow>, DbError> {
    let rows: Vec<(
        i64,
        Uuid,
        Uuid,
        i32,
        String,
        bool,
        i64,
        chrono::DateTime<chrono::Utc>,
        Vec<u8>,
        Vec<u8>,
    )> = sqlx::query_as(
        r#"
        SELECT id, session_id, question_id, seq, text, is_final,
               client_ts_ms, server_ts, prev_hash, row_hash
        FROM transcript_chunks
        WHERE session_id = $1
        ORDER BY seq
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                session_id,
                question_id,
                seq,
                text,
                is_final,
                client_ts_ms,
                server_ts,
                prev_hash,
                row_hash,
            )| ChunkRow {
                id,
                session_id,
                question_id,
                seq,
                text,
                is_final,
                client_ts_ms,
                server_ts,
                prev_hash,
                row_hash,
            },
        )
        .collect())
}

#[derive(Debug, Clone)]
pub struct CheckpointRow {
    pub id: i64,
    pub session_id: Uuid,
    pub last_seq: i32,
    pub last_row_id: i64,
    pub row_hash: Vec<u8>,
    pub signature: Vec<u8>,
    pub key_id: String,
    pub chunk_count: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn insert_checkpoint(
    pool: &PgPool,
    session_id: Uuid,
    last_seq: i32,
    last_row_id: i64,
    row_hash: &[u8],
    signature: &[u8],
    key_id: &str,
    chunk_count: i32,
) -> Result<(), DbError> {
    sqlx::query(
        r#"
        INSERT INTO transcript_checkpoints (
            session_id, last_seq, last_row_id, row_hash, signature, key_id, chunk_count
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (session_id, last_seq) DO NOTHING
        "#,
    )
    .bind(session_id)
    .bind(last_seq)
    .bind(last_row_id)
    .bind(row_hash)
    .bind(signature)
    .bind(key_id)
    .bind(chunk_count)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_checkpoints(
    pool: &PgPool,
    session_id: Uuid,
) -> Result<Vec<CheckpointRow>, DbError> {
    let rows: Vec<(
        i64,
        Uuid,
        i32,
        i64,
        Vec<u8>,
        Vec<u8>,
        String,
        i32,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        r#"
        SELECT id, session_id, last_seq, last_row_id, row_hash, signature,
               key_id, chunk_count, created_at
        FROM transcript_checkpoints
        WHERE session_id = $1
        ORDER BY last_seq
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                session_id,
                last_seq,
                last_row_id,
                row_hash,
                signature,
                key_id,
                chunk_count,
                created_at,
            )| CheckpointRow {
                id,
                session_id,
                last_seq,
                last_row_id,
                row_hash,
                signature,
                key_id,
                chunk_count,
                created_at,
            },
        )
        .collect())
}

/// Pull the last-chunk pointer the actor needs to decide whether to take a
/// fresh checkpoint at the end of an answer.
pub async fn latest_chunk_pointer(
    pool: &PgPool,
    session_id: Uuid,
) -> Result<Option<(i64, i32, Vec<u8>)>, DbError> {
    let row: Option<(i64, i32, Vec<u8>)> = sqlx::query_as(
        r#"
        SELECT id, seq, row_hash
        FROM transcript_chunks
        WHERE session_id = $1
        ORDER BY seq DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}
