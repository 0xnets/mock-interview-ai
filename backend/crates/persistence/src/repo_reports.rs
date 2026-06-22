//! Helpers for the mailer pipeline that runs off the outbox.

use chrono::{DateTime, Utc};
use std::collections::HashMap;

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
    pub mail_status: String,
}

pub async fn fetch_for_render(pool: &PgPool, session_id: Uuid) -> Result<ReportForRender, DbError> {
    let row = sqlx::query(
        r#"
        SELECT s.id, s.candidate_name, s.role_title, s.hr_email, s.pass_threshold,
               r.overall, r.technical, r.behavioral, r.passed, r.summary,
               r.strengths, r.weaknesses, r.action_items, r.raw_ai_output,
               r.generated_at, r.mail_status
        FROM reports r
        JOIN interview_sessions s ON s.id = r.session_id
        WHERE r.session_id = $1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    let row = row.ok_or(DbError::NotFound)?;
    let mut raw_ai_output: JsonValue = row.try_get("raw_ai_output")?;
    let answer_rows: Vec<(i16, Option<String>)> = sqlx::query_as(
        r#"
        SELECT q.ordinal, a.transcript_text
        FROM questions q
        LEFT JOIN answers a ON a.question_id = q.id
        WHERE q.session_id = $1
        ORDER BY
            COALESCE(
                (
                    SELECT parent.ordinal
                    FROM questions parent
                    WHERE parent.id = q.parent_question_id
                ),
                q.ordinal
            ),
            CASE WHEN q.kind = 'followup' THEN 1 ELSE 0 END,
            q.ordinal
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    attach_answers_and_order_per_question(&mut raw_ai_output, answer_rows);

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
        raw_ai_output,
        generated_at: row.try_get::<DateTime<Utc>, _>("generated_at")?,
        mail_status: row.try_get("mail_status")?,
    })
}

fn attach_answers_and_order_per_question(
    raw_ai_output: &mut JsonValue,
    answer_rows: Vec<(i16, Option<String>)>,
) {
    let answers: HashMap<i64, String> = answer_rows
        .iter()
        .filter_map(|(ordinal, answer)| answer.clone().map(|answer| (*ordinal as i64, answer)))
        .collect();
    let order: HashMap<i64, usize> = answer_rows
        .iter()
        .enumerate()
        .map(|(idx, (ordinal, _))| (*ordinal as i64, idx))
        .collect();
    let Some(items) = raw_ai_output
        .get_mut("per_question")
        .and_then(|v| v.as_array_mut())
    else {
        return;
    };

    for item in items.iter_mut() {
        let Some(obj) = item.as_object_mut() else {
            continue;
        };
        let Some(q) = obj.get("q").and_then(|v| v.as_i64()) else {
            continue;
        };
        let already_has_answer = obj
            .get("answer")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.trim().is_empty());
        if already_has_answer {
            continue;
        }
        if let Some(answer) = answers.get(&q) {
            obj.insert("answer".to_string(), JsonValue::String(answer.clone()));
        }
    }

    items.sort_by_key(|item| {
        item.get("q")
            .and_then(|v| v.as_i64())
            .and_then(|q| order.get(&q).copied())
            .unwrap_or(usize::MAX)
    });
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

pub async fn mark_mail_failed(pool: &PgPool, session_id: Uuid, err: &str) -> Result<(), DbError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn attach_answers_and_orders_existing_report_questions() {
        let mut raw = json!({
            "per_question": [
                { "q": 3, "question": "Follow-up?" },
                { "q": 1, "question": "One?" },
                { "q": 2, "question": "Two?", "answer": "already present" }
            ]
        });

        attach_answers_and_order_per_question(
            &mut raw,
            vec![
                (1, Some("first answer".to_string())),
                (2, Some("replacement should not win".to_string())),
                (3, Some("follow-up answer".to_string())),
            ],
        );

        assert_eq!(raw["per_question"][0]["q"], 1);
        assert_eq!(raw["per_question"][1]["q"], 2);
        assert_eq!(raw["per_question"][2]["q"], 3);
        assert_eq!(raw["per_question"][0]["answer"], "first answer");
        assert_eq!(raw["per_question"][1]["answer"], "already present");
        assert_eq!(raw["per_question"][2]["answer"], "follow-up answer");
    }
}
