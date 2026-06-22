use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub expires_in: i64,
    pub account_id: Uuid,
    pub role: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AcceptInviteRequest {
    pub token: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateInviteRequest {
    pub email: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "hr".into()
}

#[derive(Debug, Serialize)]
pub struct CreateInviteResponse {
    pub account_id: Uuid,
    pub token: String,
    pub accept_url: String,
    pub expires_in_hours: i64,
}

#[derive(Debug, Deserialize)]
pub struct ListInvitesQuery {
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InviteListResponse {
    pub invites: Vec<InviteListItem>,
}

#[derive(Debug, Serialize)]
pub struct InviteListItem {
    pub id: Uuid,
    pub account_id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    pub role: String,
    pub account_status: String,
    pub invited_by: Uuid,
    pub invited_by_email: Option<String>,
    pub status: String,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
