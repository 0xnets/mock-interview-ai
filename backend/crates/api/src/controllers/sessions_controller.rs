use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use persistence::repo_session;

use crate::{
    app::AppState,
    error::{ApiError, ApiResult},
    models::sessions::{
        JoinNonceResponse, ReportIncompatRequest, ReportIncompatResponse, SessionByCodeResponse,
    },
};

/// Public lookup by shortcode. Returns only state + display info — no JD,
/// resume, questions, or scoring rubric. The candidate hits `/join` next to
/// get a single-use WS nonce.
pub async fn get_by_code(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> ApiResult<Json<SessionByCodeResponse>> {
    let code = sanitize_code(&code).ok_or(ApiError::NotFound)?;
    let session = repo_session::find_by_shortcode(&state.pools.read, &code).await?;

    if session.expires_at < Utc::now() {
        return Err(ApiError::NotFound);
    }

    Ok(Json(SessionByCodeResponse {
        id: session.id,
        state: session.state,
        candidate_name: session.candidate_name,
        role_title: session.role_title,
        expires_at: session.expires_at,
        shortcode: session.shortcode,
        answer_time_limit_ms: session.answer_time_limit_ms,
    }))
}

/// Mint a single-use WebSocket join nonce for a session. The candidate uses
/// it to open `WSS /v1/ws/interview?token=<nonce>`. Issued even while the
/// session is still `pending` so the client can connect immediately; the WS
/// actor will wait for the session to become `primed` before sending
/// questions (Phase 5 will refine the pending-state UX).
///
/// The shortcode link is strictly one-use: this call atomically consumes it,
/// so the first candidate join wins and every later attempt with the same
/// code is rejected with `409 CONFLICT`, even before the session expires.
pub async fn issue_join_nonce(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> ApiResult<Json<JoinNonceResponse>> {
    let code = sanitize_code(&code).ok_or(ApiError::NotFound)?;
    // Consume the link first; only mint a nonce for the caller that won the
    // claim. A loser (Conflict) must never receive a usable nonce.
    let session = repo_session::consume_shortlink(&state.pools.primary, &code).await?;

    let nonce = state
        .nonces
        .mint(session.id)
        .await
        .map_err(|e| ApiError::Internal(format!("mint nonce: {e}")))?;

    Ok(Json(JoinNonceResponse {
        id: session.id,
        state: session.state,
        candidate_name: session.candidate_name,
        role_title: session.role_title,
        expires_at: session.expires_at,
        join_nonce: nonce,
        ws_path: "/v1/ws/interview",
        answer_time_limit_ms: session.answer_time_limit_ms,
    }))
}

/// Candidate-side report that a pre-interview system check failed. Increments
/// the link's incompatibility counter and, once the configured limit is hit,
/// expires the session so the candidate is routed to HR for a new link. This
/// does NOT consume the link — a candidate who fixes their setup before the
/// limit can still join.
pub async fn report_incompatibility(
    State(state): State<AppState>,
    Path(code): Path<String>,
    Json(req): Json<ReportIncompatRequest>,
) -> ApiResult<Json<ReportIncompatResponse>> {
    let code = sanitize_code(&code).ok_or(ApiError::NotFound)?;
    let limit = state.cfg.shortlink_incompat_limit;
    let outcome = repo_session::record_incompatibility(
        &state.pools.primary,
        &code,
        limit,
        &req.check,
        &req.detail,
    )
    .await?;

    Ok(Json(ReportIncompatResponse {
        incompat_count: outcome.incompat_count,
        limit: outcome.limit,
        attempts_remaining: (outcome.limit - outcome.incompat_count).max(0),
        expired: outcome.expired,
    }))
}

fn sanitize_code(raw: &str) -> Option<String> {
    if raw.is_empty() || raw.len() > 32 {
        return None;
    }
    if !raw.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(raw.to_string())
}
