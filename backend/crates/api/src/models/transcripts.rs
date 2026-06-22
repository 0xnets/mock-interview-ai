use serde::Serialize;
use uuid::Uuid;

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
