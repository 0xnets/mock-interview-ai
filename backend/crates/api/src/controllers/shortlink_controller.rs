use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use persistence::repo_session;

use crate::{
    app::AppState,
    error::{ApiError, ApiResult},
};

pub async fn redirect_to_session(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> ApiResult<Response> {
    let code = sanitize_code(&code).ok_or(ApiError::NotFound)?;
    let session = repo_session::find_by_shortcode(&state.pools.read, &code).await?;

    if session.expires_at < Utc::now() {
        return Ok((StatusCode::GONE, "this interview link has expired").into_response());
    }

    // Fire-and-forget hit counter; failure must not break the redirect.
    let pool = state.pools.primary.clone();
    let code_clone = code.clone();
    tokio::spawn(async move {
        if let Err(err) = repo_session::increment_shortlink_hit(&pool, &code_clone).await {
            tracing::warn!(error = ?err, code = %code_clone, "shortlink hit counter failed");
        }
    });

    let target = format!(
        "{}/?session={}",
        state.cfg.web_base_url.trim_end_matches('/'),
        session.shortcode
    );

    Ok((StatusCode::FOUND, [(header::LOCATION, target)]).into_response())
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
