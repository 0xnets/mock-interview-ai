//! Phase 4 persistence helpers: question/answer reads for the realtime actor,
//! follow-up insertion, per-question grade upserts, and session state flips.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::DbError;

#[derive(Debug, Clone)]
pub struct QuestionRow {
    pub id: Uuid,
    pub ordinal: i16,
    pub kind: String,
    pub parent_question_id: Option<Uuid>,
    pub topic: Option<String>,
    pub prompt_text: String,
}

#[derive(Debug, Clone)]
pub struct SessionForActor {
    pub id: Uuid,
    pub account_id: Uuid,
    pub state: String,
    pub candidate_name: String,
    pub role_title: String,
    pub jd_text: String,
    pub resume_text: String,
    pub scoring_context: Option<String>,
    pub pass_threshold: i16,
    pub expires_at: DateTime<Utc>,
}

type SessionForActorRow = (
    Uuid,
    Uuid,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    i16,
    DateTime<Utc>,
);

pub async fn find_session_for_actor(
    pool: &PgPool,
    session_id: Uuid,
) -> Result<SessionForActor, DbError> {
    let row: Option<SessionForActorRow> = sqlx::query_as(
        r#"
        SELECT id, account_id, state, candidate_name, role_title, jd_text, resume_text,
               scoring_context, pass_threshold, expires_at
        FROM interview_sessions
        WHERE id = $1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    let row = row.ok_or(DbError::NotFound)?;
    Ok(SessionForActor {
        id: row.0,
        account_id: row.1,
        state: row.2,
        candidate_name: row.3,
        role_title: row.4,
        jd_text: row.5,
        resume_text: row.6,
        scoring_context: row.7,
        pass_threshold: row.8,
        expires_at: row.9,
    })
}

type QuestionQueryRow = (Uuid, i16, String, Option<Uuid>, Option<String>, String);

pub async fn list_questions(pool: &PgPool, session_id: Uuid) -> Result<Vec<QuestionRow>, DbError> {
    let rows: Vec<QuestionQueryRow> = sqlx::query_as(
        r#"
        SELECT id, ordinal, kind, parent_question_id, topic, prompt_text
        FROM questions q
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
    Ok(rows
        .into_iter()
        .map(
            |(id, ordinal, kind, parent_question_id, topic, prompt_text)| QuestionRow {
                id,
                ordinal,
                kind,
                parent_question_id,
                topic,
                prompt_text,
            },
        )
        .collect())
}

/// Move a session from `primed`/`paused` to `active`. Idempotent: returns Ok
/// even if the session is already active. Errors if the session is in a
/// terminal state.
pub async fn mark_session_active(pool: &PgPool, session_id: Uuid) -> Result<(), DbError> {
    let updated: Option<(String,)> = sqlx::query_as(
        r#"
        UPDATE interview_sessions
        SET state = 'active',
            started_at = COALESCE(started_at, now())
        WHERE id = $1
          AND state IN ('primed', 'active', 'paused')
        RETURNING state
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    if updated.is_none() {
        return Err(DbError::Conflict);
    }
    Ok(())
}

/// Move a session to `completed`. Called after the last submit. The finalize
/// endpoint will move it to `scored` once the report row is written.
pub async fn mark_session_completed(pool: &PgPool, session_id: Uuid) -> Result<(), DbError> {
    sqlx::query(
        r#"
        UPDATE interview_sessions
        SET state = 'completed',
            completed_at = COALESCE(completed_at, now())
        WHERE id = $1
          AND state IN ('active', 'primed')
        "#,
    )
    .bind(session_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Persist an answer for a question. Idempotent via `UNIQUE(question_id)` —
/// resubmitting the same ordinal returns Ok without changing the stored row.
pub async fn insert_answer(
    pool: &PgPool,
    question_id: Uuid,
    session_id: Uuid,
    transcript_text: &str,
    duration_ms: i32,
) -> Result<(), DbError> {
    sqlx::query(
        r#"
        INSERT INTO answers (question_id, session_id, transcript_text, duration_ms)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (question_id) DO NOTHING
        "#,
    )
    .bind(question_id)
    .bind(session_id)
    .bind(transcript_text)
    .bind(duration_ms)
    .execute(pool)
    .await?;
    Ok(())
}

/// Insert a follow-up question after `parent_question_id`. Uses the session's
/// current max ordinal + 1. Run inside a transaction so two concurrent
/// follow-up jobs can't race on ordinal.
pub async fn insert_followup(
    pool: &PgPool,
    session_id: Uuid,
    parent_question_id: Uuid,
    prompt_text: &str,
    generated_by: &str,
) -> Result<QuestionRow, DbError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;

    let next_ordinal: (Option<i16>,) = sqlx::query_as(
        r#"
        SELECT MAX(ordinal) FROM questions WHERE session_id = $1
        "#,
    )
    .bind(session_id)
    .fetch_one(&mut *tx)
    .await?;
    let ordinal = next_ordinal.0.unwrap_or(0) + 1;

    let row: (Uuid, i16, String, Option<Uuid>, Option<String>, String) = sqlx::query_as(
        r#"
        INSERT INTO questions (
            session_id, ordinal, kind, parent_question_id, prompt_text, generated_by
        ) VALUES ($1, $2, 'followup', $3, $4, $5)
        RETURNING id, ordinal, kind, parent_question_id, topic, prompt_text
        "#,
    )
    .bind(session_id)
    .bind(ordinal)
    .bind(parent_question_id)
    .bind(prompt_text)
    .bind(generated_by)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(QuestionRow {
        id: row.0,
        ordinal: row.1,
        kind: row.2,
        parent_question_id: row.3,
        topic: row.4,
        prompt_text: row.5,
    })
}

#[derive(Debug, Clone)]
pub struct FollowupInputs {
    pub session_id: Uuid,
    pub primary_question_text: String,
    pub answer_transcript: String,
    pub role_title: String,
    pub jd_text: String,
    pub resume_text: String,
}

pub async fn find_followup_inputs(
    pool: &PgPool,
    question_id: Uuid,
) -> Result<FollowupInputs, DbError> {
    let row: Option<(Uuid, String, String, String, String, String)> = sqlx::query_as(
        r#"
        SELECT s.id, q.prompt_text, COALESCE(a.transcript_text, ''),
               s.role_title, s.jd_text, s.resume_text
        FROM questions q
        JOIN interview_sessions s ON s.id = q.session_id
        LEFT JOIN answers a ON a.question_id = q.id
        WHERE q.id = $1
        "#,
    )
    .bind(question_id)
    .fetch_optional(pool)
    .await?;
    let row = row.ok_or(DbError::NotFound)?;
    Ok(FollowupInputs {
        session_id: row.0,
        primary_question_text: row.1,
        answer_transcript: row.2,
        role_title: row.3,
        jd_text: row.4,
        resume_text: row.5,
    })
}

#[derive(Debug, Clone)]
pub struct GradeInputs {
    pub question_text: String,
    pub question_kind: String,
    pub answer_transcript: String,
    pub scoring_context: Option<String>,
    pub role_title: String,
}

pub async fn find_grade_inputs(pool: &PgPool, question_id: Uuid) -> Result<GradeInputs, DbError> {
    let row: Option<(String, String, String, Option<String>, String)> = sqlx::query_as(
        r#"
        SELECT q.prompt_text, q.kind, COALESCE(a.transcript_text, ''),
               s.scoring_context, s.role_title
        FROM questions q
        JOIN interview_sessions s ON s.id = q.session_id
        LEFT JOIN answers a ON a.question_id = q.id
        WHERE q.id = $1
        "#,
    )
    .bind(question_id)
    .fetch_optional(pool)
    .await?;
    let row = row.ok_or(DbError::NotFound)?;
    Ok(GradeInputs {
        question_text: row.0,
        question_kind: row.1,
        answer_transcript: row.2,
        scoring_context: row.3,
        role_title: row.4,
    })
}

pub async fn upsert_grade(
    pool: &PgPool,
    question_id: Uuid,
    score: i16,
    reasoning: &str,
    model: &str,
    prompt_version: &str,
) -> Result<(), DbError> {
    sqlx::query(
        r#"
        INSERT INTO question_grades (question_id, score, reasoning, model, prompt_version)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (question_id) DO UPDATE
          SET score = EXCLUDED.score,
              reasoning = EXCLUDED.reasoning,
              model = EXCLUDED.model,
              prompt_version = EXCLUDED.prompt_version,
              graded_at = now()
        "#,
    )
    .bind(question_id)
    .bind(score)
    .bind(reasoning)
    .bind(model)
    .bind(prompt_version)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct GradedQuestion {
    pub question_id: Uuid,
    pub ordinal: i16,
    pub kind: String,
    pub prompt_text: String,
    pub answer_text: String,
    /// Time the candidate took between question display and submit, in
    /// milliseconds. `None` when the question has no answer row.
    pub duration_ms: Option<i32>,
    pub grade: Option<GradeRow>,
}

#[derive(Debug, Clone)]
pub struct GradeRow {
    pub score: i16,
    pub reasoning: String,
    pub model: String,
    pub prompt_version: String,
}

/// Load every question for a session along with its answer and (if present) its
/// grade. Used by the finalize endpoint to roll a final report up from
/// per-question grades produced during the interview.
type GradedQuestionRow = (
    Uuid,
    i16,
    String,
    String,
    Option<String>,
    Option<i32>,
    Option<i16>,
    Option<String>,
    Option<String>,
    Option<String>,
);

pub async fn load_graded_session(
    pool: &PgPool,
    session_id: Uuid,
) -> Result<Vec<GradedQuestion>, DbError> {
    let rows: Vec<GradedQuestionRow> = sqlx::query_as(
        r#"
        SELECT q.id, q.ordinal, q.kind, q.prompt_text,
               a.transcript_text, a.duration_ms,
               g.score, g.reasoning, g.model, g.prompt_version
        FROM questions q
        LEFT JOIN answers a ON a.question_id = q.id
        LEFT JOIN question_grades g ON g.question_id = q.id
        WHERE q.session_id = $1
        ORDER BY q.ordinal
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                ordinal,
                kind,
                prompt_text,
                answer_text,
                duration_ms,
                score,
                reasoning,
                model,
                prompt_version,
            )| {
                let grade = match (score, reasoning, model, prompt_version) {
                    (Some(s), Some(r), Some(m), Some(v)) => Some(GradeRow {
                        score: s,
                        reasoning: r,
                        model: m,
                        prompt_version: v,
                    }),
                    _ => None,
                };
                GradedQuestion {
                    question_id: id,
                    ordinal,
                    kind,
                    prompt_text,
                    answer_text: answer_text.unwrap_or_default(),
                    duration_ms,
                    grade,
                }
            },
        )
        .collect())
}
