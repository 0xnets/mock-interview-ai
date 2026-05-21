//! Per-session WebSocket actor. Owns the cursor over the question list and
//! drives the interview state machine. Loads question state from Postgres on
//! attach so a reconnect resumes where the candidate left off.
//!
//! Single-pod-per-process for now; multi-pod coordination (Redlock + backplane)
//! is Phase 6 work.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use ai::AiProvider;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use persistence::repo_realtime::{self, QuestionRow, SessionForActor};
use persistence::repo_transcript;
use persistence::PgPool;
use uuid::Uuid;

use crate::config::Settings;
use crate::infrastructure::signing::TranscriptSigner;
use crate::realtime::followup;
use crate::realtime::grade;
use crate::realtime::protocol::{ClientMsg, ServerMsg};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);

/// Upper bound on a single answer's `duration_ms` before we clamp it. Picked
/// generously so legitimate slow answers aren't truncated; this only guards
/// against obviously broken client clocks or stale UI state.
const MAX_ANSWER_DURATION_MS: i32 = 60 * 60 * 1000;

pub struct ActorDeps {
    pub pool: PgPool,
    pub provider: Arc<dyn AiProvider>,
    pub cfg: Settings,
    pub signer: Arc<TranscriptSigner>,
}

pub async fn run_session(socket: WebSocket, session_id: Uuid, deps: ActorDeps) {
    if let Err(e) = run_session_inner(socket, session_id, deps).await {
        tracing::warn!(error=%e, %session_id, "realtime session ended with error");
    }
}

async fn run_session_inner(
    socket: WebSocket,
    session_id: Uuid,
    deps: ActorDeps,
) -> anyhow::Result<()> {
    let session = repo_realtime::find_session_for_actor(&deps.pool, session_id).await?;

    match session.state.as_str() {
        "primed" | "active" | "paused" => {}
        "completed" | "scored" => {
            // Allow a late reconnect to still receive results_ready.
        }
        other => {
            anyhow::bail!("session not joinable from state '{other}'");
        }
    }

    // Move to active up front. If we lose the race with another connection
    // (rare) we just continue — the DB state is consistent either way.
    let _ = repo_realtime::mark_session_active(&deps.pool, session_id).await;

    let questions = repo_realtime::list_questions(&deps.pool, session_id).await?;
    if questions.is_empty() {
        anyhow::bail!("session has no questions yet");
    }

    let mut actor = SessionActor::new(socket, session, questions, deps);
    actor.send_hello().await?;
    actor.advance_to_next_unanswered().await?;
    actor.run().await
}

struct SessionActor {
    socket: WebSocket,
    session: SessionForActor,
    questions: Vec<QuestionRow>,
    deps: ActorDeps,
    cursor: usize,
    /// Final-only transcript text accumulated per ordinal. Cleared when
    /// the answer for that ordinal is persisted.
    finals: HashMap<i16, String>,
    /// Latest interim text (overwritten as new interims arrive).
    interims: HashMap<i16, String>,
    /// Chunks appended to the hash chain since the last checkpoint.
    pending_checkpoint_chunks: i32,
}

impl SessionActor {
    fn new(
        socket: WebSocket,
        session: SessionForActor,
        questions: Vec<QuestionRow>,
        deps: ActorDeps,
    ) -> Self {
        Self {
            socket,
            session,
            questions,
            deps,
            cursor: 0,
            finals: HashMap::new(),
            interims: HashMap::new(),
            pending_checkpoint_chunks: 0,
        }
    }

    async fn send_hello(&mut self) -> anyhow::Result<()> {
        let total = self.planned_total_questions();
        let msg = ServerMsg::Hello {
            session_id: self.session.id.to_string(),
            candidate_name: self.session.candidate_name.clone(),
            role_title: self.session.role_title.clone(),
            state: self.session.state.clone(),
            total_questions: total,
        };
        self.send(msg).await
    }

    /// On (re)connect, fast-forward the cursor past any questions that already
    /// have a persisted answer. This makes the WS resumable across reloads.
    async fn advance_to_next_unanswered(&mut self) -> anyhow::Result<()> {
        let answered: Vec<i16> = sqlx::query_scalar(
            r#"
            SELECT q.ordinal
            FROM questions q
            JOIN answers a ON a.question_id = q.id
            WHERE q.session_id = $1
            ORDER BY q.ordinal
            "#,
        )
        .bind(self.session.id)
        .fetch_all(&self.deps.pool)
        .await?;

        let answered_set: std::collections::HashSet<i16> = answered.into_iter().collect();
        while self.cursor < self.questions.len()
            && answered_set.contains(&self.questions[self.cursor].ordinal)
        {
            self.cursor += 1;
        }
        self.send_current_question().await
    }

