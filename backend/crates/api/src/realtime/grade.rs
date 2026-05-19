//! Per-question grading. Fired as a fire-and-forget task after each `submit`
//! so the final `/finalize` call can roll a report up from existing grades
//! instead of regrading the whole transcript.

use std::sync::Arc;

use ai::prompts::{self, GRADE_PROMPT_VERSION};
use ai::{parse_json, AiProvider, Budget};
use persistence::repo_realtime;
use persistence::PgPool;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct GradeRaw {
    #[serde(default)]
    score: f64,
    #[serde(default)]
    reasoning: String,
}

pub async fn grade_one(
    pool: &PgPool,
    provider: Arc<dyn AiProvider>,
    model: &str,
    question_id: Uuid,
) {
    if let Err(e) = grade_one_inner(pool, provider, model, question_id).await {
        tracing::warn!(error=%e, %question_id, "grading failed; finalize will fall back to inline");
    }
}

async fn grade_one_inner(
    pool: &PgPool,
    provider: Arc<dyn AiProvider>,
    model: &str,
    question_id: Uuid,
) -> anyhow::Result<()> {
    let inputs = repo_realtime::find_grade_inputs(pool, question_id).await?;
    if inputs.answer_transcript.trim().is_empty() {
        // Empty answer — grade as 0 without burning a model call.
        repo_realtime::upsert_grade(
            pool,
            question_id,
            0,
            "Candidate did not provide an answer.",
            "skip",
            GRADE_PROMPT_VERSION,
        )
        .await?;
        return Ok(());
    }

    let section = match inputs.question_kind.as_str() {
        "intro" => "Intro",
        "technical" => "Technical",
        "behavioral" => "Behavioral",
        "followup" => "Follow-up",
        other => other,
    };

    let sys = prompts::grade_system();
    let user = prompts::grade_user(
        section,
        &inputs.question_text,
        &inputs.answer_transcript,
        inputs.scoring_context.as_deref().unwrap_or(""),
        &inputs.role_title,
    );

    let completion = provider
        .complete(&sys, &user, Budget::rubric(), model)
        .await?;
    let raw: GradeRaw = parse_json(&completion.text)?;
    let score = raw.score.round().clamp(0.0, 100.0) as i16;
    let reasoning = if raw.reasoning.trim().is_empty() {
        "(no reasoning returned)".to_string()
    } else {
        raw.reasoning
    };

    repo_realtime::upsert_grade(
        pool,
        question_id,
        score,
        &reasoning,
        &completion.model,
        GRADE_PROMPT_VERSION,
    )
    .await?;

    Ok(())
}
