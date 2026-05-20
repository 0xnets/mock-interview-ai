use ai::prompts::{self, SUMMARY_PROMPT_VERSION};
use ai::{parse_json, Budget};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use domain::ConfigSnapshot;
use persistence::repo_outbox;
use persistence::repo_realtime::{self, GradedQuestion};
use persistence::repo_session::{self, NewReport, NewSession};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use uuid::Uuid;

use crate::{
    app::AppState,
    auth::HrPrincipal,
    error::{ApiError, ApiResult},
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

#[derive(Debug, Deserialize)]
pub struct CreateInterviewRequest {
    pub candidate_name: String,
    pub role_title: String,
    pub jd_text: String,
    pub resume_text: String,
    pub hr_email: String,
    #[serde(default = "default_include_intro")]
    pub include_intro: bool,
    #[serde(default)]
    pub tech_count: Option<u16>,
    #[serde(default)]
    pub behavioral_count: Option<u16>,
    #[serde(default)]
    pub pass_threshold: Option<i16>,
    /// `{ "topic": ["q1", "q2"] }` (or a list of `{topic, questions}` objects).
    /// The priming worker picks one random question per topic.
    pub behavioral_bank: JsonValue,
}

fn default_include_intro() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct CreateInterviewResponse {
    pub id: Uuid,
    pub shortcode: String,
    pub share_url: String,
    pub expires_at: DateTime<Utc>,
}

pub async fn create(
    State(state): State<AppState>,
    principal: HrPrincipal,
    Json(body): Json<CreateInterviewRequest>,
) -> ApiResult<impl IntoResponse> {
    validate_create(&body)?;

    let tech_count = body.tech_count.unwrap_or(10).clamp(MIN_TECH_COUNT, MAX_TECH_COUNT);
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

#[derive(Debug, Serialize)]
pub struct FinalizeResponse {
    pub report: ReportPayload,
}

#[derive(Debug, Serialize)]
pub struct ReportPayload {
    pub id: Uuid,
    pub session_id: Uuid,
    pub overall_percentage: i16,
    pub technical_score: i16,
    pub behavioral_score: i16,
    pub passed: bool,
    pub summary: String,
    pub strengths: JsonValue,
    pub weaknesses: JsonValue,
    pub action_items: JsonValue,
    pub per_question: JsonValue,
    pub model: String,
    pub prompt_version: String,
    pub generated_at: DateTime<Utc>,
}

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
    if !graded.iter().all(|q| q.grade.is_some() || q.answer_text.trim().is_empty()) {
        graded = repo_realtime::load_graded_session(&state.pools.read, id).await?;
    }

    let (technical, behavioral, overall) = compute_scores(&graded);
    let passed = overall >= session.pass_threshold;

    let per_question_entries = build_per_question(&graded);
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
        .complete(&sys, &user, Budget::scoring(), &state.cfg.anthropic_model_scoring)
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

/// Returns (technical, behavioral, overall) on a 0-100 scale. Technical
/// includes `followup` questions. Intro is excluded from both sub-scores
/// because the intro prompt is fixed and not a skill signal — but if neither
/// section yielded a graded question, we degrade gracefully by averaging
/// whatever grades we have.
fn compute_scores(graded: &[GradedQuestion]) -> (i16, i16, i16) {
    let mut tech_sum = 0i32;
    let mut tech_n = 0i32;
    let mut beh_sum = 0i32;
    let mut beh_n = 0i32;
    let mut other_sum = 0i32;
    let mut other_n = 0i32;
    for q in graded {
        let Some(g) = &q.grade else { continue };
        match q.kind.as_str() {
            "technical" | "followup" => {
                tech_sum += g.score as i32;
                tech_n += 1;
            }
            "behavioral" => {
                beh_sum += g.score as i32;
                beh_n += 1;
            }
            _ => {
                other_sum += g.score as i32;
                other_n += 1;
            }
        }
    }

    let technical = if tech_n > 0 {
        ((tech_sum + tech_n / 2) / tech_n) as i16
    } else if beh_n > 0 {
        ((beh_sum + beh_n / 2) / beh_n) as i16
    } else if other_n > 0 {
        ((other_sum + other_n / 2) / other_n) as i16
    } else {
        0
    };
    let behavioral = if beh_n > 0 {
        ((beh_sum + beh_n / 2) / beh_n) as i16
    } else if tech_n > 0 {
        ((tech_sum + tech_n / 2) / tech_n) as i16
    } else if other_n > 0 {
        ((other_sum + other_n / 2) / other_n) as i16
    } else {
        0
    };

    let overall = match (tech_n, beh_n) {
        (0, 0) => {
            if other_n > 0 {
                ((other_sum + other_n / 2) / other_n) as i16
            } else {
                0
            }
        }
        (_, 0) => technical,
        (0, _) => behavioral,
        _ => {
            let blended = (technical as f64) * 0.6 + (behavioral as f64) * 0.4;
            blended.round().clamp(0.0, 100.0) as i16
        }
    };

    (technical, behavioral, overall)
}

#[derive(Debug, Serialize)]
struct PerQuestionEntry {
    q: i16,
    section: &'static str,
    question: String,
    score: Option<i16>,
    feedback: String,
    /// Time the candidate took between question display and submit.
    /// `None` when no answer row exists (e.g. unanswered tail).
    duration_ms: Option<i32>,
    duration_seconds: Option<i64>,
    time_bucket: Option<&'static str>,
}

fn build_per_question(graded: &[GradedQuestion]) -> Vec<PerQuestionEntry> {
    graded
        .iter()
        .map(|q| {
            let section = match q.kind.as_str() {
                "intro" => "Intro",
                "technical" => "Technical",
                "behavioral" => "Behavioral",
                "followup" => "Follow-up",
                _ => "Question",
            };
            let duration_ms = q.duration_ms;
            let duration_seconds = duration_ms.map(|d| (d as i64) / 1000);
            let time_bucket = duration_ms.map(time_bucket);
            PerQuestionEntry {
                q: q.ordinal,
                section,
                question: q.prompt_text.clone(),
                score: q.grade.as_ref().map(|g| g.score),
                feedback: q
                    .grade
                    .as_ref()
                    .map(|g| g.reasoning.clone())
                    .unwrap_or_default(),
                duration_ms,
                duration_seconds,
                time_bucket,
            }
        })
        .collect()
}

/// Heuristic bands for how long a candidate took to answer a single question.
/// Used as report metadata only — never feeds back into grading.
fn time_bucket(duration_ms: i32) -> &'static str {
    match duration_ms {
        i32::MIN..=14_999 => "very_short",
        15_000..=120_000 => "normal",
        120_001..=240_000 => "long",
        _ => "very_long",
    }
}
