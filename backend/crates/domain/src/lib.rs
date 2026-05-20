use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    Pending,
    Primed,
    Active,
    Paused,
    Completed,
    Scored,
    Expired,
    Aborted,
}

impl SessionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Primed => "primed",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Scored => "scored",
            Self::Expired => "expired",
            Self::Aborted => "aborted",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "pending" => Self::Pending,
            "primed" => Self::Primed,
            "active" => Self::Active,
            "paused" => Self::Paused,
            "completed" => Self::Completed,
            "scored" => Self::Scored,
            "expired" => Self::Expired,
            "aborted" => Self::Aborted,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuestionKind {
    Intro,
    Technical,
    Behavioral,
    Followup,
}

impl QuestionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Intro => "intro",
            Self::Technical => "technical",
            Self::Behavioral => "behavioral",
            Self::Followup => "followup",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterviewSession {
    pub id: Uuid,
    pub account_id: Uuid,
    pub candidate_name: String,
    pub role_title: String,
    pub state: SessionState,
    pub shortcode: String,
    pub expires_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub pass_threshold: i16,
    pub hr_email: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: Uuid,
    pub session_id: Uuid,
    pub ordinal: i16,
    pub kind: QuestionKind,
    pub parent_question_id: Option<Uuid>,
    pub topic: Option<String>,
    pub prompt_text: String,
    pub generated_by: String,
    pub generated_at: DateTime<Utc>,
}

/// Snapshot of the configuration that produced this session.
/// Stored as JSONB so changes to the live config don't mutate in-flight sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub tech_count: u16,
    pub behavioral_count: u16,
    pub include_intro: bool,
    pub pass_threshold: i16,
    pub behavioral_bank: serde_json::Value,
}

/// Stable intro question text per spec.
pub const INTRO_QUESTION_TEXT: &str =
    "Tell me about yourself and walk me through your experience.";
