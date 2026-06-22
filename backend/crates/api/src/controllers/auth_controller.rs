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
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use axum_extra::extract::cookie::CookieJar;
use chrono::Duration;
use persistence::{repo_audit, repo_auth};
use serde_json::json;
use uuid::Uuid;

use crate::{
    app::AppState,
    auth::{hash_password, mint_refresh_token, role_at_least, verify_password, Principal},
    config::defaults::MIN_INVITE_PASSWORD_CHARS,
    error::{ApiError, ApiResult},
    models::auth::{
        AcceptInviteRequest, CreateInviteRequest, CreateInviteResponse, InviteListItem,
        InviteListResponse, ListInvitesQuery, LoginRequest,
    },
    services::auth_session::{self, REFRESH_COOKIE},
    validation::validate_email,
};

// ─── login ──────────────────────────────────────────────────────────────────

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(body): Json<LoginRequest>,
) -> ApiResult<impl IntoResponse> {
    let email = body.email.trim().to_lowercase();
    if body.password.is_empty() {
        return Err(ApiError::BadRequest(
            "email and password are required".into(),
        ));
    }
    validate_email(&email, "email").map_err(ApiError::BadRequest)?;

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

    auth_session::issue_tokens(
        &state,
        &account.id,
        &account.role,
        &account.display_name,
        jar,
        &headers,
        event_id,
        None,
    )
    .await
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
                let _ =
                    repo_auth::revoke_all_for_account(&state.pools.primary, row.account_id).await;
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
    auth_session::issue_tokens(
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

pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> ApiResult<impl IntoResponse> {
    if let Some(c) = jar.get(REFRESH_COOKIE) {
        let _ = repo_auth::revoke_refresh_token(&state.pools.primary, c.value()).await;
    }
    let cleared = auth_session::clear_refresh_cookie(&state);
    Ok((StatusCode::NO_CONTENT, jar.add(cleared)).into_response())
}

// ─── accept invite ──────────────────────────────────────────────────────────

pub async fn accept_invite(
    State(state): State<AppState>,
    Json(body): Json<AcceptInviteRequest>,
) -> ApiResult<impl IntoResponse> {
    if body.password.len() < MIN_INVITE_PASSWORD_CHARS {
        return Err(ApiError::BadRequest(format!(
            "password must be >= {MIN_INVITE_PASSWORD_CHARS} chars"
        )));
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

pub async fn create_invite(
    State(state): State<AppState>,
    principal: Principal,
    Json(body): Json<CreateInviteRequest>,
) -> ApiResult<Json<CreateInviteResponse>> {
    let email = body.email.trim().to_lowercase();
    validate_email(&email, "email").map_err(ApiError::BadRequest)?;
    if !matches!(body.role.as_str(), "hr" | "admin") {
        return Err(ApiError::BadRequest("role must be hr or admin".into()));
    }
    if body.role == "admin" && principal.role != "super_admin" {
        return Err(ApiError::Unauthorized);
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

// ─── admin: invite management ───────────────────────────────────────────────

pub async fn list_invites(
    State(state): State<AppState>,
    Query(query): Query<ListInvitesQuery>,
) -> ApiResult<Json<InviteListResponse>> {
    let status = normalized_invite_status(query.status.as_deref())?;
    let invites = repo_auth::list_invites(&state.pools.read, status)
        .await?
        .into_iter()
        .map(invite_item_from_detail)
        .collect();
    Ok(Json(InviteListResponse { invites }))
}

pub async fn revoke_invite(
    State(state): State<AppState>,
    principal: Principal,
    Path(invite_id): Path<Uuid>,
) -> ApiResult<impl IntoResponse> {
    let invite = repo_auth::find_invite(&state.pools.read, invite_id).await?;
    ensure_can_manage_role(&principal, &invite.role)?;
    if invite.status != "active" {
        return Err(ApiError::BadRequest(
            "only active pending invites can be revoked".into(),
        ));
    }
    repo_auth::revoke_invite(&state.pools.primary, invite_id, principal.account_id).await?;
    let _ = repo_audit::write(
        &state.pools.primary,
        Some(principal.account_id),
        None,
        "auth.invite_revoked",
        Some(principal.event_id),
        json!({ "invite_id": invite_id, "account_id": invite.account_id, "role": invite.role }),
    )
    .await;
    Ok((StatusCode::NO_CONTENT, ()).into_response())
}

pub async fn undo_revoke_invite(
    State(state): State<AppState>,
    principal: Principal,
    Path(invite_id): Path<Uuid>,
) -> ApiResult<impl IntoResponse> {
    let invite = repo_auth::find_invite(&state.pools.read, invite_id).await?;
    ensure_can_manage_role(&principal, &invite.role)?;
    if invite.status != "revoked" {
        return Err(ApiError::BadRequest(
            "only revoked pending invites can be restored".into(),
        ));
    }
    if invite.account_status == "disabled" {
        return Err(ApiError::BadRequest(
            "disabled accounts cannot have invites restored".into(),
        ));
    }
    repo_auth::undo_revoke_invite(&state.pools.primary, invite_id).await?;
    let _ = repo_audit::write(
        &state.pools.primary,
        Some(principal.account_id),
        None,
        "auth.invite_revoke_undone",
        Some(principal.event_id),
        json!({ "invite_id": invite_id, "account_id": invite.account_id, "role": invite.role }),
    )
    .await;
    Ok((StatusCode::NO_CONTENT, ()).into_response())
}

pub async fn resend_invite(
    State(state): State<AppState>,
    principal: Principal,
    Path(invite_id): Path<Uuid>,
) -> ApiResult<Json<CreateInviteResponse>> {
    let invite = repo_auth::find_invite(&state.pools.read, invite_id).await?;
    ensure_can_manage_role(&principal, &invite.role)?;
    if invite.status == "accepted" {
        return Err(ApiError::BadRequest(
            "accepted invites cannot be resent".into(),
        ));
    }
    if invite.account_status == "disabled" {
        return Err(ApiError::BadRequest(
            "disabled accounts cannot receive invites".into(),
        ));
    }

    repo_auth::revoke_pending_invites_for_account(
        &state.pools.primary,
        invite.account_id,
        principal.account_id,
    )
    .await?;

    let token = mint_refresh_token();
    let ttl_hours = state.cfg.invite_ttl_hours;
    repo_auth::create_invite(
        &state.pools.primary,
        &token,
        invite.account_id,
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
        "auth.invite_resent",
        Some(principal.event_id),
        json!({ "source_invite_id": invite_id, "account_id": invite.account_id, "role": invite.role }),
    )
    .await;

    Ok(Json(CreateInviteResponse {
        account_id: invite.account_id,
        token,
        accept_url,
        expires_in_hours: ttl_hours,
    }))
}

pub async fn disable_account(
    State(state): State<AppState>,
    principal: Principal,
    Path(account_id): Path<Uuid>,
) -> ApiResult<impl IntoResponse> {
    if account_id == principal.account_id {
        return Err(ApiError::BadRequest(
            "you cannot disable your own account".into(),
        ));
    }

    let account = repo_auth::find_account_by_id(&state.pools.read, account_id).await?;
    ensure_can_manage_role(&principal, &account.role)?;
    if account.status == "disabled" {
        return Ok((StatusCode::NO_CONTENT, ()).into_response());
    }

    repo_auth::disable_account(&state.pools.primary, account_id).await?;
    repo_auth::revoke_all_for_account(&state.pools.primary, account_id).await?;
    let _ = repo_audit::write(
        &state.pools.primary,
        Some(principal.account_id),
        None,
        "auth.account_disabled",
        Some(principal.event_id),
        json!({ "account_id": account_id, "role": account.role }),
    )
    .await;

    Ok((StatusCode::NO_CONTENT, ()).into_response())
}

fn normalized_invite_status(status: Option<&str>) -> ApiResult<Option<&str>> {
    let Some(status) = status else {
        return Ok(None);
    };
    let status = status.trim();
    if status.is_empty() || status == "all" {
        return Ok(None);
    }
    if matches!(status, "active" | "expired" | "accepted" | "revoked") {
        return Ok(Some(status));
    }
    Err(ApiError::BadRequest("invalid invite status filter".into()))
}

fn ensure_can_manage_role(principal: &Principal, target_role: &str) -> ApiResult<()> {
    match target_role {
        "hr" if role_at_least(&principal.role, "admin") => Ok(()),
        "admin" if principal.role == "super_admin" => Ok(()),
        _ => Err(ApiError::Unauthorized),
    }
}

fn invite_item_from_detail(invite: repo_auth::InviteDetail) -> InviteListItem {
    InviteListItem {
        id: invite.id,
        account_id: invite.account_id,
        email: invite.email,
        display_name: invite.display_name,
        role: invite.role,
        account_status: invite.account_status,
        invited_by: invite.invited_by,
        invited_by_email: invite.invited_by_email,
        status: invite.status,
        expires_at: invite.expires_at,
        consumed_at: invite.consumed_at,
        revoked_at: invite.revoked_at,
        created_at: invite.created_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(role: &str) -> Principal {
        Principal {
            account_id: Uuid::new_v4(),
            role: role.to_string(),
            event_id: Uuid::new_v4(),
        }
    }

    #[test]
    fn admin_can_manage_hr_but_not_admin() {
        let admin = principal("admin");
        assert!(ensure_can_manage_role(&admin, "hr").is_ok());
        assert!(ensure_can_manage_role(&admin, "admin").is_err());
    }

    #[test]
    fn super_admin_can_manage_admin_and_hr() {
        let super_admin = principal("super_admin");
        assert!(ensure_can_manage_role(&super_admin, "hr").is_ok());
        assert!(ensure_can_manage_role(&super_admin, "admin").is_ok());
    }

    #[test]
    fn only_known_invite_status_filters_are_accepted() {
        assert_eq!(
            normalized_invite_status(Some("active")).unwrap(),
            Some("active")
        );
        assert_eq!(normalized_invite_status(Some("all")).unwrap(), None);
        assert!(normalized_invite_status(Some("pending")).is_err());
    }
}
