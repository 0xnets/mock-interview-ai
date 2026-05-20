use std::sync::Arc;
use std::time::Duration;

use ai::prompts::{self, TECH_QUESTIONS_PROMPT_VERSION};
use ai::{parse_json, AiProvider, Budget};
use persistence::repo_session::{self, BehavioralPick, SessionForPriming};
use persistence::PgPool;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde_json::Value as JsonValue;
use tokio::sync::Notify;
use uuid::Uuid;

use crate::config::Settings;

/// Background loop that picks up `pending` sessions, calls the AI provider
/// to generate technical questions + a scoring rubric, picks behavioral
/// questions from the session's snapshot, then transitions the session to
/// `primed`. Phase 4 will replace this with a Redis Streams consumer group.
pub struct PrimingWorker {
    pool: PgPool,
    provider: Arc<dyn AiProvider>,
    cfg: Settings,
    notify: Arc<Notify>,
}

impl PrimingWorker {
    pub fn new(pool: PgPool, provider: Arc<dyn AiProvider>, cfg: Settings) -> Self {
        Self {
            pool,
            provider,
            cfg,
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn notify_handle(&self) -> Arc<Notify> {
        self.notify.clone()
    }

    pub async fn run(self) {
        let interval = Duration::from_millis(self.cfg.prime_poll_interval_ms.max(50));
        tracing::info!(
            interval_ms = self.cfg.prime_poll_interval_ms,
            "priming worker started"
        );

        loop {
            let work_found = self.tick().await;

            if work_found {
                // Drain the pending backlog as fast as we can.
                continue;
            }

            tokio::select! {
                _ = tokio::time::sleep(interval) => {},
                _ = self.notify.notified() => {
                    tracing::debug!("priming worker woke on notify");
                },
            }
        }
    }

    /// Process one batch. Returns true if any work was done so the loop can
    /// re-poll immediately instead of sleeping.
    async fn tick(&self) -> bool {
        let pending =
            match repo_session::fetch_pending_for_priming(&self.pool, self.cfg.prime_batch_size)
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!(error = %e, "fetch_pending_for_priming failed");
                    return false;
                }
            };

        if pending.is_empty() {
            return false;
        }

        for session in pending {
            let session_id = session.id;
            if let Err(e) = self.prime_one(session).await {
                tracing::error!(error = %e, %session_id, "session priming failed; marking aborted");
                if let Err(e2) =
                    repo_session::mark_aborted(&self.pool, session_id, &format!("prime: {e}")).await
                {
                    tracing::error!(error = %e2, %session_id, "mark_aborted failed");
                }
            }
        }
        true
    }

    async fn prime_one(&self, session: SessionForPriming) -> anyhow::Result<()> {
        let tech_count = session.config_snapshot.tech_count.max(1);
        let behavioral_count = session.config_snapshot.behavioral_count;

        tracing::info!(session_id=%session.id, tech_count, behavioral_count, "priming session");

        // 1. Generate technical questions.
        let tech_sys = prompts::tech_questions_system(tech_count);
        let tech_user =
            prompts::tech_questions_user(&session.jd_text, &session.resume_text, tech_count);
        let tech_completion = self
            .provider
            .complete(
                &tech_sys,
                &tech_user,
                Budget::balanced(),
                &self.cfg.anthropic_model_priming,
            )
            .await?;
        let tech_raw: Vec<String> = parse_json(&tech_completion.text)?;
        let mut tech_questions: Vec<String> = tech_raw
            .into_iter()
            .map(|q| q.trim().to_string())
            .filter(|q| !q.is_empty())
            .collect();
        tech_questions.truncate(tech_count as usize);
        if tech_questions.is_empty() {
            anyhow::bail!("AI returned no technical questions");
        }

        // 2. Generate the scoring rubric.
        let rubric_sys = prompts::rubric_system();
        let rubric_user =
            prompts::rubric_user(&session.jd_text, &session.resume_text, &session.role_title);
        let rubric_completion = self
            .provider
            .complete(
                &rubric_sys,
                &rubric_user,
                Budget::rubric(),
                &self.cfg.anthropic_model_priming,
            )
            .await?;
        let scoring_context = strip_code_fences(rubric_completion.text.trim());

        // 3. Pick behavioral questions from the snapshot.
        let picks = pick_behavioral(
            session.id,
            &session.config_snapshot.behavioral_bank,
            behavioral_count,
        );

        // 4. Persist and flip state.
        let generated_by = format!(
            "ai:{}@{}",
            tech_completion.model, TECH_QUESTIONS_PROMPT_VERSION
        );
        repo_session::complete_priming(
            &self.pool,
            session.id,
            session.account_id,
            session.include_intro,
            &tech_questions,
            &picks,
            &scoring_context,
            &generated_by,
        )
        .await?;

        tracing::info!(
            session_id=%session.id,
            latency_ms = tech_completion.latency_ms + rubric_completion.latency_ms,
            input_tokens = tech_completion.usage.input_tokens + rubric_completion.usage.input_tokens,
            output_tokens = tech_completion.usage.output_tokens + rubric_completion.usage.output_tokens,
            "session primed",
        );

        Ok(())
    }
}

fn strip_code_fences(s: &str) -> String {
    s.trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string()
}

/// Picks one random question per topic, deterministic per session_id so
/// re-priming a session yields the same picks.
fn pick_behavioral(session_id: Uuid, bank: &JsonValue, desired_count: u16) -> Vec<BehavioralPick> {
    let mut rng = ChaCha8Rng::seed_from_u64(uuid_seed(session_id));

    let mut by_topic: Vec<(Option<String>, Vec<String>)> = Vec::new();
    match bank {
        JsonValue::Object(map) => {
            for (topic, list) in map {
                let questions = json_to_string_list(list);
                if !questions.is_empty() {
                    by_topic.push((Some(topic.clone()), questions));
                }
            }
        }
        JsonValue::Array(items) => {
            // Allow [{"topic": "...", "questions": [...]}] as an alternative shape.
            for item in items {
                let topic = item
                    .get("topic")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let questions = item
                    .get("questions")
                    .map(json_to_string_list)
                    .unwrap_or_default();
                if !questions.is_empty() {
                    by_topic.push((topic, questions));
                }
            }
        }
        _ => {}
    }

    if by_topic.is_empty() {
        return Vec::new();
    }

    // Shuffle topics so when the bank has more topics than `desired_count`,
    // we pick a different subset across sessions.
    by_topic.shuffle(&mut rng);

    let want = desired_count as usize;
    let mut picks = Vec::with_capacity(want);
    for (topic, questions) in by_topic.into_iter() {
        if picks.len() >= want {
            break;
        }
        if let Some(q) = questions.choose(&mut rng) {
            picks.push(BehavioralPick {
                topic,
                question: q.clone(),
            });
        }
    }
    picks
}

fn json_to_string_list(v: &JsonValue) -> Vec<String> {
    match v {
        JsonValue::Array(items) => items
            .iter()
            .filter_map(|i| i.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        JsonValue::String(s) => s
            .split('\n')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn uuid_seed(id: Uuid) -> u64 {
    let bytes = id.as_bytes();
    u64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}
