//! Phase 5: polling outbox dispatcher.
//!
//! Claims pending `event_outbox` rows under a short lease, routes by topic,
//! and either marks the row dispatched or schedules a backoff retry. The
//! lease lets us run multiple replicas safely (Phase 6 will replace this
//! with Redis Streams).

use std::sync::Arc;
use std::time::Duration;

use chrono::Duration as ChronoDuration;
use persistence::repo_outbox::{self, OutboxRow};
use persistence::PgPool;
use serde::Deserialize;
use uuid::Uuid;

use crate::config::Settings;
use crate::storage::BlobStore;
use crate::workers::mailer::Mailer;
use crate::workers::pdf_job::PdfJob;

pub struct OutboxWorker {
    pool: PgPool,
    cfg: Settings,
    blob: Option<Arc<BlobStore>>,
    mailer: Arc<Mailer>,
}

#[derive(Debug, Deserialize)]
struct SessionPayload {
    session_id: Uuid,
}

impl OutboxWorker {
    pub fn new(
        pool: PgPool,
        cfg: Settings,
        blob: Option<Arc<BlobStore>>,
        mailer: Arc<Mailer>,
    ) -> Self {
        Self {
            pool,
            cfg,
            blob,
            mailer,
        }
    }

    pub async fn run(self) {
        let interval = Duration::from_millis(self.cfg.outbox_poll_interval_ms.max(100));
        let lease = ChronoDuration::seconds(60);
        tracing::info!(
            interval_ms = self.cfg.outbox_poll_interval_ms,
            batch = self.cfg.outbox_batch_size,
            "outbox worker started"
        );

        loop {
            let rows = match repo_outbox::claim_batch(
                &self.pool,
                self.cfg.outbox_batch_size,
                lease,
            )
            .await
            {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error=%e, "outbox claim_batch failed");
                    tokio::time::sleep(interval).await;
                    continue;
                }
            };

            if rows.is_empty() {
                tokio::time::sleep(interval).await;
                continue;
            }

            for row in rows {
                self.handle(row).await;
            }
        }
    }

    async fn handle(&self, row: OutboxRow) {
        let topic = row.topic.clone();
        let id = row.id;
        let attempts = row.attempts;
        let max = self.cfg.outbox_max_attempts;
        let started = std::time::Instant::now();
        tracing::debug!(%id, %topic, attempts, "outbox dispatch");

        let result: anyhow::Result<()> = match topic.as_str() {
            "report.generate_pdf" => {
                let payload: SessionPayload = match serde_json::from_value(row.payload.clone()) {
                    Ok(p) => p,
                    Err(e) => {
                        repo_outbox::mark_failed(
                            &self.pool,
                            id,
                            &format!("bad payload: {e}"),
                            ChronoDuration::hours(24),
                        )
                        .await
                        .ok();
                        return;
                    }
                };
                let job = PdfJob {
                    pool: &self.pool,
                    cfg: &self.cfg,
                    blob: self.blob.as_deref(),
                };
                job.run(payload.session_id).await
            }
            "mail.report" => {
                let payload: SessionPayload = match serde_json::from_value(row.payload.clone()) {
                    Ok(p) => p,
                    Err(e) => {
                        repo_outbox::mark_failed(
                            &self.pool,
                            id,
                            &format!("bad payload: {e}"),
                            ChronoDuration::hours(24),
                        )
                        .await
                        .ok();
                        return;
                    }
                };
                self.mailer.send_report(&self.pool, payload.session_id).await
            }
            other => {
                tracing::warn!(%other, "outbox unknown topic; marking dispatched");
                repo_outbox::mark_dispatched(&self.pool, id).await.ok();
                return;
            }
        };

        let latency_ms = started.elapsed().as_millis() as f64;
        crate::metrics::OUTBOX_DISPATCH_LATENCY_MS
            .with_label_values(&[&topic])
            .observe(latency_ms);
        match result {
            Ok(()) => {
                crate::metrics::OUTBOX_ATTEMPTS_TOTAL
                    .with_label_values(&[&topic, "success"])
                    .inc();
                repo_outbox::mark_dispatched(&self.pool, id).await.ok();
            }
            Err(e) => {
                let err = e.to_string();
                crate::metrics::OUTBOX_ATTEMPTS_TOTAL
                    .with_label_values(&[&topic, "error"])
                    .inc();
                if attempts >= max {
                    tracing::error!(%id, %topic, attempts, error=%err, "outbox exceeded max attempts; parking");
                    // Push the lease far out so it stops retrying without
                    // losing the audit trail. Audit log entry would belong
                    // here when Phase 6 wires it up.
                    repo_outbox::mark_failed(
                        &self.pool,
                        id,
                        &err,
                        ChronoDuration::days(7),
                    )
                    .await
                    .ok();
                } else {
                    let backoff = backoff_for(attempts);
                    tracing::warn!(%id, %topic, attempts, error=%err, "outbox dispatch failed; will retry");
                    repo_outbox::mark_failed(&self.pool, id, &err, backoff)
                        .await
                        .ok();
                }
            }
        }
    }
}

/// Exponential backoff capped at 5 minutes.
fn backoff_for(attempts: i32) -> ChronoDuration {
    let secs = (2_i64.saturating_pow(attempts.clamp(1, 8) as u32)) * 5;
    let capped = secs.min(300);
    ChronoDuration::seconds(capped)
}
