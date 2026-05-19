//! Phase 6: per-session WS ownership lock (a single-instance Redlock).
//!
//! When `FEATURE_REDIS_WS_LOCKS=true`, every WS upgrade acquires
//! `ws:session:{id}` in Redis with `SET NX EX`. A background task renews the
//! TTL while the connection is alive; on disconnect the lock is released so
//! a reconnect can land on any pod.
//!
//! The "single-instance" Redlock is sufficient because all pods talk to the
//! same Redis backplane (RFC requires N>=3 only for cross-region availability).
//! When Redis is missing the helper degrades to a no-op so single-pod dev
//! keeps working.

use std::time::Duration;

use anyhow::Result;
use redis::AsyncCommands;
use uuid::Uuid;

use crate::redis_backplane::RedisBackplane;

const HOLD_SECS: u64 = 30;
const RENEW_EVERY: Duration = Duration::from_secs(10);

pub struct SessionLock {
    key: String,
    token: String,
    mgr: redis::aio::ConnectionManager,
    cancel: tokio::sync::oneshot::Sender<()>,
}

impl SessionLock {
    /// Try to acquire `ws:session:{session_id}`. Returns None if another pod
    /// holds it (caller should reject with 409).
    pub async fn acquire(
        backplane: &RedisBackplane,
        session_id: Uuid,
    ) -> Result<Option<Self>> {
        let key = format!("ws:session:{session_id}");
        let token = Uuid::new_v4().simple().to_string();
        let mut mgr = backplane.mgr.clone();
        let opts = redis::SetOptions::default()
            .with_expiration(redis::SetExpiry::EX(HOLD_SECS))
            .conditional_set(redis::ExistenceCheck::NX);
        let acquired: Option<String> = mgr.set_options(&key, &token, opts).await?;
        if acquired.is_none() {
            return Ok(None);
        }

        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let renew_key = key.clone();
        let renew_token = token.clone();
        let mut renew_mgr = mgr.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut rx => break,
                    _ = tokio::time::sleep(RENEW_EVERY) => {
                        // Renew only if we still hold it. Lua keeps the
                        // compare-and-set atomic.
                        let script = redis::Script::new(
                            r#"
                            if redis.call('GET', KEYS[1]) == ARGV[1] then
                                return redis.call('EXPIRE', KEYS[1], ARGV[2])
                            else
                                return 0
                            end
                            "#,
                        );
                        let res: redis::RedisResult<i32> = script
                            .key(&renew_key)
                            .arg(&renew_token)
                            .arg(HOLD_SECS as i64)
                            .invoke_async(&mut renew_mgr)
                            .await;
                        if !matches!(res, Ok(1)) {
                            tracing::warn!(key=%renew_key, "ws session lock lost during renew");
                            break;
                        }
                    }
                }
            }
        });

        Ok(Some(Self {
            key,
            token,
            mgr,
            cancel: tx,
        }))
    }

    pub async fn release(self) {
        let SessionLock { key, token, mut mgr, cancel } = self;
        let _ = cancel.send(());
        let script = redis::Script::new(
            r#"
            if redis.call('GET', KEYS[1]) == ARGV[1] then
                return redis.call('DEL', KEYS[1])
            else
                return 0
            end
            "#,
        );
        let _: redis::RedisResult<i32> = script
            .key(key)
            .arg(token)
            .invoke_async(&mut mgr)
            .await;
    }
}