    async fn send_current_question(&mut self) -> anyhow::Result<()> {
        if self.cursor >= self.questions.len() {
            return self.finish().await;
        }
        let q = self.questions[self.cursor].clone();
        let question_number = (self.cursor + 1) as u32;
        let total_questions = self.planned_total_questions();
        let msg = if q.kind == "followup" {
            let parent_ordinal = self
                .questions
                .iter()
                .find(|p| Some(p.id) == q.parent_question_id)
                .map(|p| p.ordinal)
                .unwrap_or(q.ordinal - 1);
            ServerMsg::Followup {
                ordinal: q.ordinal,
                question_number,
                total_questions,
                parent_ordinal,
                text: q.prompt_text,
            }
        } else {
            ServerMsg::Question {
                ordinal: q.ordinal,
                question_number,
                total_questions,
                kind: q.kind,
                text: q.prompt_text,
            }
        };
        self.send(msg).await
    }

    fn planned_total_questions(&self) -> u32 {
        self.questions
            .iter()
            .map(|q| {
                if q.kind == "technical" {
                    2
                } else if q.kind == "followup" {
                    0
                } else {
                    1
                }
            })
            .sum()
    }

    async fn run(&mut self) -> anyhow::Result<()> {
        let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
        heartbeat.tick().await; // first tick is immediate; skip

        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    if self.socket.send(Message::Ping(Vec::new().into())).await.is_err() {
                        return Ok(());
                    }
                }
                msg = self.socket.recv() => {
                    let Some(msg) = msg else { return Ok(()) };
                    let msg = match msg {
                        Ok(m) => m,
                        Err(e) => {
                            tracing::debug!(error=%e, "ws recv error; closing");
                            return Ok(());
                        }
                    };
                    match msg {
                        Message::Text(t) => self.handle_text(t.as_str()).await?,
                        Message::Binary(_) => {
                            // Phase 5 may add binary audio; ignore for now.
                        }
                        Message::Ping(payload) => {
                            let _ = self.socket.send(Message::Pong(payload)).await;
                        }
                        Message::Pong(_) => {}
                        Message::Close(_) => return Ok(()),
                    }
                }
            }
        }
    }

    async fn handle_text(&mut self, text: &str) -> anyhow::Result<()> {
        let parsed: ClientMsg = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(e) => {
                tracing::debug!(error=%e, payload=%text, "bad client frame");
                return self
                    .send(ServerMsg::Error {
                        code: "BAD_FRAME".into(),
                        message: format!("invalid client message: {e}"),
                        terminal: false,
                    })
                    .await;
            }
        };

        match parsed {
            ClientMsg::Hello { .. } => Ok(()),
            ClientMsg::Ping => Ok(()),
            ClientMsg::Utterance {
                ordinal,
                text,
                is_final,
                ..
            } => {
                self.append_chunk(ordinal, &text, is_final).await;
                if is_final {
                    let buf = self.finals.entry(ordinal).or_default();
                    if !buf.is_empty() {
                        buf.push(' ');
                    }
                    buf.push_str(text.trim());
                    self.interims.remove(&ordinal);
                } else {
                    self.interims.insert(ordinal, text);
                }
                Ok(())
            }
            ClientMsg::Submit {
                ordinal,
                duration_ms,
            } => self.handle_submit(ordinal, duration_ms).await,
            ClientMsg::Skip { ordinal, .. } => {
                // Treat skip like submit with empty answer.
                self.finals.insert(ordinal, String::new());
                self.interims.remove(&ordinal);
                self.handle_submit(ordinal, 0).await
            }
        }
    }

    async fn handle_submit(&mut self, ordinal: i16, duration_ms: i32) -> anyhow::Result<()> {
        let Some(idx) = self.questions.iter().position(|q| q.ordinal == ordinal) else {
            return self
                .send(ServerMsg::Error {
                    code: "UNKNOWN_ORDINAL".into(),
                    message: format!("ordinal {ordinal} not in this session"),
                    terminal: false,
                })
                .await;
        };
        let q = self.questions[idx].clone();

        let mut answer = self.finals.remove(&ordinal).unwrap_or_default();
        if let Some(interim) = self.interims.remove(&ordinal) {
            if !interim.trim().is_empty() {
                if !answer.is_empty() {
                    answer.push(' ');
                }
                answer.push_str(interim.trim());
            }
        }

        let safe_duration_ms = duration_ms.clamp(0, MAX_ANSWER_DURATION_MS);
        if safe_duration_ms != duration_ms {
            tracing::warn!(
                session_id = %self.session.id,
                ordinal,
                submitted = duration_ms,
                clamped = safe_duration_ms,
                "answer duration_ms outside [0, MAX_ANSWER_DURATION_MS]; clamped"
            );
        }

        repo_realtime::insert_answer(
            &self.deps.pool,
            q.id,
            self.session.id,
            &answer,
            safe_duration_ms,
        )
        .await?;

        // End-of-answer checkpoint: signs whatever has accumulated since the
        // last checkpoint so the transcript chain is durable on every submit.
        self.checkpoint_if_due(true).await;

        // Fire grading in the background — finalize will pick it up later.
        let pool = self.deps.pool.clone();
        let provider = self.deps.provider.clone();
        let grade_model = self.deps.cfg.anthropic_model_grading.clone();
        let qid = q.id;
        tokio::spawn(async move {
            grade::grade_one(&pool, provider, &grade_model, qid).await;
        });

        // Technical primaries get one AI follow-up inserted before we advance.
        // The follow-up is appended to `self.questions` and becomes the next
        // cursor target.
        if q.kind == "technical" {
            self.send(ServerMsg::State {
                value: "thinking".into(),
            })
            .await?;

            let pool = self.deps.pool.clone();
            let provider = self.deps.provider.clone();
            let model = self.deps.cfg.anthropic_model_followup.clone();
            let followup_row = followup::generate_and_insert(&pool, provider, &model, q.id).await;

            match followup_row {
                Ok(row) => {
                    let insert_at = idx + 1;
                    self.questions.insert(insert_at, row);
                    // Ensure cursor lands on the follow-up next.
                    self.cursor = insert_at;
                    self.send(ServerMsg::State {
                        value: "active".into(),
                    })
                    .await?;
                    return self.send_current_question().await;
                }
                Err(e) => {
                    tracing::warn!(error=%e, "follow-up insert failed; skipping to next primary");
                    // Fall through to advance past this question.
                }
            }
        }

        // Advance the cursor past this ordinal (and any other already-answered
        // ones in case of out-of-order submits).
        self.cursor = idx + 1;
        self.send(ServerMsg::State {
            value: "active".into(),
        })
        .await?;
        self.send_current_question().await
    }

    async fn finish(&mut self) -> anyhow::Result<()> {
        let _ = repo_realtime::mark_session_completed(&self.deps.pool, self.session.id).await;
        self.send(ServerMsg::ResultsReady {
            session_id: self.session.id.to_string(),
        })
        .await
    }

    async fn send(&mut self, msg: ServerMsg) -> anyhow::Result<()> {
        let payload = serde_json::to_string(&msg)?;
        self.socket
            .send(Message::Text(Utf8Bytes::from(payload)))
            .await
            .map_err(|e| anyhow::anyhow!("ws send failed: {e}"))
    }

    async fn append_chunk(&mut self, ordinal: i16, text: &str, is_final: bool) {
        if !self.deps.cfg.feature_transcript_chain {
            return;
        }
        if text.is_empty() {
            return;
        }
        let Some(qid) = self
            .questions
            .iter()
            .find(|q| q.ordinal == ordinal)
            .map(|q| q.id)
        else {
            tracing::debug!(ordinal, "utterance for unknown ordinal; chunk dropped");
            return;
        };
        let client_ts_ms = chrono::Utc::now().timestamp_millis();
        match repo_transcript::append_chunk(
            &self.deps.pool,
            self.session.id,
            qid,
            is_final,
            client_ts_ms,
            text,
        )
        .await
        {
            Ok(_) => {
                self.pending_checkpoint_chunks += 1;
                let label = if is_final { "true" } else { "false" };
                crate::infrastructure::metrics::TRANSCRIPT_CHUNKS_TOTAL
                    .with_label_values(&[label])
                    .inc();
                crate::infrastructure::metrics::WS_UTTERANCES_TOTAL
                    .with_label_values(&[label])
                    .inc();
                self.checkpoint_if_due(false).await;
            }
            Err(e) => {
                tracing::warn!(error=%e, session_id=%self.session.id, "transcript_chunks append failed");
            }
        }
    }

    /// Take a checkpoint when either the bucket of unsigned chunks has filled
    /// up or the caller forces it (end of answer). Either path is best-effort:
    /// the chain itself is intact even if we never sign — checkpoints are a
    /// performance/UX optimization for the verifier.
    async fn checkpoint_if_due(&mut self, force: bool) {
        if !self.deps.cfg.feature_transcript_chain {
            return;
        }
        let threshold = self.deps.cfg.transcript_checkpoint_every.max(1);
        if !force && self.pending_checkpoint_chunks < threshold {
            return;
        }
        if self.pending_checkpoint_chunks == 0 {
            return;
        }

        let pointer =
            match repo_transcript::latest_chunk_pointer(&self.deps.pool, self.session.id).await {
                Ok(Some(p)) => p,
                Ok(None) => return,
                Err(e) => {
                    tracing::warn!(error=%e, "latest_chunk_pointer failed");
                    return;
                }
            };
        let (last_row_id, last_seq, row_hash) = pointer;
        let signature =
            self.deps
                .signer
                .sign_checkpoint(&self.session.id.to_string(), last_seq, &row_hash);
        let chunk_count = self.pending_checkpoint_chunks;
        if let Err(e) = repo_transcript::insert_checkpoint(
            &self.deps.pool,
            self.session.id,
            last_seq,
            last_row_id,
            &row_hash,
            &signature,
            self.deps.signer.key_id(),
            chunk_count,
        )
        .await
        {
            tracing::warn!(error=%e, "checkpoint insert failed");
            return;
        }
        self.pending_checkpoint_chunks = 0;
    }
}
