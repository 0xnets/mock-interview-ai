//! Retention worker queries.
//!
//! Each helper targets one table/column the application no longer needs once
//! the per-session report has been mailed. Eligibility is keyed on
//! `reports.mail_status = 'sent'` AND `reports.mail_sent_at < cutoff`, so a
//! session is only touched after the PDF report has actually left the system.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use sqlx::PgPool;

use crate::DbError;

fn cutoff(days: i64) -> DateTime<Utc> {
    Utc::now() - ChronoDuration::days(days.max(0))
}

/// Delete `transcript_checkpoints` then `transcript_chunks` for sessions whose
/// report mail was dispatched before the cutoff. Checkpoints reference chunks
/// via `last_row_id`, so they must go first.
pub async fn purge_transcripts(pool: &PgPool, older_than_days: i64) -> Result<u64, DbError> {
    let cutoff = cutoff(older_than_days);
    let mut tx = pool.begin().await?;

    let cp = sqlx::query(
        r#"
        DELETE FROM transcript_checkpoints
        WHERE session_id IN (
            SELECT session_id
            FROM reports
            WHERE mail_status = 'sent'
              AND mail_sent_at IS NOT NULL
              AND mail_sent_at < $1
        )
        "#,
    )
    .bind(cutoff)
    .execute(&mut *tx)
    .await?;

    let ch = sqlx::query(
        r#"
        DELETE FROM transcript_chunks
        WHERE session_id IN (
            SELECT session_id
            FROM reports
            WHERE mail_status = 'sent'
              AND mail_sent_at IS NOT NULL
              AND mail_sent_at < $1
        )
        "#,
    )
    .bind(cutoff)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(cp.rows_affected() + ch.rows_affected())
}

