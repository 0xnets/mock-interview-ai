//! Single-use join nonces. The candidate (or HR-as-candidate) hits
//! `GET /v1/sessions/by-code/{code}` to get one; the realtime hub validates
//! and consumes it on WS upgrade.
//!
//! Phase 6: when `FEATURE_REDIS_NONCES=true` and Redis is configured, the
//! store moves to Redis (SET NX EX) so realtime pods share the namespace.
//! Otherwise an in-memory `Mutex<HashMap>` keeps single-pod behavior.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;
use redis::AsyncCommands;
use uuid::Uuid;

const NONCE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Copy)]
pub struct NonceInfo {
    pub session_id: Uuid,
    pub created_at: Instant,
}

#[allow(clippy::large_enum_variant)]
pub enum NonceStore {
    InMemory(Mutex<HashMap<String, NonceInfo>>),
    Redis(redis::aio::ConnectionManager),
}

impl std::fmt::Debug for NonceStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NonceStore::InMemory(_) => write!(f, "NonceStore::InMemory"),
            NonceStore::Redis(_) => write!(f, "NonceStore::Redis"),
        }
    }
}

impl NonceStore {
    pub fn new_in_memory() -> Self {
        NonceStore::InMemory(Mutex::new(HashMap::new()))
    }

    pub fn new_redis(mgr: redis::aio::ConnectionManager) -> Self {
        NonceStore::Redis(mgr)
    }

    /// Mint a new nonce for `session_id`.
    pub async fn mint(&self, session_id: Uuid) -> Result<String> {
        let token = Uuid::new_v4().simple().to_string();
        match self {
            NonceStore::InMemory(m) => {
                let mut guard = m.lock().expect("nonce mutex poisoned");
                Self::gc_locked(&mut guard);
                guard.insert(
                    token.clone(),
                    NonceInfo {
                        session_id,
                        created_at: Instant::now(),
                    },
                );
            }
            NonceStore::Redis(mgr) => {
                let mut conn = mgr.clone();
                let _: () = conn
                    .set_ex(
                        format!("nonce:{token}"),
                        session_id.to_string(),
                        NONCE_TTL.as_secs(),
                    )
                    .await?;
            }
        }
        Ok(token)
    }

    /// Consume `token`. Returns the bound session id if the nonce was valid
    /// and not yet expired; removes it from the store either way.
    pub async fn consume(&self, token: &str) -> Option<Uuid> {
        match self {
            NonceStore::InMemory(m) => {
                let mut guard = m.lock().expect("nonce mutex poisoned");
                let entry = guard.remove(token)?;
                if entry.created_at.elapsed() > NONCE_TTL {
                    return None;
                }
                Some(entry.session_id)
            }
            NonceStore::Redis(mgr) => {
                let mut conn = mgr.clone();
                let key = format!("nonce:{token}");
                let raw: Option<String> = redis::cmd("GETDEL")
                    .arg(&key)
                    .query_async(&mut conn)
                    .await
                    .ok()?;
                raw.and_then(|s| Uuid::parse_str(&s).ok())
            }
        }
    }

    fn gc_locked(guard: &mut HashMap<String, NonceInfo>) {
        guard.retain(|_, v| v.created_at.elapsed() <= NONCE_TTL);
    }
}
