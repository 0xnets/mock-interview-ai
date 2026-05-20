use ai::prompts::{self, SUMMARY_PROMPT_VERSION};
use ai::{parse_json, Budget};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use domain::ConfigSnapshot;
use persistence::repo_outbox;
use persistence::repo_realtime;
use persistence::repo_session::{self, NewReport, NewSession};
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use uuid::Uuid;

use crate::{
    app::AppState,
    auth::HrPrincipal,
    error::{ApiError, ApiResult},
    models::interviews::{
        CreateInterviewRequest, CreateInterviewResponse, FinalizeResponse, ReportPayload,
    },
    realtime::grade,
    shortcode::generate_shortcode,
};

const MAX_JD_BYTES: usize = 64 * 1024;
const MAX_RESUME_BYTES: usize = 256 * 1024;
const MIN_TECH_COUNT: u16 = 1;
const MAX_TECH_COUNT: u16 = 20;
const MIN_BEHAVIORAL_COUNT: u16 = 0;
const MAX_BEHAVIORAL_COUNT: u16 = 20;
const MIN_PASS_THRESHOLD: i16 = 0;
const MAX_PASS_THRESHOLD: i16 = 100;

pub async fn create(
    State(state): State<AppState>,
    principal: HrPrincipal,
    Json(body): Json<CreateInterviewRequest>,
) -> ApiResult<impl IntoResponse> {
    validate_create(&body)?;

    let tech_count = body
        .tech_count
        .unwrap_or(10)
        .clamp(MIN_TECH_COUNT, MAX_TECH_COUNT);
    let behavioral_count = body
        .behavioral_count
        .unwrap_or(3)
        .clamp(MIN_BEHAVIORAL_COUNT, MAX_BEHAVIORAL_COUNT);
    let pass_threshold = body
        .pass_threshold
        .unwrap_or(state.cfg.default_pass_threshold)
        .clamp(MIN_PASS_THRESHOLD, MAX_PASS_THRESHOLD);

    let snapshot = ConfigSnapshot {
        tech_count,
        behavioral_count,
        include_intro: body.include_intro,
        pass_threshold,
        behavioral_bank: body.behavioral_bank,
    };

    let new_session = NewSession {
        account_id: principal.account_id,
        candidate_name: body.candidate_name.trim().to_string(),
        role_title: body.role_title.trim().to_string(),
        jd_text: body.jd_text,
        resume_text: body.resume_text,
        hr_email: body.hr_email.trim().to_string(),
        pass_threshold,
        include_intro: body.include_intro,
        config_snapshot: snapshot,
        session_ttl_hours: state.cfg.session_ttl_hours,
        shortcode_factory: generate_shortcode,
    };

    let created = repo_session::create_session(&state.pools.primary, new_session).await?;

    state.prime_notify.notify_one();

    let share_url = format!(
        "{}/?session={}",
        state.cfg.web_base_url.trim_end_matches('/'),
        created.shortcode
    );

    let response = CreateInterviewResponse {
        id: created.id,
        shortcode: created.shortcode,
        share_url,
        expires_at: created.expires_at,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

fn validate_create(req: &CreateInterviewRequest) -> Result<(), ApiError> {
    if req.candidate_name.trim().is_empty() {
        return Err(ApiError::BadRequest("candidate_name is required".into()));
    }
    if req.role_title.trim().is_empty() {
        return Err(ApiError::BadRequest("role_title is required".into()));
    }
    if req.hr_email.trim().is_empty() {
        return Err(ApiError::BadRequest("hr_email is required".into()));
    }
    if !req.hr_email.contains('@') {
        return Err(ApiError::BadRequest("hr_email looks invalid".into()));
    }
    if req.jd_text.trim().is_empty() {
        return Err(ApiError::BadRequest("jd_text is required".into()));
    }
    if req.resume_text.trim().is_empty() {
        return Err(ApiError::BadRequest("resume_text is required".into()));
    }
    if req.jd_text.len() > MAX_JD_BYTES {
        return Err(ApiError::BadRequest(format!(
            "jd_text exceeds {MAX_JD_BYTES} bytes"
        )));
    }
    if req.resume_text.len() > MAX_RESUME_BYTES {
        return Err(ApiError::BadRequest(format!(
            "resume_text exceeds {MAX_RESUME_BYTES} bytes"
        )));
    }
    match &req.behavioral_bank {
        JsonValue::Object(m) if !m.is_empty() => {}
        JsonValue::Array(a) if !a.is_empty() => {}
        _ => {
            return Err(ApiError::BadRequest(
                "behavioral_bank must be a non-empty object or array".into(),
            ));
        }
    }
    Ok(())
}

// ─── Finalize / scoring ─────────────────────────────────────────────────────

/// Finalize an interview. Answers were already streamed in via WS and graded
/// per-question by `worker-grade`; this endpoint just rolls those grades into
/// a final report. Any question without an answer-side grade is graded inline
/// before the summary call.
pub async fn finalize(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<FinalizeResponse>> {
    let session = repo_session::find_for_scoring(&state.pools.read, id).await?;

    match session.state.as_str() {
        "active" | "completed" | "scored" | "primed" | "paused" => {}
        "expired" => return Err(ApiError::BadRequest("session has expired".into())),
        "aborted" => return Err(ApiError::BadRequest("session is aborted".into())),
        _ => return Err(ApiError::Conflict),
    }

    let mut graded = repo_realtime::load_graded_session(&state.pools.read, id).await?;
    if graded.is_empty() {
        return Err(ApiError::Conflict);
    }

    // Inline-grade anything that's missing a grade but has an answer.
    let missing: Vec<Uuid> = graded
        .iter()
        .filter(|q| q.grade.is_none() && !q.answer_text.trim().is_empty())
        .map(|q| q.question_id)
        .collect();
    for qid in missing {
        grade::grade_one(
            &state.pools.primary,
            state.provider.clone(),
            &state.cfg.anthropic_model_grading,
            qid,
        )
        .await;
    }
    // Re-load so we pick up the just-written grade rows.
    if !graded
        .iter()
        .all(|q| q.grade.is_some() || q.answer_text.trim().is_empty())
    {
        graded = repo_realtime::load_graded_session(&state.pools.read, id).await?;
    }

    let (technical, behavioral, overall) = crate::services::scoring::compute_scores(&graded);
    let passed = overall >= session.pass_threshold;

    let per_question_entries = crate::services::scoring::build_per_question(&graded);
    let per_question = serde_json::to_value(&per_question_entries)
        .map_err(|e| ApiError::Internal(format!("serialize per_question: {e}")))?;

    // Aggregate answer-time metadata for HR. Presentation only — never fed
    // into scoring. Skipped/missing answers have None and are ignored.
    let durations_ms: Vec<i32> = graded.iter().filter_map(|q| q.duration_ms).collect();
    let total_answer_time_ms: i64 = durations_ms.iter().map(|d| *d as i64).sum();
    let answered_count = durations_ms.len() as i64;
    let average_answer_time_ms: i64 = if answered_count > 0 {
        total_answer_time_ms / answered_count
    } else {
        0
    };

    // Synthesize narrative sections.
    let graded_json = serde_json::to_string(&per_question)
        .map_err(|e| ApiError::Internal(format!("serialize grades: {e}")))?;

    let sys = prompts::summary_system();
    let user = prompts::summary_user(
        &session.role_title,
        &session.candidate_name,
        session.scoring_context.as_deref().unwrap_or(""),
        technical,
        behavioral,
        overall,
        &graded_json,
    );

    let completion = state
        .provider
        .complete(
            &sys,
            &user,
            Budget::scoring(),
            &state.cfg.anthropic_model_scoring,
        )
        .await
        .map_err(|e| ApiError::Internal(format!("AI summary failed: {e}")))?;

    let summary_raw: SummaryRaw = parse_json(&completion.text)
        .map_err(|e| ApiError::Internal(format!("AI returned unexpected JSON: {e}")))?;

    let raw_value = json!({
        "technical_score": technical,
        "behavioral_score": behavioral,
        "overall_percentage": overall,
        "per_question": per_question,
        "strengths": summary_raw.strengths,
        "weaknesses": summary_raw.weaknesses,
        "action_items": summary_raw.action_items,
        "summary": summary_raw.summary,
        "summary_model": completion.model,
        "graded_questions": graded.len(),
        "answered_count": answered_count,
        "total_answer_time_ms": total_answer_time_ms,
        "total_answer_time_seconds": total_answer_time_ms / 1000,
        "average_answer_time_ms": average_answer_time_ms,
        "average_answer_time_seconds": average_answer_time_ms / 1000,
    });

    let stored = repo_session::upsert_report(
        &state.pools.primary,
        NewReport {
            session_id: id,
            overall,
            technical,
            behavioral,
            passed,
            summary: summary_raw.summary.clone(),
            strengths: json!(summary_raw.strengths),
            weaknesses: json!(summary_raw.weaknesses),
            action_items: json!(summary_raw.action_items),
            raw_ai_output: raw_value,
            model: completion.model.clone(),
            prompt_version: SUMMARY_PROMPT_VERSION.to_string(),
        },
    )
    .await?;

    // Kick the mailer through the outbox. The mailer renders the PDF in
    // memory, attaches it to the email, and (if attachment fails) falls back
    // to a text-only email linking to /report.pdf. Failures retry with backoff.
    if state.cfg.feature_backend_mailer {
        if let Err(e) = repo_outbox::enqueue(
            &state.pools.primary,
            "mail.report",
            &json!({ "session_id": id }),
        )
        .await
        {
            tracing::error!(error=%e, %id, "outbox enqueue (mail.report) failed");
        }
    }

    Ok(Json(FinalizeResponse {
        report: ReportPayload {
            id: stored.id,
            session_id: stored.session_id,
            overall_percentage: stored.overall,
            technical_score: stored.technical,
            behavioral_score: stored.behavioral,
            passed: stored.passed,
            summary: stored.summary,
            strengths: stored.strengths,
            weaknesses: stored.weaknesses,
            action_items: stored.action_items,
            per_question,
            model: stored.model,
            prompt_version: stored.prompt_version,
            generated_at: stored.generated_at,
        },
    }))
}

#[derive(Debug, Deserialize)]
struct SummaryRaw {
    #[serde(default)]
    strengths: Vec<String>,
    #[serde(default)]
    weaknesses: Vec<String>,
    #[serde(default)]
    action_items: Vec<String>,
    #[serde(default)]
    summary: String,
}
