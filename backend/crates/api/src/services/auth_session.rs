use axum::{
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::Duration;
use persistence::{repo_audit, repo_auth};
use serde_json::json;
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::mint_refresh_token;
use crate::error::{ApiError, ApiResult};
use crate::models::auth::LoginResponse;

pub(crate) const REFRESH_COOKIE: &str = "mi_rt";

pub async fn issue_tokens(
    state: &AppState,
    account_id: &Uuid,
    role: &str,
    display_name: &Option<String>,
    jar: CookieJar,
    headers: &HeaderMap,
    event_id: Uuid,
    rotated_from: Option<String>,
) -> ApiResult<Response> {
    let keys = state
        .jwt
        .as_ref()
        .ok_or_else(|| ApiError::Internal("JWT keys not initialized".into()))?;

    let access_ttl_min = state.cfg.access_token_ttl_minutes;
    let refresh_ttl_days = state.cfg.refresh_token_ttl_days;

    let access = keys
        .mint_access(
            *account_id,
            role,
            event_id,
            Duration::minutes(access_ttl_min),
        )
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

pub fn build_refresh_cookie(state: &AppState, value: String, ttl_days: i64) -> Cookie<'static> {
    let mut c = Cookie::new(REFRESH_COOKIE.to_string(), value);
    c.set_http_only(true);
    c.set_same_site(SameSite::Strict);
    c.set_path("/v1/auth");
    c.set_secure(state.cfg.cookies_secure);
    c.set_max_age(time::Duration::days(ttl_days));
    c
}

pub fn clear_refresh_cookie(state: &AppState) -> Cookie<'static> {
    let mut c = Cookie::new(REFRESH_COOKIE.to_string(), "".to_string());
    c.set_http_only(true);
    c.set_same_site(SameSite::Strict);
    c.set_path("/v1/auth");
    c.set_secure(state.cfg.cookies_secure);
    c.set_max_age(time::Duration::seconds(0));
    c
}
