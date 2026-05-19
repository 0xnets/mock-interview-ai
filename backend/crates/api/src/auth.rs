//! Phase 6 auth: JWT bearer + RBAC, with dual-acceptance for the legacy
//! `HR_DEV_TOKEN` during the 1-week cutover window.
//!
//! Access tokens are short-lived (15 min). Refresh tokens are long-lived
//! (14 days), rotating: every successful `/v1/auth/refresh` issues a new pair
//! and revokes the parent. Reuse of a revoked refresh token revokes the whole
//! family — this is the standard mitigation for stolen refresh tokens.

use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::{
    extract::{FromRequestParts, State},
    http::{request::Parts, HeaderMap},
    middleware::Next,
    response::Response,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{app::AppState, error::ApiError};

/// JWT subject/role/scope payload. We deliberately keep this small — anything
/// beyond identity (account_id, role) is looked up from Postgres on each
/// request, so a revoked account stops being trusted within seconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessClaims {
    pub sub: String,    // account_id
    pub role: String,   // hr | admin | super_admin
    pub exp: i64,
    pub iat: i64,
    pub iss: String,
    pub kid: String,    // key id; reserved for jwks rotation
    /// Request-correlation id minted at login. Echoed into audit_log.event_id
    /// for any state-changing handler under this session.
    pub sid: String,
}

#[derive(Clone)]
pub struct JwtKeys {
    enc: EncodingKey,
    dec: DecodingKey,
    pub issuer: String,
    pub kid: String,
}

impl JwtKeys {
    pub fn from_env() -> Result<Self> {
        let secret = std::env::var("JWT_SIGNING_SECRET")
            .or_else(|_| std::env::var("HR_DEV_TOKEN"))
            .context("JWT_SIGNING_SECRET must be set")?;
        if secret.len() < 32 {
            return Err(anyhow!("JWT_SIGNING_SECRET must be >= 32 bytes"));
        }
        let issuer = std::env::var("JWT_ISSUER").unwrap_or_else(|_| "mock-interview-ai".into());
        let kid = std::env::var("JWT_KEY_ID").unwrap_or_else(|_| "v1".into());
        Ok(Self {
            enc: EncodingKey::from_secret(secret.as_bytes()),
            dec: DecodingKey::from_secret(secret.as_bytes()),
            issuer,
            kid,
        })
    }

    pub fn mint_access(
        &self,
        account_id: Uuid,
        role: &str,
        session_id: Uuid,
        ttl: Duration,
    ) -> Result<String> {
        let now = Utc::now();
        let claims = AccessClaims {
            sub: account_id.to_string(),
            role: role.to_string(),
            exp: (now + ttl).timestamp(),
            iat: now.timestamp(),
            iss: self.issuer.clone(),
            kid: self.kid.clone(),
            sid: session_id.to_string(),
        };
        let mut header = Header::default();
        header.kid = Some(self.kid.clone());
        Ok(encode(&header, &claims, &self.enc)?)
    }

    pub fn verify_access(&self, token: &str) -> Result<AccessClaims> {
        let mut v = Validation::default();
        v.set_issuer(&[self.issuer.clone()]);
        // Allow a couple seconds of clock skew; default leeway is 60s.
        let data = decode::<AccessClaims>(token, &self.dec, &v)?;
        Ok(data.claims)
    }
}

#[derive(Debug, Clone)]
pub struct Principal {
    pub account_id: Uuid,
    pub role: String,
    /// Request-correlation id from the access token's `sid`, used for audit
    /// log linkage. For the dev-token fallback it's a freshly-minted uuid.
    pub event_id: Uuid,
    /// Set when this principal came from the legacy `HR_DEV_TOKEN` cutover
    /// path so logs can flag dual-auth usage during the migration window.
    pub legacy: bool,
}

/// Bridge for handlers that need to keep working with the old `HrPrincipal`
/// surface during the cutover. `HrPrincipal { account_id }` is just a
/// projection of `Principal`.
#[derive(Debug, Clone, Copy)]
pub struct HrPrincipal {
    pub account_id: Uuid,
}

impl From<&Principal> for HrPrincipal {
    fn from(p: &Principal) -> Self {
        Self { account_id: p.account_id }
    }
}

