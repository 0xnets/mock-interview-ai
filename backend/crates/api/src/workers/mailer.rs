//! Phase 5 mailer worker. Consumes `mail.report` events from the outbox and
//! sends a notification email via Resend. The email body links back to the
//! API for the PDF (signed-URL endpoint) — we never inline the PDF as an
//! attachment so the candidate's mailbox stays small and the HR side never
//! caches a stale copy.

use anyhow::{anyhow, Context, Result};
use persistence::repo_audit;
use persistence::repo_reports::{self, ReportForRender};
use persistence::PgPool;
use serde_json::json;
use std::time::Instant;
use uuid::Uuid;

use crate::config::Settings;

#[derive(Clone)]
pub struct Mailer {
    http: reqwest::Client,
    cfg: Settings,
}

impl Mailer {
    pub fn new(cfg: Settings) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
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
        let html = render_html(&report, &self.cfg.web_base_url);
        let payload = json!({
            "from": self.cfg.mail_from_address,
            "to": [report.hr_email],
            "subject": format!(
                "Interview report: {} — {} ({}%)",
                report.candidate_name, report.role_title, report.overall
            ),
            "html": html,
            "reply_to": self.cfg.mail_reply_to,
        });

        let key = self
            .cfg
            .resend_api_key
            .as_deref()
            .ok_or_else(|| anyhow!("resend api key missing"))?;
        let resp = self
            .http
            .post("https://api.resend.com/emails")
            .bearer_auth(key)
            .json(&payload)
            .send()
            .await
            .context("resend send")?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            repo_reports::mark_mail_failed(
                pool,
                session_id,
                &format!("resend status={status}: {body}"),
            )
            .await
            .ok();
            crate::metrics::MAIL_SEND_DURATION_MS
                .with_label_values(&["failed"])
                .observe(started.elapsed().as_millis() as f64);
            return Err(anyhow!("resend status {status}: {body}"));
        }
        repo_reports::mark_mail_sent(pool, session_id).await?;
        crate::metrics::MAIL_SEND_DURATION_MS
            .with_label_values(&["sent"])
            .observe(started.elapsed().as_millis() as f64);
        let _ = repo_audit::write(
            pool,
            None,
            Some(session_id),
            "report.mail_dispatched",
            None,
            json!({}),
        )
        .await;
        Ok(())
    }
}

fn render_html(r: &ReportForRender, web_base_url: &str) -> String {
    let bullets = |items: &serde_json::Value| -> String {
        items
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| format!("<li>{}</li>", html_escape(s)))
                    .collect::<String>()
            })
            .unwrap_or_default()
    };
    let banner = if r.passed {
        format!(
            "<p style=\"color:#059669;font-weight:bold\">PASSED — {}% (threshold {}%)</p>",
            r.overall, r.pass_threshold
        )
    } else {
        format!(
            "<p style=\"color:#dc2626;font-weight:bold\">BELOW THRESHOLD — {}% (threshold {}%)</p>",
            r.overall, r.pass_threshold
        )
    };
    let pdf_link = format!(
        "{}/v1/interviews/{}/report.pdf",
        web_base_url.trim_end_matches('/'),
        r.session_id
    );
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
  <p style="margin-top:24px"><a href="{pdf_link}" style="color:#4f46e5">Download full PDF report</a></p>
</div>"#,
        name = html_escape(&r.candidate_name),
        role = html_escape(&r.role_title),
        banner = banner,
        tech = r.technical,
        beh = r.behavioral,
        summary = html_escape(&r.summary),
        strengths = bullets(&r.strengths),
        weaknesses = bullets(&r.weaknesses),
        actions = bullets(&r.action_items),
        pdf_link = pdf_link,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
