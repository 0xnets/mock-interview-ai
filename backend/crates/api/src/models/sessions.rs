use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct SessionByCodeResponse {
    pub id: Uuid,
    pub state: String,
    pub candidate_name: String,
    pub role_title: String,
    pub expires_at: DateTime<Utc>,
    pub shortcode: String,
}

#[derive(Debug, Serialize)]
pub struct JoinNonceResponse {
    pub id: Uuid,
    pub state: String,
    pub candidate_name: String,
    pub role_title: String,
    pub expires_at: DateTime<Utc>,
    pub join_nonce: String,
    pub ws_path: &'static str,
}
