use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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

/// Candidate-side report that a pre-interview system check failed because the
/// device or browser cannot run the interview.
#[derive(Debug, Deserialize)]
pub struct ReportIncompatRequest {
    pub check: String,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct ReportIncompatResponse {
    pub incompat_count: i32,
    pub limit: i32,
    pub attempts_remaining: i32,
    pub expired: bool,
}
