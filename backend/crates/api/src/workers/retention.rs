//! Retention worker: deletes or redacts per-session data the application no
//! longer reads once `reports.mail_status = 'sent'`. Runs on a slow tick (hourly
//! by default) and is safe to run on multiple replicas — every operation is
//! idempotent.

use std::time::Duration;

use persistence::repo_retention;
use persistence::PgPool;

use crate::config::Settings;
use crate::infrastructure::metrics::{RETENTION_ROWS_AFFECTED_TOTAL, RETENTION_TICK_LATENCY_MS};

pub struct RetentionWorker {
    pool: PgPool,
    cfg: Settings,
}

impl RetentionWorker {
    pub fn new(pool: PgPool, cfg: Settings) -> Self {
        Self { pool, cfg }
    }

    pub async fn run(self) {
        let interval = Duration::from_secs(self.cfg.retention_poll_interval_secs.max(60));
        tracing::info!(
            interval_secs = interval.as_secs(),
            transcript_days = self.cfg.retention_transcript_days,
            answers_days = self.cfg.retention_answers_days,
            pii_days = self.cfg.retention_pii_days,
            outbox_days = self.cfg.retention_outbox_days,
            "retention worker started"
        );

        loop {
            let started = std::time::Instant::now();
            let status = match self.tick().await {
                Ok(()) => "ok",
                Err(e) => {
                    tracing::error!(error=%e, "retention tick failed");
                    "error"
                }
            };
            RETENTION_TICK_LATENCY_MS
                .with_label_values(&[status])
                .observe(started.elapsed().as_millis() as f64);
            tokio::time::sleep(interval).await;
        }
    }

    pub async fn tick(&self) -> anyhow::Result<()> {
        let transcripts =
            repo_retention::purge_transcripts(&self.pool, self.cfg.retention_transcript_days)
                .await?;
        RETENTION_ROWS_AFFECTED_TOTAL
            .with_label_values(&["transcripts", "delete"])
            .inc_by(transcripts);

        let answers =
            repo_retention::purge_answers(&self.pool, self.cfg.retention_answers_days).await?;
        RETENTION_ROWS_AFFECTED_TOTAL
            .with_label_values(&["answers", "delete"])
            .inc_by(answers);

        let questions =
            repo_retention::redact_question_text(&self.pool, self.cfg.retention_pii_days).await?;
        RETENTION_ROWS_AFFECTED_TOTAL
            .with_label_values(&["questions", "redact"])
            .inc_by(questions);

        let grades =
            repo_retention::redact_grade_reasoning(&self.pool, self.cfg.retention_pii_days).await?;
        RETENTION_ROWS_AFFECTED_TOTAL
            .with_label_values(&["question_grades", "redact"])
            .inc_by(grades);

        let sessions =
            repo_retention::redact_session_pii(&self.pool, self.cfg.retention_pii_days).await?;
        RETENTION_ROWS_AFFECTED_TOTAL
            .with_label_values(&["interview_sessions", "redact"])
            .inc_by(sessions);

        let outbox =
            repo_retention::purge_outbox(&self.pool, self.cfg.retention_outbox_days).await?;
        RETENTION_ROWS_AFFECTED_TOTAL
            .with_label_values(&["event_outbox", "delete"])
            .inc_by(outbox);

        tracing::info!(
            transcripts,
            answers,
            questions_redacted = questions,
            grades_redacted = grades,
            sessions_redacted = sessions,
            outbox,
            "retention tick complete"
        );
        Ok(())
    }
}
