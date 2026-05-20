//! Shared application state cloned into every request handler.

use std::sync::Arc;

use persistence::Pools;
use tokio::sync::Notify;

use crate::{
    auth::JwtKeys,
    config::Settings,
    infrastructure::{redis_backplane::RedisBackplane, signing::TranscriptSigner},
    rate_limit::InMemoryWindow,
    realtime,
};

#[derive(Clone)]
pub struct AppState {
    pub pools: Pools,
    pub cfg: Settings,
    pub provider: Arc<dyn ai::AiProvider>,
    pub prime_notify: Arc<Notify>,
    pub nonces: Arc<realtime::NonceStore>,
    pub signer: Arc<TranscriptSigner>,
    pub redis: Option<Arc<RedisBackplane>>,
    pub jwt: Option<Arc<JwtKeys>>,
    pub rate_limit_mem: Arc<InMemoryWindow>,
}
