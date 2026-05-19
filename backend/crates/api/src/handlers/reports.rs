//! Phase 5: signed-URL redirect for the server-rendered PDF report.
//! Phase 6: log a `report.pdf_downloaded` audit row on every redirect.

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use persistence::{repo_audit, repo_reports};
use serde_json::json;
use uuid::Uuid;

use crate::{
    app::AppState,
    auth::Principal,
    error::{ApiError, ApiResult},
};

pub async fn get_report_pdf(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    principal: Option<axum::Extension<Principal>>,
) -> ApiResult<Response> {
    if !state.cfg.feature_server_pdf {
        return Err(ApiError::NotFound);
    }
    let Some(store) = state.blob_store.as_ref() else {
        return Err(ApiError::Internal(
            "blob store not configured; set S3_BUCKET".into(),
        ));
    };
    let key = repo_reports::fetch_pdf_object_key(&state.pools.read, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let url = store
        .presign_get(&key)
        .await
        .map_err(|e| ApiError::Internal(format!("presign_get failed: {e}")))?;

    let actor_id = principal.as_ref().map(|p| p.account_id);
    let event_id = principal.as_ref().map(|p| p.event_id);
    let _ = repo_audit::write(
        &state.pools.primary,
        actor_id,
        Some(id),
        "report.pdf_downloaded",
        event_id,
        json!({}),
    )
    .await;

    Ok(Redirect::to(&url).into_response())
}

pub async fn get_report_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Response> {
    let report = repo_reports::fetch_for_render(&state.pools.read, id).await?;
    let body = json!({
        "pdf_status": report.pdf_status,
        "mail_status": report.mail_status,
        "ready": report.pdf_object_key.is_some() && report.pdf_status == "rendered",
    });
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response())
}
