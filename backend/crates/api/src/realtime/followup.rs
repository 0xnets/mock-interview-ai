//! Generate a follow-up question from the candidate's answer to a technical
//! primary. Failure falls back to a generic probe so the interview never stalls.

use std::sync::Arc;
use std::time::Duration;

use ai::prompts::{self, FOLLOWUP_PROMPT_VERSION};
use ai::{parse_json, AiProvider, Budget};
use persistence::repo_realtime::{self, FollowupInputs, QuestionRow};
use persistence::PgPool;
use serde::Deserialize;
use uuid::Uuid;

const FALLBACK: &str = "Can you walk me through the trade-offs of that approach in more detail?";

#[derive(Debug, Deserialize)]
struct FollowupRaw {
    #[serde(default)]
    question: String,
    #[serde(default)]
    probe_target: Option<String>,
}

/// Generate a follow-up for `parent_question_id` and insert it into the
/// session. Returns the inserted question row so the actor can stream it
/// straight to the client.
pub async fn generate_and_insert(
    pool: &PgPool,
    provider: Arc<dyn AiProvider>,
    model: &str,
    parent_question_id: Uuid,
) -> anyhow::Result<QuestionRow> {
    let inputs: FollowupInputs =
        repo_realtime::find_followup_inputs(pool, parent_question_id).await?;

    let sys = prompts::followup_system();
    let user = prompts::followup_user(
        &inputs.role_title,
        &inputs.jd_text,
        &inputs.primary_question_text,
        &inputs.answer_transcript,
    );

    // Tight budget: follow-ups are user-facing and need to be quick.
    let budget = Budget {
        max_tokens: 256,
        timeout: Duration::from_secs(15),
        temperature: 0.6,
    };

    let prompt_text = match provider.complete(&sys, &user, budget, model).await {
        Ok(completion) => match parse_json::<FollowupRaw>(&completion.text) {
            Ok(raw) if !raw.question.trim().is_empty() => raw.question.trim().to_string(),
            Ok(_) => {
                tracing::warn!(parent_question_id=%parent_question_id, "follow-up JSON missing question; using fallback");
                FALLBACK.to_string()
            }
            Err(e) => {
                tracing::warn!(parent_question_id=%parent_question_id, error=%e, "follow-up JSON parse failed; using fallback");
                FALLBACK.to_string()
            }
        },
        Err(e) => {
            tracing::warn!(parent_question_id=%parent_question_id, error=%e, "follow-up AI call failed; using fallback");
            FALLBACK.to_string()
        }
    };

    let generated_by = format!("ai:{}@{}", model, FOLLOWUP_PROMPT_VERSION);
    let row = repo_realtime::insert_followup(
        pool,
        inputs.session_id,
        parent_question_id,
        &prompt_text,
        &generated_by,
    )
    .await?;

    Ok(row)
}
