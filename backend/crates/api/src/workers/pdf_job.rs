//! Phase 5: outbox handler for `report.generate_pdf`. Renders the PDF,
//! uploads it to S3, persists the object key, then chains the mailer.

use anyhow::{anyhow, Context, Result};
use persistence::repo_outbox;
use persistence::repo_reports;
use persistence::PgPool;
use serde_json::json;
use uuid::Uuid;

use crate::config::Settings;
use crate::pdf::render_report_pdf;
use crate::storage::BlobStore;

pub struct PdfJob<'a> {
    pub pool: &'a PgPool,
    pub cfg: &'a Settings,
    pub blob: Option<&'a BlobStore>,
}

impl<'a> PdfJob<'a> {
    pub async fn run(&self, session_id: Uuid) -> Result<()> {
        if !self.cfg.feature_server_pdf {
            return Err(anyhow!("FEATURE_SERVER_PDF disabled"));
        }
        let store = self
            .blob
            .ok_or_else(|| anyhow!("S3 not configured; refusing to render PDF"))?;

        let report = repo_reports::fetch_for_render(self.pool, session_id)
            .await
            .context("load report for render")?;

        // Skip work if the PDF already exists and is rendered.
        if report.pdf_status == "rendered" {
            if report.pdf_object_key.is_some() {
                return self.enqueue_mail(session_id).await;
            }
        }

        let started = std::time::Instant::now();
        let pdf_bytes = render_report_pdf(&report).context("render_report_pdf")?;
        let key = object_key(session_id, &report.candidate_name);
        if let Err(e) = store.put_pdf(&key, &pdf_bytes).await {
            repo_reports::mark_pdf_failed(self.pool, session_id, &e.to_string())
                .await
                .ok();
            crate::metrics::PDF_RENDER_DURATION_MS
                .with_label_values(&["failed"])
                .observe(started.elapsed().as_millis() as f64);
            return Err(e);
        }
        repo_reports::mark_pdf_rendered(self.pool, session_id, &key).await?;
        crate::metrics::PDF_RENDER_DURATION_MS
            .with_label_values(&["rendered"])
            .observe(started.elapsed().as_millis() as f64);
        tracing::info!(%session_id, %key, bytes = pdf_bytes.len(), "report PDF uploaded");

        self.enqueue_mail(session_id).await
    }

    async fn enqueue_mail(&self, session_id: Uuid) -> Result<()> {
        if !self.cfg.feature_backend_mailer {
            return Ok(());
        }
        repo_outbox::enqueue(
            self.pool,
            "mail.report",
            &json!({ "session_id": session_id }),
        )
        .await
        .context("enqueue mail.report")?;
        Ok(())
    }
}

fn object_key(session_id: Uuid, candidate_name: &str) -> String {
    let safe = candidate_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>();
    let safe = safe.trim_matches('_');
    let safe = if safe.is_empty() { "candidate" } else { safe };
    format!("reports/{session_id}/{safe}.pdf")
}
