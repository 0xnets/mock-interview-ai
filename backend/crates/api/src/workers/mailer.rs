//! Mailer worker. Consumes `mail.report` events from the outbox, renders the
//! report PDF in-memory, and posts an email via Resend with the PDF as an
//! attachment. On Resend rejection (or PDF render failure), retries once with
//! a text-only body that links to the on-demand `/report.pdf` route.

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use persistence::repo_audit;
use persistence::repo_reports::{self, ReportForRender};
use persistence::PgPool;
use serde_json::{json, Value as JsonValue};
use std::time::Instant;
use uuid::Uuid;

use crate::config::Settings;
use crate::infrastructure::pdf::render_report_pdf;

#[derive(Clone)]
pub struct Mailer {
    http: reqwest::Client,
    cfg: Settings,
}

impl Mailer {
    pub fn new(cfg: Settings) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client builds"),
            cfg,
        }
    }

    pub fn enabled(&self) -> bool {
        self.cfg.feature_backend_mailer && self.cfg.resend_api_key.is_some()
    }

    pub async fn send_report(&self, pool: &PgPool, session_id: Uuid) -> Result<()> {
        if !self.enabled() {
            return Err(anyhow!("mailer disabled or RESEND_API_KEY missing"));
        }
        let started = Instant::now();
        let report = repo_reports::fetch_for_render(pool, session_id).await?;
        if report.mail_status == "sent" {
            return Ok(());
        }

        let pdf_link = format!(
            "{}/v1/interviews/{}/report.pdf",
            self.cfg.web_base_url.trim_end_matches('/'),
            report.session_id
        );
        let subject = format!(
            "Interview report: {} — {} ({}%)",
            report.candidate_name, report.role_title, report.overall
        );
        let key = self
            .cfg
            .resend_api_key
            .as_deref()
            .ok_or_else(|| anyhow!("resend api key missing"))?;

        // Primary path: render PDF and send as attachment.
        let pdf_result = render_report_pdf(&report).context("render_report_pdf");
        let mut used_fallback = false;
        let attempt = match pdf_result {
            Ok(bytes) => {
                let filename = pdf_filename(&report);
                let html = render_html_with_attachment(&report);
                let payload = build_payload(
                    &self.cfg,
                    &report.hr_email,
                    &subject,
                    &html,
                    Some((&filename, &bytes)),
                );
                self.post_resend(key, &payload).await
            }
            Err(e) => {
                tracing::warn!(%session_id, error=%e, "PDF render failed; falling back to link-only email");
                used_fallback = true;
                let html = render_html_link_only(&report, &pdf_link);
                let payload = build_payload(&self.cfg, &report.hr_email, &subject, &html, None);
                self.post_resend(key, &payload).await
            }
        };

        let attempt = match attempt {
            Ok(()) => Ok(()),
            Err(e) if !used_fallback => {
                // Attachment send rejected (size, content, transient). Retry
                // once with a text-only body so HR still gets the report link.
                tracing::warn!(%session_id, error=%e, "attachment send failed; retrying with link-only fallback");
                used_fallback = true;
                let html = render_html_link_only(&report, &pdf_link);
                let payload = build_payload(&self.cfg, &report.hr_email, &subject, &html, None);
                self.post_resend(key, &payload).await
            }
            Err(e) => Err(e),
        };

        match attempt {
            Ok(()) => {
                repo_reports::mark_mail_sent(pool, session_id).await?;
                crate::infrastructure::metrics::MAIL_SEND_DURATION_MS
                    .with_label_values(&["sent"])
                    .observe(started.elapsed().as_millis() as f64);
                let _ = repo_audit::write(
                    pool,
                    None,
                    Some(session_id),
                    "report.mail_dispatched",
                    None,
                    json!({ "fallback": used_fallback }),
                )
                .await;
                Ok(())
            }
            Err(e) => {
                let err = e.to_string();
                repo_reports::mark_mail_failed(pool, session_id, &err)
                    .await
                    .ok();
                crate::infrastructure::metrics::MAIL_SEND_DURATION_MS
                    .with_label_values(&["failed"])
                    .observe(started.elapsed().as_millis() as f64);
                Err(e)
            }
        }
    }

    async fn post_resend(&self, api_key: &str, payload: &JsonValue) -> Result<()> {
        let resp = self
            .http
            .post("https://api.resend.com/emails")
            .bearer_auth(api_key)
            .json(payload)
            .send()
            .await
            .context("resend send")?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!("resend status {status}: {body}"));
        }
        Ok(())
    }
}

