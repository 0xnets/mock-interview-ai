//! Phase 6 auth handlers.
//!
//!   POST /v1/auth/login            email + password → access JWT + refresh cookie
//!   POST /v1/auth/refresh          refresh cookie   → new access JWT + rotated refresh cookie
//!   POST /v1/auth/logout           revokes the presented refresh token
//!   POST /v1/auth/accept-invite    sets a password for an invited account
//!   POST /v1/admin/invites         admin-only: creates an invited account + link
//!
//! Refresh tokens live in an HttpOnly + SameSite=Strict cookie so they can't
//! be exfiltrated via XSS. Access tokens go in the response body and the
//! frontend stashes them in memory (never localStorage).

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::Duration;
use persistence::{repo_audit, repo_auth};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    app::AppState,
    auth::{hash_password, mint_refresh_token, verify_password, Principal},
    error::{ApiError, ApiResult},
};

const REFRESH_COOKIE: &str = "mi_rt";

// ─── login ──────────────────────────────────────────────────────────────────

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

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(body): Json<LoginRequest>,
) -> ApiResult<impl IntoResponse> {
    let email = body.email.trim().to_lowercase();
    if email.is_empty() || body.password.is_empty() {
        return Err(ApiError::BadRequest("email and password are required".into()));
    }

    let event_id = Uuid::new_v4();

    let account = match repo_auth::find_account_by_email(&state.pools.read, &email).await {
        Ok(a) => a,
        Err(_) => {
            // Always burn at least a hash so the timing of "no such account"
            // and "wrong password" is indistinguishable.
            let _ = verify_password(&body.password, "$argon2id$v=19$m=19456,t=2,p=1$cmFuZG9tc2FsdA$aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
            audit_login_failed(&state, &email, "no_such_account", event_id, &headers).await;
            return Err(ApiError::Unauthorized);
        }
    };

    if account.status != "active" {
        audit_login_failed(&state, &email, "account_inactive", event_id, &headers).await;
        return Err(ApiError::Unauthorized);
    }

    let hash = match account.password_hash.as_deref() {
        Some(h) if !h.is_empty() => h,
        _ => {
            audit_login_failed(&state, &email, "no_password_set", event_id, &headers).await;
            return Err(ApiError::Unauthorized);
        }
    };

    if !verify_password(&body.password, hash) {
        audit_login_failed(&state, &email, "bad_password", event_id, &headers).await;
        return Err(ApiError::Unauthorized);
    }

    issue_tokens(&state, &account.id, &account.role, &account.display_name, jar, &headers, event_id, None).await
}

async fn audit_login_failed(
    state: &AppState,
    email: &str,
    reason: &str,
    event_id: Uuid,
    headers: &HeaderMap,
) {
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let _ = repo_audit::write(
        &state.pools.primary,
        None,
        None,
        "auth.failed_login",
        Some(event_id),
        json!({ "email_hash": email_hash(email), "reason": reason, "ua": ua }),
    )
    .await;
}

fn email_hash(email: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(email.as_bytes());
    hex::encode(h.finalize())
}

// ─── refresh ────────────────────────────────────────────────────────────────

pub async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
) -> ApiResult<impl IntoResponse> {
    let raw = jar
        .get(REFRESH_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or(ApiError::Unauthorized)?;

    // Rotate atomically. Any reuse of an already-revoked token is treated as
    // theft and revokes the family.
    let account_id = match repo_auth::rotate_refresh_token(&state.pools.primary, &raw).await {
        Ok(id) => id,
        Err(_) => {
            if let Ok(row) = repo_auth::lookup_refresh_token(&state.pools.primary, &raw).await {
                let _ = repo_auth::revoke_all_for_account(&state.pools.primary, row.account_id).await;
                let _ = repo_audit::write(
                    &state.pools.primary,
                    Some(row.account_id),
                    None,
                    "auth.refresh_reuse",
                    None,
                    json!({}),
                )
                .await;
            }
            return Err(ApiError::Unauthorized);
        }
    };

    let account = repo_auth::find_account_by_id(&state.pools.read, account_id).await?;
    if account.status != "active" {
        return Err(ApiError::Unauthorized);
    }

    let event_id = Uuid::new_v4();
    issue_tokens(
        &state,
        &account.id,
        &account.role,
        &account.display_name,
        jar,
        &headers,
        event_id,
        Some(raw),
    )
    .await
}

// ─── logout ─────────────────────────────────────────────────────────────────

pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> ApiResult<impl IntoResponse> {
    if let Some(c) = jar.get(REFRESH_COOKIE) {
        let _ = repo_auth::revoke_refresh_token(&state.pools.primary, c.value()).await;
    }
    let cleared = clear_refresh_cookie(&state);
    Ok((StatusCode::NO_CONTENT, jar.add(cleared)).into_response())
}

// ─── accept invite ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AcceptInviteRequest {
    pub token: String,
    pub password: String,
}

