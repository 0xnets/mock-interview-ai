//! Phase 6: shared Redis connection manager + helpers.
//!
//! One ConnectionManager is shared by the rate limiter, the nonce store, the
//! WS Redlock, and the Streams relays. When `REDIS_URL` is unset we return
//! `None` so the in-memory fallbacks stay in charge — every consumer is built
//! to degrade if Redis is missing.

use anyhow::{Context, Result};
use redis::aio::ConnectionManager;

#[derive(Clone)]
pub struct RedisBackplane {
    pub mgr: ConnectionManager,
}

impl RedisBackplane {
    pub async fn connect(url: &str) -> Result<Self> {
        let client = redis::Client::open(url).context("invalid REDIS_URL")?;
        let mgr = client
            .get_connection_manager()
            .await
            .context("redis ConnectionManager")?;
        Ok(Self { mgr })
    }

    /// Lightweight reachability check for /readyz.
    pub async fn ping(&self) -> Result<()> {
        let mut conn = self.mgr.clone();
        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .context("redis ping")?;
        Ok(())
    }
}
