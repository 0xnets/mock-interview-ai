//! Axum WebSocket upgrade for `/v1/ws/interview?token=<nonce>`. Validates and
//! consumes the join nonce, then hands the socket to the per-session actor.

use std::collections::HashMap;

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Query, State};
use axum::response::Response;

use crate::app::AppState;
use crate::error::ApiError;
use crate::realtime::actor::{self, ActorDeps};
use crate::realtime::locks::SessionLock;

pub async fn ws_handler(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let token = params.get("token").map(|s| s.as_str()).unwrap_or("");
    if token.is_empty() {
        return Err(ApiError::Unauthorized);
    }

    let session_id = state
        .nonces
        .consume(token)
        .await
        .ok_or(ApiError::Unauthorized)?;

    // Phase 6 HA: try to take exclusive ownership across pods.
    let session_lock = if state.cfg.feature_redis_ws_locks {
        match state.redis.as_ref() {
            Some(r) => match SessionLock::acquire(r, session_id).await {
                Ok(Some(l)) => Some(l),
                Ok(None) => return Err(ApiError::Conflict),
                Err(e) => {
                    tracing::warn!(error=%e, "ws lock acquire failed; falling back to single-pod");
                    None
                }
            },
            None => None,
        }
    } else {
        None
    };

    let deps = ActorDeps {
        pool: state.pools.primary.clone(),
        provider: state.provider.clone(),
        cfg: state.cfg.clone(),
        signer: state.signer.clone(),
    };

    crate::infrastructure::metrics::WS_SESSIONS_ACTIVE.inc();
    Ok(ws.on_upgrade(move |socket| async move {
        actor::run_session(socket, session_id, deps).await;
        if let Some(lock) = session_lock {
            lock.release().await;
        }
        crate::infrastructure::metrics::WS_SESSIONS_ACTIVE.dec();
    }))
}