pub async fn purge_answers(pool: &PgPool, older_than_days: i64) -> Result<u64, DbError> {
    let cutoff = cutoff(older_than_days);
    let res = sqlx::query(
        r#"
        DELETE FROM answers
        WHERE session_id IN (
            SELECT session_id
            FROM reports
            WHERE mail_status = 'sent'
              AND mail_sent_at IS NOT NULL
              AND mail_sent_at < $1
        )
        "#,
    )
    .bind(cutoff)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// `questions.prompt_text` is `NOT NULL`; redact in place with `''`. The full
/// text is preserved inside `reports.raw_ai_output.per_question[].question`.
pub async fn redact_question_text(pool: &PgPool, older_than_days: i64) -> Result<u64, DbError> {
    let cutoff = cutoff(older_than_days);
    let res = sqlx::query(
        r#"
        UPDATE questions
        SET prompt_text = ''
        WHERE prompt_text <> ''
          AND session_id IN (
            SELECT session_id
            FROM reports
            WHERE mail_status = 'sent'
              AND mail_sent_at IS NOT NULL
              AND mail_sent_at < $1
        )
        "#,
    )
    .bind(cutoff)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// `question_grades.reasoning` is `NOT NULL`; redact in place. Final feedback
/// already lives in `reports.raw_ai_output.per_question[].feedback`.
pub async fn redact_grade_reasoning(pool: &PgPool, older_than_days: i64) -> Result<u64, DbError> {
    let cutoff = cutoff(older_than_days);
    let res = sqlx::query(
        r#"
        UPDATE question_grades qg
        SET reasoning = ''
        WHERE qg.reasoning <> ''
          AND qg.question_id IN (
            SELECT q.id
            FROM questions q
            JOIN reports r ON r.session_id = q.session_id
            WHERE r.mail_status = 'sent'
              AND r.mail_sent_at IS NOT NULL
              AND r.mail_sent_at < $1
        )
        "#,
    )
    .bind(cutoff)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// Redact `jd_text` and `resume_text` on `interview_sessions`. Both columns
/// are `NOT NULL`, so we overwrite with `''` rather than NULL.
pub async fn redact_session_pii(pool: &PgPool, older_than_days: i64) -> Result<u64, DbError> {
    let cutoff = cutoff(older_than_days);
    let res = sqlx::query(
        r#"
        UPDATE interview_sessions s
        SET jd_text = '',
            resume_text = ''
        WHERE (s.jd_text <> '' OR s.resume_text <> '')
          AND s.id IN (
            SELECT session_id
            FROM reports
            WHERE mail_status = 'sent'
              AND mail_sent_at IS NOT NULL
              AND mail_sent_at < $1
        )
        "#,
    )
    .bind(cutoff)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// Delete dispatched outbox rows older than the cutoff. Independent of any
/// session join — `dispatched_at` alone signals the row is inert.
pub async fn purge_outbox(pool: &PgPool, older_than_days: i64) -> Result<u64, DbError> {
    let cutoff = cutoff(older_than_days);
    let res = sqlx::query(
        r#"
        DELETE FROM event_outbox
        WHERE dispatched_at IS NOT NULL
          AND dispatched_at < $1
        "#,
    )
    .bind(cutoff)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

#[cfg(test)]
mod tests {
    //! Requires a Postgres server reachable via DATABASE_URL with permission
    //! to create databases. `cargo test -p persistence` is a no-op without one.

    use super::*;
    use chrono::Duration as ChronoDuration;
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::Uuid;

    const SEED_ACCOUNT: Uuid = Uuid::from_bytes([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7,
    ]);

    /// Insert a finalized session + report whose mail was sent `age_days` ago.
    /// Returns the new session id.
    async fn seed_finalized(pool: &PgPool, age_days: i64, shortcode: &str) -> Uuid {
        let session_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO interview_sessions
              (account_id, candidate_name, role_title, jd_text, resume_text,
               state, shortcode, expires_at, pass_threshold, hr_email, config_snapshot)
            VALUES ($1, 'Cand', 'Role', 'jd body', 'resume body',
                    'scored', $2, now() + interval '7 days', 70, 'hr@example.com', '{}'::jsonb)
            RETURNING id
            "#,
        )
        .bind(SEED_ACCOUNT)
        .bind(shortcode)
        .fetch_one(pool)
        .await
        .expect("insert session");

        sqlx::query(
            r#"
            INSERT INTO reports
              (session_id, overall, technical, behavioral, passed, summary,
               strengths, weaknesses, action_items, raw_ai_output, model,
               prompt_version, mail_status, mail_sent_at)
            VALUES ($1, 80, 80, 80, true, 'ok',
                    '[]'::jsonb, '[]'::jsonb, '[]'::jsonb, '{}'::jsonb, 'm',
                    'v1', 'sent', now() - ($2 || ' days')::interval)
            "#,
        )
        .bind(session_id)
        .bind(age_days.to_string())
        .execute(pool)
        .await
        .expect("insert report");

        session_id
    }

    async fn seed_question(pool: &PgPool, session_id: Uuid, ordinal: i16) -> Uuid {
        sqlx::query_scalar(
            r#"
            INSERT INTO questions
              (session_id, ordinal, kind, prompt_text, generated_by)
            VALUES ($1, $2, 'technical', 'real prompt body', 'seed')
            RETURNING id
            "#,
        )
        .bind(session_id)
        .bind(ordinal)
        .fetch_one(pool)
        .await
        .expect("insert question")
    }

    async fn seed_answer(pool: &PgPool, session_id: Uuid, question_id: Uuid) {
        sqlx::query(
            r#"
            INSERT INTO answers (question_id, session_id, transcript_text, duration_ms)
            VALUES ($1, $2, 'i said things', 1000)
            "#,
        )
        .bind(question_id)
        .bind(session_id)
        .execute(pool)
        .await
        .expect("insert answer");
    }

    async fn seed_chunk(pool: &PgPool, session_id: Uuid, question_id: Uuid, seq: i32) -> i64 {
        sqlx::query_scalar(
            r#"
            INSERT INTO transcript_chunks
              (session_id, question_id, seq, text, is_final, client_ts_ms,
               prev_hash, row_hash)
            VALUES ($1, $2, $3, 'chunk text', true, 0,
                    '\x00'::bytea, '\x01'::bytea)
            RETURNING id
            "#,
        )
        .bind(session_id)
        .bind(question_id)
        .bind(seq)
        .fetch_one(pool)
        .await
        .expect("insert chunk")
    }

    async fn seed_grade(pool: &PgPool, question_id: Uuid) {
        sqlx::query(
            r#"
            INSERT INTO question_grades
              (question_id, score, reasoning, model, prompt_version)
            VALUES ($1, 80, 'real reasoning blob', 'm', 'v1')
            "#,
        )
        .bind(question_id)
        .execute(pool)
        .await
        .expect("insert grade");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn purge_transcripts_only_for_old_sent_reports(pool: PgPool) {
        let old = seed_finalized(&pool, 30, "old-1").await;
        let recent = seed_finalized(&pool, 1, "new-1").await;

        let q_old = seed_question(&pool, old, 1).await;
        let q_recent = seed_question(&pool, recent, 1).await;
        let chunk_old = seed_chunk(&pool, old, q_old, 1).await;
        let _chunk_recent = seed_chunk(&pool, recent, q_recent, 1).await;
        sqlx::query(
            r#"
            INSERT INTO transcript_checkpoints
              (session_id, last_seq, last_row_id, row_hash, signature, key_id, chunk_count)
            VALUES ($1, 1, $2, '\x01'::bytea, '\x02'::bytea, 'k', 1)
            "#,
        )
        .bind(old)
        .bind(chunk_old)
        .execute(&pool)
        .await
        .expect("insert checkpoint");

        let n = purge_transcripts(&pool, 7).await.expect("purge");
        assert_eq!(n, 2, "1 checkpoint + 1 chunk for the old session");

        let remaining: i64 =
            sqlx::query_scalar("SELECT count(*) FROM transcript_chunks WHERE session_id = $1")
                .bind(recent)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(remaining, 1, "recent session's chunk is untouched");

        let again = purge_transcripts(&pool, 7).await.expect("purge twice");
        assert_eq!(again, 0, "idempotent");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn purge_answers_skips_recent(pool: PgPool) {
        let old = seed_finalized(&pool, 60, "old-2").await;
        let recent = seed_finalized(&pool, 5, "new-2").await;
        let q_old = seed_question(&pool, old, 1).await;
        let q_recent = seed_question(&pool, recent, 1).await;
        seed_answer(&pool, old, q_old).await;
        seed_answer(&pool, recent, q_recent).await;

        let n = purge_answers(&pool, 30).await.expect("purge");
        assert_eq!(n, 1);

        let left: i64 = sqlx::query_scalar("SELECT count(*) FROM answers")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(left, 1);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn redact_session_pii_clears_text(pool: PgPool) {
        let old = seed_finalized(&pool, 60, "old-3").await;
        let n = redact_session_pii(&pool, 30).await.expect("redact");
        assert_eq!(n, 1);

        let (jd, resume): (String, String) =
            sqlx::query_as("SELECT jd_text, resume_text FROM interview_sessions WHERE id = $1")
                .bind(old)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(jd, "");
        assert_eq!(resume, "");

        let again = redact_session_pii(&pool, 30).await.expect("redact twice");
        assert_eq!(again, 0, "idempotent: WHERE clause skips empty rows");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn redact_question_text_and_grade_reasoning(pool: PgPool) {
        let old = seed_finalized(&pool, 60, "old-4").await;
        let recent = seed_finalized(&pool, 1, "new-4").await;
        let q_old = seed_question(&pool, old, 1).await;
        let q_recent = seed_question(&pool, recent, 1).await;
        seed_grade(&pool, q_old).await;
        seed_grade(&pool, q_recent).await;

        let qn = redact_question_text(&pool, 30).await.expect("redact q");
        assert_eq!(qn, 1);
        let gn = redact_grade_reasoning(&pool, 30).await.expect("redact g");
        assert_eq!(gn, 1);

        let (prompt,): (String,) =
            sqlx::query_as("SELECT prompt_text FROM questions WHERE id = $1")
                .bind(q_recent)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(prompt, "real prompt body", "recent untouched");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn purge_outbox_only_dispatched_and_old(pool: PgPool) {
        use repo_outbox::{enqueue, mark_dispatched};
        let payload = json!({"session_id": Uuid::new_v4()});

        let stale = enqueue(&pool, "mail.report", &payload).await.unwrap();
        mark_dispatched(&pool, stale).await.unwrap();
        sqlx::query("UPDATE event_outbox SET dispatched_at = now() - interval '60 days' WHERE id = $1")
            .bind(stale)
            .execute(&pool)
            .await
            .unwrap();

        let fresh = enqueue(&pool, "mail.report", &payload).await.unwrap();
        mark_dispatched(&pool, fresh).await.unwrap();

        let _pending = enqueue(&pool, "mail.report", &payload).await.unwrap();

        let n = purge_outbox(&pool, 30).await.expect("purge");
        assert_eq!(n, 1);

        let left: i64 = sqlx::query_scalar("SELECT count(*) FROM event_outbox")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(left, 2);
    }

    // Suppress unused-import warning when the crate is built without the
    // extra helpers above.
    #[allow(unused)]
    use crate::repo_outbox;
    #[allow(unused)]
    fn _unused_chrono_ref(_: ChronoDuration) {}
}