fn build_payload(
    cfg: &Settings,
    to: &str,
    subject: &str,
    html: &str,
    attachment: Option<(&str, &[u8])>,
) -> JsonValue {
    let mut obj = json!({
        "from": cfg.mail_from_address,
        "to": [to],
        "subject": subject,
        "html": html,
        "reply_to": cfg.mail_reply_to,
    });
    if let Some((filename, bytes)) = attachment {
        obj["attachments"] = json!([{
            "filename": filename,
            "content": B64.encode(bytes),
            "content_type": "application/pdf",
        }]);
    }
    obj
}

fn pdf_filename(r: &ReportForRender) -> String {
    let safe: String = r
        .candidate_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let safe = safe.trim_matches('_');
    let safe = if safe.is_empty() { "candidate" } else { safe };
    format!("interview-report-{safe}.pdf")
}

fn render_html_with_attachment(r: &ReportForRender) -> String {
    let banner = banner(r);
    let strengths = bullets(&r.strengths);
    let weaknesses = bullets(&r.weaknesses);
    let actions = bullets(&r.action_items);
    format!(
        r#"<div style="font-family:Inter,Arial,sans-serif;max-width:640px;margin:auto;padding:24px;color:#1f2937">
  <h2 style="margin:0 0 8px 0">Interview report: {name}</h2>
  <p style="margin:0 0 16px 0;color:#4b5563">{role}</p>
  {banner}
  <p>Technical: {tech}%   |   Behavioral: {beh}%</p>
  <h3>Summary</h3>
  <p>{summary}</p>
  <h3>Strengths</h3><ul>{strengths}</ul>
  <h3>Weaknesses</h3><ul>{weaknesses}</ul>
  <h3>Action Items</h3><ul>{actions}</ul>
  <p style="margin-top:24px;color:#4b5563">The full PDF report is attached to this email.</p>
</div>"#,
        name = html_escape(&r.candidate_name),
        role = html_escape(&r.role_title),
        summary = html_escape(&r.summary),
        tech = r.technical,
        beh = r.behavioral,
    )
}

fn render_html_link_only(r: &ReportForRender, pdf_link: &str) -> String {
    let banner = banner(r);
    let strengths = bullets(&r.strengths);
    let weaknesses = bullets(&r.weaknesses);
    let actions = bullets(&r.action_items);
    format!(
        r#"<div style="font-family:Inter,Arial,sans-serif;max-width:640px;margin:auto;padding:24px;color:#1f2937">
  <h2 style="margin:0 0 8px 0">Interview report: {name}</h2>
  <p style="margin:0 0 16px 0;color:#4b5563">{role}</p>
  {banner}
  <p>Technical: {tech}%   |   Behavioral: {beh}%</p>
  <h3>Summary</h3>
  <p>{summary}</p>
  <h3>Strengths</h3><ul>{strengths}</ul>
  <h3>Weaknesses</h3><ul>{weaknesses}</ul>
  <h3>Action Items</h3><ul>{actions}</ul>
  <p style="margin-top:24px"><a href="{pdf_link}" style="color:#4f46e5">Download the full PDF report</a></p>
</div>"#,
        name = html_escape(&r.candidate_name),
        role = html_escape(&r.role_title),
        summary = html_escape(&r.summary),
        tech = r.technical,
        beh = r.behavioral,
    )
}

fn banner(r: &ReportForRender) -> String {
    if r.passed {
        format!(
            "<p style=\"color:#059669;font-weight:bold\">PASSED — {}% (threshold {}%)</p>",
            r.overall, r.pass_threshold
        )
    } else {
        format!(
            "<p style=\"color:#dc2626;font-weight:bold\">BELOW THRESHOLD — {}% (threshold {}%)</p>",
            r.overall, r.pass_threshold
        )
    }
}

fn bullets(items: &serde_json::Value) -> String {
    items
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| format!("<li>{}</li>", html_escape(s)))
                .collect::<String>()
        })
        .unwrap_or_default()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
