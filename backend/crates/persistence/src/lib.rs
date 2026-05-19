pub mod migrate;
pub mod repo_audit;
pub mod repo_auth;
pub mod repo_keys;
pub mod repo_outbox;
pub mod repo_rate_limit;
pub mod repo_realtime;
pub mod repo_reports;
pub mod repo_session;
pub mod repo_transcript;

pub use sqlx::PgPool;

use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("not found")]
    NotFound,
    #[error("conflict")]
    Conflict,
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

pub async fn connect(url: &str, max_connections: u32) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect(url)
        .await
}

/// Phase 6: primary + optional read-replica pool. Repos that don't mutate
/// state (`list_*`, `find_*`, `candidate_payload`, …) route reads to
/// `read.clone()`; everything else uses the primary.
#[derive(Debug, Clone)]
pub struct Pools {
    pub primary: PgPool,
    pub read: PgPool,
}

impl Pools {
    pub fn from_primary(pool: PgPool) -> Self {
        Self {
            primary: pool.clone(),
            read: pool,
        }
    }

    pub async fn connect(
        primary_url: &str,
        replica_url: Option<&str>,
        max_connections: u32,
    ) -> Result<Self, sqlx::Error> {
        let primary = connect(primary_url, max_connections).await?;
        let read = match replica_url {
            Some(url) if !url.is_empty() && url != primary_url => {
                connect(url, max_connections).await?
            }
            _ => primary.clone(),
        };
        Ok(Self { primary, read })
    }
}
