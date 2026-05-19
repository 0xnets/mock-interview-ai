//! Phase 5: helpers for the PDF + mailer pipeline that runs off the outbox.

use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::DbError;

#[derive(Debug, Clone)]
pub struct ReportForRender {
    pub session_id: Uuid,
    pub candidate_name: String,
    pub role_title: String,
    pub hr_email: String,
    pub pass_threshold: i16,
    pub overall: i16,
    pub technical: i16,
    pub behavioral: i16,
    pub passed: bool,
    pub summary: String,
    pub strengths: JsonValue,
    pub weaknesses: JsonValue,
    pub action_items: JsonValue,
    pub raw_ai_output: JsonValue,
    pub generated_at: DateTime<Utc>,
    pub pdf_object_key: Option<String>,
    pub pdf_status: String,
    pub mail_status: String,
}

pub async fn fetch_for_render(
    pool: &PgPool,
    session_id: Uuid,
) -> Result<ReportForRender, DbError> {
    let row = sqlx::query(
        r#"
        SELECT s.id, s.candidate_name, s.role_title, s.hr_email, s.pass_threshold,
               r.overall, r.technical, r.behavioral, r.passed, r.summary,
               r.strengths, r.weaknesses, r.action_items, r.raw_ai_output,
               r.generated_at, r.pdf_object_key, r.pdf_status, r.mail_status
        FROM reports r
        JOIN interview_sessions s ON s.id = r.session_id
        WHERE r.session_id = $1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    let row = row.ok_or(DbError::NotFound)?;
    Ok(ReportForRender {
        session_id: row.try_get::<Uuid, _>("id")?,
        candidate_name: row.try_get("candidate_name")?,
        role_title: row.try_get("role_title")?,
        hr_email: row.try_get("hr_email")?,
        pass_threshold: row.try_get("pass_threshold")?,
        overall: row.try_get("overall")?,
        technical: row.try_get("technical")?,
        behavioral: row.try_get("behavioral")?,
        passed: row.try_get("passed")?,
        summary: row.try_get("summary")?,
        strengths: row.try_get("strengths")?,
        weaknesses: row.try_get("weaknesses")?,
        action_items: row.try_get("action_items")?,
        raw_ai_output: row.try_get("raw_ai_output")?,
        generated_at: row.try_get::<DateTime<Utc>, _>("generated_at")?,
        pdf_object_key: row.try_get("pdf_object_key")?,
        pdf_status: row.try_get("pdf_status")?,
        mail_status: row.try_get("mail_status")?,
    })
}

pub async fn mark_pdf_rendered(
    pool: &PgPool,
    session_id: Uuid,
    object_key: &str,
) -> Result<(), DbError> {
    sqlx::query(
        r#"
        UPDATE reports
        SET pdf_object_key = $2,
            pdf_status = 'rendered',
            pdf_generated_at = now(),
            pdf_error = NULL
        WHERE session_id = $1
        "#,
    )
    .bind(session_id)
    .bind(object_key)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_pdf_failed(
    pool: &PgPool,
    session_id: Uuid,
    err: &str,
) -> Result<(), DbError> {
    sqlx::query(
        r#"
        UPDATE reports
        SET pdf_status = 'failed',
            pdf_error = $2
        WHERE session_id = $1
        "#,
    )
    .bind(session_id)
    .bind(err)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_mail_sent(pool: &PgPool, session_id: Uuid) -> Result<(), DbError> {
    sqlx::query(
        r#"
        UPDATE reports
        SET mail_status = 'sent',
            mail_sent_at = now(),
            mail_error = NULL
        WHERE session_id = $1
        "#,
    )
    .bind(session_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_mail_failed(
    pool: &PgPool,
    session_id: Uuid,
    err: &str,
) -> Result<(), DbError> {
    sqlx::query(
        r#"
        UPDATE reports
        SET mail_status = 'failed',
            mail_error = $2
        WHERE session_id = $1
        "#,
    )
    .bind(session_id)
    .bind(err)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fetch_pdf_object_key(
    pool: &PgPool,
    session_id: Uuid,
) -> Result<Option<String>, DbError> {
    let row: Option<(Option<String>, String)> = sqlx::query_as(
        r#"
        SELECT pdf_object_key, pdf_status
        FROM reports
        WHERE session_id = $1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|(k, status)| if status == "rendered" { k } else { None }))
}