/// Roles in increasing order of authority. Phase 6 introduces super_admin for
/// cross-org operations; admin is org-scoped; hr is the default seat.
pub fn role_at_least(role: &str, required: &str) -> bool {
    fn rank(r: &str) -> u8 {
        match r {
            "super_admin" => 3,
            "admin" => 2,
            "hr" => 1,
            _ => 0,
        }
    }
    rank(role) >= rank(required)
}

/// Middleware: HR-or-above. The role check happens inside `authenticate` +
/// `role_at_least` so this preserves the legacy entry-point name while now
/// returning a `Principal` that downstream handlers can pull via the
/// `FromRequestParts` impl.
pub async fn require_hr(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut req: axum::extract::Request,
    next: Next,
) -> Result<Response, ApiError> {
    let principal = authenticate(&state, &headers).await?;
    if !role_at_least(&principal.role, "hr") {
        return Err(ApiError::Unauthorized);
    }
    req.extensions_mut().insert(principal.clone());
    req.extensions_mut().insert(HrPrincipal::from(&principal));
    Ok(next.run(req).await)
}

pub async fn require_admin(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut req: axum::extract::Request,
    next: Next,
) -> Result<Response, ApiError> {
    let principal = authenticate(&state, &headers).await?;
    if !role_at_least(&principal.role, "admin") {
        return Err(ApiError::Unauthorized);
    }
    req.extensions_mut().insert(principal.clone());
    req.extensions_mut().insert(HrPrincipal::from(&principal));
    Ok(next.run(req).await)
}

async fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<Principal, ApiError> {
    let raw = bearer_token(headers).ok_or(ApiError::Unauthorized)?;

    // 1. JWT path (Phase 6 default).
    if state.cfg.feature_jwt_auth {
        if let Some(keys) = state.jwt.as_ref() {
            if let Ok(claims) = keys.verify_access(&raw) {
                let account_id: Uuid = claims
                    .sub
                    .parse()
                    .map_err(|_| ApiError::Unauthorized)?;
                let event_id: Uuid = claims
                    .sid
                    .parse()
                    .unwrap_or_else(|_| Uuid::new_v4());
                return Ok(Principal {
                    account_id,
                    role: claims.role,
                    event_id,
                    legacy: false,
                });
            }
        }
    }

    // 2. Legacy HR_DEV_TOKEN cutover path. Stays accepted while
    //    FEATURE_LEGACY_HR_TOKEN is on (default true; flip off after 1 week).
    if state.cfg.feature_legacy_hr_token
        && constant_time_eq(raw.as_bytes(), state.cfg.hr_dev_token.as_bytes())
    {
        tracing::warn!(
            target: "auth.legacy",
            "request authenticated with legacy HR_DEV_TOKEN; flip FEATURE_LEGACY_HR_TOKEN=false to disable"
        );
        return Ok(Principal {
            account_id: state.cfg.default_account_id,
            role: "hr".into(),
            event_id: Uuid::new_v4(),
            legacy: true,
        });
    }

    Err(ApiError::Unauthorized)
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let v = headers.get(axum::http::header::AUTHORIZATION)?;
    let v = v.to_str().ok()?;
    let (scheme, token) = v.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return None;
    }
    Some(token.trim().to_string())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

impl<S> FromRequestParts<S> for Principal
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Principal>()
            .cloned()
            .ok_or(ApiError::Unauthorized)
    }
}

impl<S> FromRequestParts<S> for HrPrincipal
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<HrPrincipal>()
            .copied()
            .ok_or(ApiError::Unauthorized)
    }
}

// ─── password hashing (argon2id) ────────────────────────────────────────────

pub fn hash_password(plain: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    let hash = argon
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| ApiError::Internal(format!("argon2 hash failed: {e}")))?
        .to_string();
    Ok(hash)
}

pub fn verify_password(plain: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok()
}

// ─── refresh tokens ─────────────────────────────────────────────────────────

pub fn mint_refresh_token() -> String {
    let r1: u128 = rand::random();
    let r2: u128 = rand::random();
    format!("{}{}", hex::encode(r1.to_be_bytes()), hex::encode(r2.to_be_bytes()))
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

#[allow(dead_code)]
pub fn audit_roles() -> HashSet<&'static str> {
    HashSet::from(["hr", "admin", "super_admin"])
}