pub async fn accept_invite(
    State(state): State<AppState>,
    Json(body): Json<AcceptInviteRequest>,
) -> ApiResult<impl IntoResponse> {
    if body.password.len() < 12 {
        return Err(ApiError::BadRequest("password must be >= 12 chars".into()));
    }
    let invite = repo_auth::consume_invite(&state.pools.primary, &body.token)
        .await
        .map_err(|_| ApiError::Unauthorized)?;
    let hash = hash_password(&body.password)?;
    repo_auth::set_password_hash(&state.pools.primary, invite.account_id, &hash).await?;
    let _ = repo_audit::write(
        &state.pools.primary,
        Some(invite.account_id),
        None,
        "auth.invite_consumed",
        None,
        json!({}),
    )
    .await;
    Ok((StatusCode::NO_CONTENT, ()).into_response())
}

// ─── admin: create invite ───────────────────────────────────────────────────

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

pub async fn create_invite(
    State(state): State<AppState>,
    principal: Principal,
    Json(body): Json<CreateInviteRequest>,
) -> ApiResult<Json<CreateInviteResponse>> {
    let email = body.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err(ApiError::BadRequest("invalid email".into()));
    }
    if !matches!(body.role.as_str(), "hr" | "admin") {
        return Err(ApiError::BadRequest("role must be hr or admin".into()));
    }
    let account_id = repo_auth::create_invited_account(
        &state.pools.primary,
        repo_auth::NewAccount {
            email: email.clone(),
            display_name: body.display_name.clone(),
            role: body.role.clone(),
            invited_by: principal.account_id,
        },
    )
    .await?;

    let token = mint_refresh_token();
    let ttl_hours = state.cfg.invite_ttl_hours;
    repo_auth::create_invite(
        &state.pools.primary,
        &token,
        account_id,
        principal.account_id,
        Duration::hours(ttl_hours),
    )
    .await?;

    let accept_url = format!(
        "{}/?invite={}",
        state.cfg.web_base_url.trim_end_matches('/'),
        token
    );

    let _ = repo_audit::write(
        &state.pools.primary,
        Some(principal.account_id),
        None,
        "auth.invite_created",
        Some(principal.event_id),
        json!({ "invitee": email_hash(&email), "role": body.role }),
    )
    .await;

    Ok(Json(CreateInviteResponse {
        account_id,
        token,
        accept_url,
        expires_in_hours: ttl_hours,
    }))
}

// ─── shared: cookie + JWT minting ───────────────────────────────────────────

async fn issue_tokens(
    state: &AppState,
    account_id: &Uuid,
    role: &str,
    display_name: &Option<String>,
    jar: CookieJar,
    headers: &HeaderMap,
    event_id: Uuid,
    rotated_from: Option<String>,
) -> ApiResult<axum::response::Response> {
    let keys = state
        .jwt
        .as_ref()
        .ok_or_else(|| ApiError::Internal("JWT keys not initialized".into()))?;

    let access_ttl_min = state.cfg.access_token_ttl_minutes;
    let refresh_ttl_days = state.cfg.refresh_token_ttl_days;

    let access = keys
        .mint_access(*account_id, role, event_id, Duration::minutes(access_ttl_min))
        .map_err(|e| ApiError::Internal(format!("mint access: {e}")))?;

    let raw_refresh = mint_refresh_token();
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok());
    repo_auth::insert_refresh_token(
        &state.pools.primary,
        &raw_refresh,
        *account_id,
        Duration::days(refresh_ttl_days),
        rotated_from.as_deref(),
        ua,
        None,
    )
    .await?;
    let _ = repo_auth::touch_last_login(&state.pools.primary, *account_id).await;

    let _ = repo_audit::write(
        &state.pools.primary,
        Some(*account_id),
        None,
        "auth.login",
        Some(event_id),
        json!({ "rotated": rotated_from.is_some() }),
    )
    .await;

    let body = LoginResponse {
        access_token: access,
        expires_in: access_ttl_min * 60,
        account_id: *account_id,
        role: role.to_string(),
        display_name: display_name.clone(),
    };

    let cookie = build_refresh_cookie(state, raw_refresh, refresh_ttl_days);
    Ok((StatusCode::OK, jar.add(cookie), Json(body)).into_response())
}

fn build_refresh_cookie(state: &AppState, value: String, ttl_days: i64) -> Cookie<'static> {
    let mut c = Cookie::new(REFRESH_COOKIE.to_string(), value);
    c.set_http_only(true);
    c.set_same_site(SameSite::Strict);
    c.set_path("/v1/auth");
    c.set_secure(state.cfg.cookies_secure);
    c.set_max_age(time::Duration::days(ttl_days));
    c
}

fn clear_refresh_cookie(state: &AppState) -> Cookie<'static> {
    let mut c = Cookie::new(REFRESH_COOKIE.to_string(), "".to_string());
    c.set_http_only(true);
    c.set_same_site(SameSite::Strict);
    c.set_path("/v1/auth");
    c.set_secure(state.cfg.cookies_secure);
    c.set_max_age(time::Duration::seconds(0));
    c
}
