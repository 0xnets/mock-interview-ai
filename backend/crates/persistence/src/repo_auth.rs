//! Phase 6: auth/identity persistence — accounts, refresh tokens, invites.
//!
//! Password hashes are written/verified in the api crate (argon2 lives there);
//! this module only reads/writes the stored hash bytes and metadata. Refresh
//! tokens are stored as a SHA-256 of the raw token so a leaked DB row cannot
//! be replayed against /v1/auth/refresh.

use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::DbError;

#[derive(Debug, Clone)]
pub struct Account {
    pub id: Uuid,
    pub email: String,
    pub password_hash: Option<String>,
    pub display_name: Option<String>,
    pub role: String,
    pub status: String,
    pub org_id: Option<Uuid>,
    pub last_login_at: Option<DateTime<Utc>>,
}

type AccountRow = (
    Uuid,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
    Option<Uuid>,
    Option<DateTime<Utc>>,
);

pub async fn find_account_by_email(pool: &PgPool, email: &str) -> Result<Account, DbError> {
    let row: Option<AccountRow> = sqlx::query_as(
        r#"
            SELECT id, email::text, password_hash, display_name, role, status, org_id, last_login_at
            FROM accounts
            WHERE email = $1::citext
            "#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;
    let row = row.ok_or(DbError::NotFound)?;
    Ok(Account {
        id: row.0,
        email: row.1,
        password_hash: row.2,
        display_name: row.3,
        role: row.4,
        status: row.5,
        org_id: row.6,
        last_login_at: row.7,
    })
}

pub async fn find_account_by_id(pool: &PgPool, id: Uuid) -> Result<Account, DbError> {
    let row: Option<AccountRow> = sqlx::query_as(
        r#"
            SELECT id, email::text, password_hash, display_name, role, status, org_id, last_login_at
            FROM accounts
            WHERE id = $1
            "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    let row = row.ok_or(DbError::NotFound)?;
    Ok(Account {
        id: row.0,
        email: row.1,
        password_hash: row.2,
        display_name: row.3,
        role: row.4,
        status: row.5,
        org_id: row.6,
        last_login_at: row.7,
    })
}

pub async fn touch_last_login(pool: &PgPool, account_id: Uuid) -> Result<(), DbError> {
    sqlx::query("UPDATE accounts SET last_login_at = now() WHERE id = $1")
        .bind(account_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_password_hash(
    pool: &PgPool,
    account_id: Uuid,
    new_hash: &str,
) -> Result<(), DbError> {
    sqlx::query("UPDATE accounts SET password_hash = $2, status = 'active' WHERE id = $1")
        .bind(account_id)
        .bind(new_hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn disable_account(pool: &PgPool, account_id: Uuid) -> Result<(), DbError> {
    let result = sqlx::query("UPDATE accounts SET status = 'disabled' WHERE id = $1")
        .bind(account_id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(DbError::NotFound);
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct NewAccount {
    pub email: String,
    pub display_name: Option<String>,
    pub role: String,
    pub invited_by: Uuid,
}

pub async fn create_invited_account(pool: &PgPool, req: NewAccount) -> Result<Uuid, DbError> {
    let id = Uuid::new_v4();
    let inserted: Option<(Uuid,)> = sqlx::query_as(
        r#"
        INSERT INTO accounts (id, email, display_name, role, status, invited_by)
        VALUES ($1, $2::citext, $3, $4, 'invited', $5)
        ON CONFLICT (email) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(id)
    .bind(&req.email)
    .bind(req.display_name.as_deref())
    .bind(&req.role)
    .bind(req.invited_by)
    .fetch_optional(pool)
    .await?;
    Ok(inserted.ok_or(DbError::Conflict)?.0)
}

// ─── refresh tokens ─────────────────────────────────────────────────────────

pub fn hash_token(raw: &str) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(raw.as_bytes());
    h.finalize().to_vec()
}

pub async fn insert_refresh_token(
    pool: &PgPool,
    raw_token: &str,
    account_id: Uuid,
    ttl: Duration,
    parent_raw: Option<&str>,
    user_agent: Option<&str>,
    ip: Option<&str>,
) -> Result<(), DbError> {
    let hash = hash_token(raw_token);
    let parent_hash = parent_raw.map(hash_token);
    sqlx::query(
        r#"
        INSERT INTO refresh_tokens
            (token_hash, account_id, expires_at, parent_hash, user_agent, ip_inet)
        VALUES ($1, $2, $3, $4, $5, $6::inet)
        "#,
    )
    .bind(&hash)
    .bind(account_id)
    .bind(Utc::now() + ttl)
    .bind(parent_hash)
    .bind(user_agent)
    .bind(ip)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct RefreshTokenRow {
    pub account_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

pub async fn lookup_refresh_token(
    pool: &PgPool,
    raw_token: &str,
) -> Result<RefreshTokenRow, DbError> {
    let hash = hash_token(raw_token);
    let row: Option<(Uuid, DateTime<Utc>, Option<DateTime<Utc>>)> = sqlx::query_as(
        r#"
        SELECT account_id, expires_at, revoked_at
        FROM refresh_tokens
        WHERE token_hash = $1
        "#,
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await?;
    let row = row.ok_or(DbError::NotFound)?;
    Ok(RefreshTokenRow {
        account_id: row.0,
        expires_at: row.1,
        revoked_at: row.2,
    })
}

/// Atomically marks the token revoked and touches last_used_at. Returns the
/// account id only if the row was active. A revoked/expired token returns
/// `Err(DbError::NotFound)` — callers should treat that as a reuse attempt
/// and revoke the whole family (see `revoke_family`).
pub async fn rotate_refresh_token(pool: &PgPool, raw_token: &str) -> Result<Uuid, DbError> {
    let hash = hash_token(raw_token);
    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"
        UPDATE refresh_tokens
        SET revoked_at = now(),
            last_used_at = now()
        WHERE token_hash = $1
          AND revoked_at IS NULL
          AND expires_at > now()
        RETURNING account_id
        "#,
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await?;
    Ok(row.ok_or(DbError::NotFound)?.0)
}

pub async fn revoke_refresh_token(pool: &PgPool, raw_token: &str) -> Result<(), DbError> {
    let hash = hash_token(raw_token);
    sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = now() WHERE token_hash = $1 AND revoked_at IS NULL",
    )
    .bind(&hash)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn revoke_all_for_account(pool: &PgPool, account_id: Uuid) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = now() WHERE account_id = $1 AND revoked_at IS NULL",
    )
    .bind(account_id)
    .execute(pool)
    .await?;
    Ok(())
}

// ─── invites ────────────────────────────────────────────────────────────────

pub async fn create_invite(
    pool: &PgPool,
    raw_token: &str,
    account_id: Uuid,
    invited_by: Uuid,
    ttl: Duration,
) -> Result<(), DbError> {
    let hash = hash_token(raw_token);
    sqlx::query(
        r#"
        INSERT INTO account_invites (token_hash, account_id, invited_by, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(&hash)
    .bind(account_id)
    .bind(invited_by)
    .bind(Utc::now() + ttl)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct InviteRow {
    pub account_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
}

pub async fn consume_invite(pool: &PgPool, raw_token: &str) -> Result<InviteRow, DbError> {
    let hash = hash_token(raw_token);
    let row: Option<(Uuid, DateTime<Utc>, Option<DateTime<Utc>>)> = sqlx::query_as(
        r#"
        UPDATE account_invites
        SET consumed_at = now()
        WHERE token_hash = $1
          AND consumed_at IS NULL
          AND revoked_at IS NULL
          AND expires_at > now()
          AND EXISTS (
              SELECT 1
              FROM accounts a
              WHERE a.id = account_invites.account_id
                AND a.status <> 'disabled'
          )
        RETURNING account_id, expires_at, consumed_at
        "#,
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await?;
    let row = row.ok_or(DbError::NotFound)?;
    Ok(InviteRow {
        account_id: row.0,
        expires_at: row.1,
        consumed_at: row.2,
    })
}

#[derive(Debug, Clone)]
pub struct InviteDetail {
    pub id: Uuid,
    pub account_id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    pub role: String,
    pub account_status: String,
    pub invited_by: Uuid,
    pub invited_by_email: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub status: String,
}

type InviteDetailRow = (
    Uuid,
    Uuid,
    String,
    Option<String>,
    String,
    String,
    Uuid,
    Option<String>,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    DateTime<Utc>,
    String,
);

fn invite_detail_from_row(row: InviteDetailRow) -> InviteDetail {
    InviteDetail {
        id: row.0,
        account_id: row.1,
        email: row.2,
        display_name: row.3,
        role: row.4,
        account_status: row.5,
        invited_by: row.6,
        invited_by_email: row.7,
        expires_at: row.8,
        consumed_at: row.9,
        revoked_at: row.10,
        created_at: row.11,
        status: row.12,
    }
}

pub async fn list_invites(
    pool: &PgPool,
    status: Option<&str>,
) -> Result<Vec<InviteDetail>, DbError> {
    let rows: Vec<InviteDetailRow> = sqlx::query_as(
        r#"
        SELECT
            i.id,
            a.id AS account_id,
            a.email::text,
            a.display_name,
            a.role,
            a.status,
            i.invited_by,
            inviter.email::text AS invited_by_email,
            i.expires_at,
            i.consumed_at,
            i.revoked_at,
            i.created_at,
            CASE
                WHEN i.consumed_at IS NOT NULL THEN 'accepted'
                WHEN i.revoked_at IS NOT NULL THEN 'revoked'
                WHEN i.expires_at <= now() THEN 'expired'
                ELSE 'active'
            END AS invite_status
        FROM account_invites i
        JOIN accounts a ON a.id = i.account_id
        LEFT JOIN accounts inviter ON inviter.id = i.invited_by
        WHERE $1::text IS NULL
           OR $1::text = 'all'
           OR $1::text = CASE
                WHEN i.consumed_at IS NOT NULL THEN 'accepted'
                WHEN i.revoked_at IS NOT NULL THEN 'revoked'
                WHEN i.expires_at <= now() THEN 'expired'
                ELSE 'active'
            END
        ORDER BY i.created_at DESC
        "#,
    )
    .bind(status)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(invite_detail_from_row).collect())
}

pub async fn find_invite(pool: &PgPool, invite_id: Uuid) -> Result<InviteDetail, DbError> {
    let row: Option<InviteDetailRow> = sqlx::query_as(
        r#"
        SELECT
            i.id,
            a.id AS account_id,
            a.email::text,
            a.display_name,
            a.role,
            a.status,
            i.invited_by,
            inviter.email::text AS invited_by_email,
            i.expires_at,
            i.consumed_at,
            i.revoked_at,
            i.created_at,
            CASE
                WHEN i.consumed_at IS NOT NULL THEN 'accepted'
                WHEN i.revoked_at IS NOT NULL THEN 'revoked'
                WHEN i.expires_at <= now() THEN 'expired'
                ELSE 'active'
            END AS invite_status
        FROM account_invites i
        JOIN accounts a ON a.id = i.account_id
        LEFT JOIN accounts inviter ON inviter.id = i.invited_by
        WHERE i.id = $1
        "#,
    )
    .bind(invite_id)
    .fetch_optional(pool)
    .await?;

    row.map(invite_detail_from_row).ok_or(DbError::NotFound)
}

pub async fn revoke_invite(
    pool: &PgPool,
    invite_id: Uuid,
    revoked_by: Uuid,
) -> Result<(), DbError> {
    let result = sqlx::query(
        r#"
        UPDATE account_invites
        SET revoked_at = now(), revoked_by = $2
        WHERE id = $1
          AND consumed_at IS NULL
          AND revoked_at IS NULL
          AND expires_at > now()
        "#,
    )
    .bind(invite_id)
    .bind(revoked_by)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(DbError::NotFound);
    }
    Ok(())
}

pub async fn undo_revoke_invite(pool: &PgPool, invite_id: Uuid) -> Result<(), DbError> {
    let result = sqlx::query(
        r#"
        UPDATE account_invites
        SET revoked_at = NULL, revoked_by = NULL
        WHERE id = $1
          AND consumed_at IS NULL
          AND revoked_at IS NOT NULL
          AND expires_at > now()
          AND EXISTS (
              SELECT 1
              FROM accounts a
              WHERE a.id = account_invites.account_id
                AND a.status <> 'disabled'
          )
        "#,
    )
    .bind(invite_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(DbError::NotFound);
    }
    Ok(())
}

pub async fn revoke_pending_invites_for_account(
    pool: &PgPool,
    account_id: Uuid,
    revoked_by: Uuid,
) -> Result<(), DbError> {
    sqlx::query(
        r#"
        UPDATE account_invites
        SET revoked_at = COALESCE(revoked_at, now()), revoked_by = COALESCE(revoked_by, $2)
        WHERE account_id = $1
          AND consumed_at IS NULL
          AND expires_at > now()
        "#,
    )
    .bind(account_id)
    .bind(revoked_by)
    .execute(pool)
    .await?;
    Ok(())
}
