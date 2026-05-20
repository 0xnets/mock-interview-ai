//! On-demand PDF report renderer. Loads the stored report, renders a PDF in
//! memory, and streams it back. Also serves as the fallback link target in
//! emails when the attached-PDF path can't be used.

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use persistence::{repo_audit, repo_reports};
use serde_json::json;
use uuid::Uuid;

use crate::{
    app::AppState,
    auth::Principal,
    error::{ApiError, ApiResult},
    pdf::render_report_pdf,
};

pub async fn get_report_pdf(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    principal: Option<axum::Extension<Principal>>,
) -> ApiResult<Response> {
    let report = repo_reports::fetch_for_render(&state.pools.read, id).await?;

    let pdf_bytes = render_report_pdf(&report)
        .map_err(|e| ApiError::Internal(format!("render_report_pdf failed: {e}")))?;

    let filename = pdf_filename(&report.candidate_name);

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

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{filename}\""),
            ),
        ],
        pdf_bytes,
    )
        .into_response())
}

fn pdf_filename(candidate_name: &str) -> String {
    let safe: String = candidate_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let safe = safe.trim_matches('_');
    let safe = if safe.is_empty() { "candidate" } else { safe };
    format!("interview-report-{safe}.pdf")
}
