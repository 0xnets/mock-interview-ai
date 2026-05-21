use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// Candidate/session input for a new interview. The HR config (recipient
/// email, question counts, behavioral bank, pass threshold) is loaded
/// server-side from the current account, not sent by the client.
#[derive(Debug, Deserialize)]
pub struct CreateInterviewRequest {
    pub candidate_name: String,
    pub role_title: String,
    pub jd_text: String,
    pub resume_text: String,
    #[serde(default = "default_include_intro")]
    pub include_intro: bool,
}

fn default_include_intro() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct CreateInterviewResponse {
    pub id: Uuid,
    pub shortcode: String,
    pub share_url: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct FinalizeResponse {
    pub report: ReportPayload,
}

#[derive(Debug, Serialize)]
pub struct ReportPayload {
    pub id: Uuid,
    pub session_id: Uuid,
    pub overall_percentage: i16,
    pub technical_score: i16,
    pub behavioral_score: i16,
    pub passed: bool,
    pub summary: String,
    pub strengths: JsonValue,
    pub weaknesses: JsonValue,
    pub action_items: JsonValue,
    pub per_question: JsonValue,
    pub model: String,
    pub prompt_version: String,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PerQuestionEntry {
    pub q: i16,
    pub section: &'static str,
    pub question: String,
    pub score: Option<i16>,
    pub feedback: String,
    /// Time the candidate took between question display and submit.
    /// `None` when no answer row exists (e.g. unanswered tail).
    pub duration_ms: Option<i32>,
    pub duration_seconds: Option<i64>,
    pub time_bucket: Option<&'static str>,
}
