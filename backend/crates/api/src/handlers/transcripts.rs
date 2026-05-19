//! Phase 5: tamper-evident transcript export.
//!
//! Returns `{chunks[], checkpoints[], public_key, canonical_row_format}` so an
//! offline verifier can re-hash the chain and check each checkpoint's
//! signature without talking to the API again.

use axum::{
    extract::{Path, State},
    Json,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use persistence::repo_transcript;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    app::AppState,
    error::ApiResult,
};

#[derive(Debug, Serialize)]
pub struct TranscriptResponse {
    pub session_id: Uuid,
    pub public_key_b64: String,
    pub canonical_row_format: &'static str,
    pub checkpoint_signing_format: &'static str,
    pub chunks: Vec<ChunkPayload>,
    pub checkpoints: Vec<CheckpointPayload>,
}

#[derive(Debug, Serialize)]
pub struct ChunkPayload {
    pub seq: i32,
    pub question_id: Uuid,
    pub is_final: bool,
    pub client_ts_ms: i64,
    pub server_ts: chrono::DateTime<chrono::Utc>,
    pub text: String,
    pub prev_hash_hex: String,
    pub row_hash_hex: String,
}

#[derive(Debug, Serialize)]
pub struct CheckpointPayload {
    pub last_seq: i32,
    pub row_hash_hex: String,
    pub signature_b64: String,
    pub key_id: String,
    pub chunk_count: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn get_transcript(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<TranscriptResponse>> {
    let chunks = repo_transcript::list_chunks(&state.pools.read, id).await?;
    let checkpoints = repo_transcript::list_checkpoints(&state.pools.read, id).await?;

    let chunks = chunks
        .into_iter()
        .map(|c| ChunkPayload {
            seq: c.seq,
            question_id: c.question_id,
            is_final: c.is_final,
            client_ts_ms: c.client_ts_ms,
            server_ts: c.server_ts,
            text: c.text,
            prev_hash_hex: hex::encode(c.prev_hash),
            row_hash_hex: hex::encode(c.row_hash),
        })
        .collect();

    let checkpoints = checkpoints
        .into_iter()
        .map(|c| CheckpointPayload {
            last_seq: c.last_seq,
            row_hash_hex: hex::encode(c.row_hash),
            signature_b64: B64.encode(c.signature),
            key_id: c.key_id,
            chunk_count: c.chunk_count,
            created_at: c.created_at,
        })
        .collect();

    Ok(Json(TranscriptResponse {
        session_id: id,
        public_key_b64: B64.encode(state.signer.public_key_bytes()),
        canonical_row_format: "seq|question_id|is_final(0/1)|client_ts_ms|text\\n",
        checkpoint_signing_format: "session_id|last_seq|key_id|row_hash",
        chunks,
        checkpoints,
    }))
}
