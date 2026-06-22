//! Liveness / readiness probes.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use super::AppState;

pub async fn healthz() -> impl IntoResponse {
    Json(json!({ "ok": true }))
}

pub async fn livez() -> impl IntoResponse {
    // Process is up. /readyz is the one that checks dependencies.
    Json(json!({ "ok": true }))
}

pub async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    let db_ok = sqlx::query("SELECT 1")
        .execute(&state.pools.primary)
        .await
        .is_ok();

    let redis_ok = match state.redis.as_ref() {
        Some(r) => r.ping().await.is_ok(),
        None => true,
    };

    let ok = db_ok && redis_ok;
    let status = if ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(json!({
            "ok": ok,
            "db": db_ok,
            "redis": redis_ok,
        })),
    )
}
