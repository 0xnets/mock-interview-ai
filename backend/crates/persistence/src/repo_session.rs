use chrono::{DateTime, Duration, Utc};
use domain::{ConfigSnapshot, QuestionKind, SessionState, INTRO_QUESTION_TEXT};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::DbError;

#[derive(Debug, Clone)]
pub struct BehavioralPick {
    pub topic: Option<String>,
    pub question: String,
}

/// Inputs needed to start a session. From Phase 3 forward, questions are NOT
/// supplied by the caller — the priming worker fills them in asynchronously.
#[derive(Debug, Clone)]
pub struct NewSession {
    pub account_id: Uuid,
    pub candidate_name: String,
    pub role_title: String,
    pub jd_text: String,
    pub resume_text: String,
    pub hr_email: String,
    pub pass_threshold: i16,
    pub include_intro: bool,
    pub config_snapshot: ConfigSnapshot,
    pub session_ttl_hours: i64,
    pub shortcode_factory: fn() -> String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreatedSession {
    pub id: Uuid,
    pub shortcode: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionByCode {
    pub id: Uuid,
    pub state: String,
    pub candidate_name: String,
    pub role_title: String,
    pub expires_at: DateTime<Utc>,
    pub shortcode: String,
    pub answer_time_limit_ms: u32,
}

#[derive(Debug, Clone)]
pub struct SessionForPriming {
    pub id: Uuid,
    pub role_title: String,
    pub jd_text: String,
    pub resume_text: String,
    pub config_snapshot: ConfigSnapshot,
    pub include_intro: bool,
    pub account_id: Uuid,
}

pub async fn create_session(pool: &PgPool, req: NewSession) -> Result<CreatedSession, DbError> {
    let expires_at = Utc::now() + Duration::hours(req.session_ttl_hours);

    for _ in 0..5 {
        let mut tx: Transaction<'_, Postgres> = pool.begin().await?;
        let session_id = Uuid::new_v4();
        let shortcode = (req.shortcode_factory)();

        let config_snapshot =
            serde_json::to_value(&req.config_snapshot).expect("ConfigSnapshot serializes cleanly");

        let inserted: Option<(Uuid,)> = sqlx::query_as(
            r#"
            INSERT INTO interview_sessions (
                id, account_id, candidate_name, role_title,
                jd_text, resume_text, state, shortcode, expires_at,
                pass_threshold, hr_email, config_snapshot
            ) VALUES (
                $1, $2, $3, $4,
                $5, $6, $7, $8, $9,
                $10, $11, $12
            )
            ON CONFLICT (shortcode) DO NOTHING
            RETURNING id
            "#,
        )
        .bind(session_id)
        .bind(req.account_id)
        .bind(&req.candidate_name)
        .bind(&req.role_title)
        .bind(&req.jd_text)
        .bind(&req.resume_text)
        .bind(SessionState::Pending.as_str())
        .bind(&shortcode)
        .bind(expires_at)
        .bind(req.pass_threshold)
        .bind(&req.hr_email)
        .bind(&config_snapshot)
        .fetch_optional(&mut *tx)
        .await?;

        if inserted.is_none() {
            tx.rollback().await?;
            continue;
        }

        sqlx::query(
            r#"
            INSERT INTO shortlinks (code, session_id) VALUES ($1, $2)
            "#,
        )
        .bind(&shortcode)
        .bind(session_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO audit_log (actor_id, session_id, action, metadata)
            VALUES ($1, $2, 'session.created', $3)
            "#,
        )
        .bind(req.account_id)
        .bind(session_id)
        .bind(serde_json::json!({
            "include_intro": req.include_intro,
            "pass_threshold": req.pass_threshold,
        }))
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        return Ok(CreatedSession {
            id: session_id,
            shortcode,
            expires_at,
        });
    }

    Err(DbError::Conflict)
}

/// Atomically claim the next pending session for priming. Returns None when
/// nothing is pending. The session stays in `pending` so a crash mid-prime
/// leaves it claimable again next loop iteration.
pub async fn fetch_pending_for_priming(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<SessionForPriming>, DbError> {
    let rows: Vec<(Uuid, String, String, String, serde_json::Value, Uuid)> = sqlx::query_as(
        r#"
        SELECT id, role_title, jd_text, resume_text, config_snapshot, account_id
        FROM interview_sessions
        WHERE state = 'pending'
          AND expires_at > now()
        ORDER BY created_at
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for (id, role_title, jd_text, resume_text, snapshot_json, account_id) in rows {
        let snapshot: ConfigSnapshot = match serde_json::from_value(snapshot_json) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(session_id=%id, error=%e, "config_snapshot deserialize failed");
                continue;
            }
        };
        out.push(SessionForPriming {
            id,
            role_title,
            jd_text,
            resume_text,
            include_intro: snapshot.include_intro,
            config_snapshot: snapshot,
            account_id,
        });
    }
    Ok(out)
}

/// Insert primed questions and flip state to `primed` atomically. The state
/// guard `WHERE state = 'pending'` makes it safe to call twice — only the
/// first caller wins.
#[allow(clippy::too_many_arguments)]
pub async fn complete_priming(
    pool: &PgPool,
    session_id: Uuid,
    account_id: Uuid,
    include_intro: bool,
    tech_questions: &[String],
    behavioral_picks: &[BehavioralPick],
    scoring_context: &str,
    generated_by: &str,
) -> Result<(), DbError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;

    let mut ordinal: i16 = 0;
    if include_intro {
        ordinal += 1;
        sqlx::query(
            r#"
            INSERT INTO questions (session_id, ordinal, kind, prompt_text, generated_by)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(session_id)
        .bind(ordinal)
        .bind(QuestionKind::Intro.as_str())
        .bind(INTRO_QUESTION_TEXT)
        .bind("fixed")
        .execute(&mut *tx)
        .await?;
    }

    for q in tech_questions {
        ordinal += 1;
        sqlx::query(
            r#"
            INSERT INTO questions (session_id, ordinal, kind, prompt_text, generated_by)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(session_id)
        .bind(ordinal)
        .bind(QuestionKind::Technical.as_str())
        .bind(q)
        .bind(generated_by)
        .execute(&mut *tx)
        .await?;
    }

    for pick in behavioral_picks {
        ordinal += 1;
        sqlx::query(
            r#"
            INSERT INTO questions (session_id, ordinal, kind, topic, prompt_text, generated_by)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(session_id)
        .bind(ordinal)
        .bind(QuestionKind::Behavioral.as_str())
        .bind(pick.topic.as_deref())
        .bind(&pick.question)
        .bind("bank")
        .execute(&mut *tx)
        .await?;
    }

    let updated: Option<(Uuid,)> = sqlx::query_as(
        r#"
        UPDATE interview_sessions
        SET state = 'primed',
            scoring_context = $1
        WHERE id = $2
          AND state = 'pending'
        RETURNING id
        "#,
    )
    .bind(scoring_context)
    .bind(session_id)
    .fetch_optional(&mut *tx)
    .await?;

    if updated.is_none() {
        // Lost the race or session was aborted. Roll back so we don't leave
        // orphan question rows.
        tx.rollback().await?;
        return Err(DbError::Conflict);
    }

    sqlx::query(
        r#"
        INSERT INTO audit_log (actor_id, session_id, action, metadata)
        VALUES ($1, $2, 'session.primed', $3)
        "#,
    )
    .bind(account_id)
    .bind(session_id)
    .bind(serde_json::json!({
        "tech_count": tech_questions.len(),
        "behavioral_count": behavioral_picks.len(),
    }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn mark_aborted(pool: &PgPool, session_id: Uuid, reason: &str) -> Result<(), DbError> {
    sqlx::query(
        r#"
        UPDATE interview_sessions
        SET state = 'aborted'
        WHERE id = $1
          AND state NOT IN ('scored', 'aborted', 'expired')
        "#,
    )
    .bind(session_id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO audit_log (session_id, action, metadata)
        VALUES ($1, 'session.aborted', $2)
        "#,
    )
    .bind(session_id)
    .bind(serde_json::json!({ "reason": reason }))
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_by_shortcode(pool: &PgPool, code: &str) -> Result<SessionByCode, DbError> {
    let row: Option<(
        Uuid,
        String,
        String,
        String,
        DateTime<Utc>,
        String,
        serde_json::Value,
    )> = sqlx::query_as(
        r#"
        SELECT s.id, s.state, s.candidate_name, s.role_title, s.expires_at, s.shortcode,
               s.config_snapshot
        FROM interview_sessions s
        JOIN shortlinks l ON l.session_id = s.id
        WHERE l.code = $1
        "#,
    )
    .bind(code)
    .fetch_optional(pool)
    .await?;

    let row = row.ok_or(DbError::NotFound)?;
    Ok(SessionByCode {
        id: row.0,
        state: row.1,
        candidate_name: row.2,
        role_title: row.3,
        expires_at: row.4,
        shortcode: row.5,
        answer_time_limit_ms: snapshot_answer_time_limit_ms(&row.6),
    })
}

fn snapshot_answer_time_limit_ms(snapshot: &serde_json::Value) -> u32 {
    snapshot
        .get("answer_time_limit_ms")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(180_000)
}

/// Atomically claim a shortlink on the first candidate join. The link is
/// one-use: the first caller flips `consumed_at` and receives the session;
/// every later caller gets `Conflict`. `SELECT ... FOR UPDATE` serializes
/// concurrent callers, so two clients can never both consume the same code.
///
/// Returns `NotFound` when the code is missing or the session has expired (an
/// expired link is left un-consumed). Returns `Conflict` when the link was
/// already consumed or the session is no longer in a joinable state — in both
/// cases the link is left untouched.
type SessionByCodeRow = (
    Uuid,
    String,
    String,
    String,
    DateTime<Utc>,
    String,
    serde_json::Value,
    Option<DateTime<Utc>>,
);

pub async fn consume_shortlink(pool: &PgPool, code: &str) -> Result<SessionByCode, DbError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;

    let row: Option<SessionByCodeRow> = sqlx::query_as(
        r#"
            SELECT s.id, s.state, s.candidate_name, s.role_title, s.expires_at,
                   s.shortcode, s.config_snapshot, l.consumed_at
            FROM shortlinks l
            JOIN interview_sessions s ON s.id = l.session_id
            WHERE l.code = $1
            FOR UPDATE OF l
            "#,
    )
    .bind(code)
    .fetch_optional(&mut *tx)
    .await?;

    let row = match row {
        Some(r) => r,
        None => {
            tx.rollback().await?;
            return Err(DbError::NotFound);
        }
    };

    let session = SessionByCode {
        id: row.0,
        state: row.1,
        candidate_name: row.2,
        role_title: row.3,
        expires_at: row.4,
        shortcode: row.5,
        answer_time_limit_ms: snapshot_answer_time_limit_ms(&row.6),
    };
    let consumed_at = row.7;

    if session.expires_at < Utc::now() {
        tx.rollback().await?;
        return Err(DbError::NotFound);
    }
    if consumed_at.is_some() {
        tx.rollback().await?;
        return Err(DbError::Conflict);
    }
    match session.state.as_str() {
        "pending" | "primed" | "active" | "paused" => {}
        _ => {
            tx.rollback().await?;
            return Err(DbError::Conflict);
        }
    }

    sqlx::query(
        r#"
        UPDATE shortlinks SET consumed_at = now() WHERE code = $1
        "#,
    )
    .bind(code)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO audit_log (session_id, action, metadata)
        VALUES ($1, 'shortlink.consumed', $2)
        "#,
    )
    .bind(session.id)
    .bind(serde_json::json!({ "code": code }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(session)
}

#[derive(Debug, Clone, Serialize)]
pub struct CandidatePayload {
    pub state: String,
    pub name: String,
    pub role: String,
    pub tech_questions: Vec<String>,
    pub non_tech_questions: Vec<String>,
    pub scoring_context: String,
    pub expires_at: DateTime<Utc>,
}

type CandidatePayloadRow = (Uuid, String, String, String, DateTime<Utc>, Option<String>);

pub async fn candidate_payload(pool: &PgPool, code: &str) -> Result<CandidatePayload, DbError> {
    let row: Option<CandidatePayloadRow> = sqlx::query_as(
        r#"
        SELECT s.id, s.state, s.candidate_name, s.role_title, s.expires_at, s.scoring_context
        FROM interview_sessions s
        JOIN shortlinks l ON l.session_id = s.id
        WHERE l.code = $1
        "#,
    )
    .bind(code)
    .fetch_optional(pool)
    .await?;
    let row = row.ok_or(DbError::NotFound)?;
    let (session_id, state, name, role, expires_at, scoring_context) = row;

    let question_rows: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT kind, prompt_text
        FROM questions
        WHERE session_id = $1
          AND kind IN ('technical', 'behavioral')
        ORDER BY ordinal
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    let mut tech_questions = Vec::new();
    let mut non_tech_questions = Vec::new();
    for (kind, text) in question_rows {
        match kind.as_str() {
            "technical" => tech_questions.push(text),
            "behavioral" => non_tech_questions.push(text),
            _ => {}
        }
    }

    Ok(CandidatePayload {
        state,
        name,
        role,
        tech_questions,
        non_tech_questions,
        scoring_context: scoring_context.unwrap_or_default(),
        expires_at,
    })
}

#[derive(Debug, Clone)]
pub struct SessionForScoring {
    pub id: Uuid,
    pub state: String,
    pub candidate_name: String,
    pub role_title: String,
    pub jd_text: String,
    pub resume_text: String,
    pub pass_threshold: i16,
    pub scoring_context: Option<String>,
    pub hr_email: String,
}

type SessionForScoringRow = (
    Uuid,
    String,
    String,
    String,
    String,
    String,
    i16,
    Option<String>,
    String,
);

pub async fn find_for_scoring(pool: &PgPool, id: Uuid) -> Result<SessionForScoring, DbError> {
    let row: Option<SessionForScoringRow> = sqlx::query_as(
        r#"
            SELECT id, state, candidate_name, role_title, jd_text, resume_text,
                   pass_threshold, scoring_context, hr_email
            FROM interview_sessions
            WHERE id = $1
            "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    let row = row.ok_or(DbError::NotFound)?;
    Ok(SessionForScoring {
        id: row.0,
        state: row.1,
        candidate_name: row.2,
        role_title: row.3,
        jd_text: row.4,
        resume_text: row.5,
        pass_threshold: row.6,
        scoring_context: row.7,
        hr_email: row.8,
    })
}

#[derive(Debug, Clone)]
pub struct StoredReport {
    pub id: Uuid,
    pub session_id: Uuid,
    pub overall: i16,
    pub technical: i16,
    pub behavioral: i16,
    pub passed: bool,
    pub summary: String,
    pub strengths: serde_json::Value,
    pub weaknesses: serde_json::Value,
    pub action_items: serde_json::Value,
    pub raw_ai_output: serde_json::Value,
    pub model: String,
    pub prompt_version: String,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewReport {
    pub session_id: Uuid,
    pub overall: i16,
    pub technical: i16,
    pub behavioral: i16,
    pub passed: bool,
    pub summary: String,
    pub strengths: serde_json::Value,
    pub weaknesses: serde_json::Value,
    pub action_items: serde_json::Value,
    pub raw_ai_output: serde_json::Value,
    pub model: String,
    pub prompt_version: String,
}

/// Insert a report and flip the session to `scored`. Idempotent via the
/// `UNIQUE (session_id)` constraint on `reports` — a second call returns
/// the previously stored row.
pub async fn upsert_report(pool: &PgPool, req: NewReport) -> Result<StoredReport, DbError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;

    let row: Option<(Uuid, DateTime<Utc>)> = sqlx::query_as(
        r#"
        INSERT INTO reports (
            session_id, overall, technical, behavioral, passed, summary,
            strengths, weaknesses, action_items, raw_ai_output, model, prompt_version
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
        ON CONFLICT (session_id) DO UPDATE
          SET overall = EXCLUDED.overall,
              technical = EXCLUDED.technical,
              behavioral = EXCLUDED.behavioral,
              passed = EXCLUDED.passed,
              summary = EXCLUDED.summary,
              strengths = EXCLUDED.strengths,
              weaknesses = EXCLUDED.weaknesses,
              action_items = EXCLUDED.action_items,
              raw_ai_output = EXCLUDED.raw_ai_output,
              model = EXCLUDED.model,
              prompt_version = EXCLUDED.prompt_version,
              generated_at = now()
        RETURNING id, generated_at
        "#,
    )
    .bind(req.session_id)
    .bind(req.overall)
    .bind(req.technical)
    .bind(req.behavioral)
    .bind(req.passed)
    .bind(&req.summary)
    .bind(&req.strengths)
    .bind(&req.weaknesses)
    .bind(&req.action_items)
    .bind(&req.raw_ai_output)
    .bind(&req.model)
    .bind(&req.prompt_version)
    .fetch_optional(&mut *tx)
    .await?;

    let (id, generated_at) = row.ok_or(DbError::NotFound)?;

    sqlx::query(
        r#"
        UPDATE interview_sessions
        SET state = 'scored',
            completed_at = COALESCE(completed_at, now())
        WHERE id = $1
          AND state NOT IN ('aborted', 'expired')
        "#,
    )
    .bind(req.session_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO audit_log (session_id, action, metadata)
        VALUES ($1, 'report.created', $2)
        "#,
    )
    .bind(req.session_id)
    .bind(serde_json::json!({
        "overall": req.overall,
        "passed": req.passed,
        "model": &req.model,
    }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(StoredReport {
        id,
        session_id: req.session_id,
        overall: req.overall,
        technical: req.technical,
        behavioral: req.behavioral,
        passed: req.passed,
        summary: req.summary,
        strengths: req.strengths,
        weaknesses: req.weaknesses,
        action_items: req.action_items,
        raw_ai_output: req.raw_ai_output,
        model: req.model,
        prompt_version: req.prompt_version,
        generated_at,
    })
}

pub async fn increment_shortlink_hit(pool: &PgPool, code: &str) -> Result<(), DbError> {
    sqlx::query(
        r#"
        UPDATE shortlinks SET hit_count = hit_count + 1 WHERE code = $1
        "#,
    )
    .bind(code)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct IncompatOutcome {
    pub incompat_count: i32,
    pub limit: i32,
    pub expired: bool,
}

/// Record a failed pre-interview system check ("system incompatibility") against
/// a shortlink. Increments `shortlinks.incompat_count`; once the count reaches
/// `limit` the interview session is moved to `expired` so the candidate is sent
/// to HR for a fresh link. The shortlink itself is left un-consumed, so a
/// candidate who later fixes their setup can still join while the count is below
/// the limit. Idempotent for an already-expired session — it reports
/// `expired: true` without incrementing further.
pub async fn record_incompatibility(
    pool: &PgPool,
    code: &str,
    limit: i32,
    check: &str,
    detail: &str,
) -> Result<IncompatOutcome, DbError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;

    let row: Option<(Uuid, String, DateTime<Utc>, i32)> = sqlx::query_as(
        r#"
        SELECT s.id, s.state, s.expires_at, l.incompat_count
        FROM shortlinks l
        JOIN interview_sessions s ON s.id = l.session_id
        WHERE l.code = $1
        FOR UPDATE OF l
        "#,
    )
    .bind(code)
    .fetch_optional(&mut *tx)
    .await?;

    let (session_id, state, expires_at, incompat_count) = match row {
        Some(r) => r,
        None => {
            tx.rollback().await?;
            return Err(DbError::NotFound);
        }
    };

    // Already dead — report it as expired without counting further.
    if state == "expired" || expires_at < Utc::now() {
        tx.rollback().await?;
        return Ok(IncompatOutcome {
            incompat_count,
            limit,
            expired: true,
        });
    }

    let new_count = incompat_count + 1;
    sqlx::query(r#"UPDATE shortlinks SET incompat_count = $1 WHERE code = $2"#)
        .bind(new_count)
        .bind(code)
        .execute(&mut *tx)
        .await?;

    // Only a not-yet-started session can be expired by this path; an interview
    // already underway is left alone.
    let expired = new_count >= limit && matches!(state.as_str(), "pending" | "primed");
    if expired {
        sqlx::query(
            r#"
            UPDATE interview_sessions
            SET state = 'expired'
            WHERE id = $1
              AND state IN ('pending', 'primed')
            "#,
        )
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO audit_log (session_id, action, metadata)
        VALUES ($1, 'shortlink.incompatible', $2)
        "#,
    )
    .bind(session_id)
    .bind(serde_json::json!({
        "code": code,
        "check": check,
        "detail": detail,
        "count": new_count,
        "limit": limit,
    }))
    .execute(&mut *tx)
    .await?;

    if expired {
        sqlx::query(
            r#"
            INSERT INTO audit_log (session_id, action, metadata)
            VALUES ($1, 'session.expired', $2)
            "#,
        )
        .bind(session_id)
        .bind(serde_json::json!({
            "reason": "system_incompatible",
            "count": new_count,
        }))
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(IncompatOutcome {
        incompat_count: new_count,
        limit,
        expired,
    })
}
