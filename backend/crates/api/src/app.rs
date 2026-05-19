use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    http::{header, HeaderValue, Method, StatusCode},
    middleware,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use persistence::Pools;
use serde_json::json;
use tokio::sync::Notify;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{
    auth::{self, JwtKeys},
    config::Settings,
    handlers,
    kms::CheckpointSigner,
    rate_limit::InMemoryWindow,
    realtime,
    redis_backplane::RedisBackplane,
    signing::TranscriptSigner,
    storage::BlobStore,
};

#[derive(Clone)]
pub struct AppState {
    pub pools: Pools,
    pub cfg: Settings,
    pub provider: Arc<dyn ai::AiProvider>,
    pub prime_notify: Arc<Notify>,
    pub nonces: Arc<realtime::NonceStore>,
    pub signer: Arc<TranscriptSigner>,
    pub kms_signer: Arc<dyn CheckpointSigner>,
    pub blob_store: Option<Arc<BlobStore>>,
    pub redis: Option<Arc<RedisBackplane>>,
    pub jwt: Option<Arc<JwtKeys>>,
    pub rate_limit_mem: Arc<InMemoryWindow>,
}

pub fn router(state: AppState) -> Router {
    let cors = build_cors(&state.cfg);

    let public = Router::new()
        .route("/healthz", get(healthz))
        .route("/livez", get(livez))
        .route("/readyz", get(readyz))
        .route("/s/{code}", get(handlers::shortlink::redirect_to_session))
        .route(
            "/v1/sessions/by-code/{code}",
            get(handlers::sessions::get_by_code),
        )
        .route(
            "/v1/sessions/by-code/{code}/join",
            get(handlers::sessions::issue_join_nonce)
                .post(handlers::sessions::issue_join_nonce),
        )
        .route("/v1/ws/interview", get(realtime::ws_handler))
        .route(
            "/v1/interviews/{id}/finalize",
            post(handlers::interviews::finalize),
        )
        .route(
            "/v1/interviews/{id}/transcript",
            get(handlers::transcripts::get_transcript),
        )
        .route(
            "/v1/interviews/{id}/report.pdf",
            get(handlers::reports::get_report_pdf),
        )
        .route(
            "/v1/interviews/{id}/report.status",
            get(handlers::reports::get_report_status),
        );

    // Phase 6: auth surface. Login + refresh + invite are public-ish (they
    // accept anonymous traffic but are rate-limited); logout authenticates
    // via the refresh cookie.
    let auth_routes = Router::new()
        .route("/v1/auth/login", post(handlers::auth::login))
        .route("/v1/auth/refresh", post(handlers::auth::refresh))
        .route("/v1/auth/logout", post(handlers::auth::logout))
        .route("/v1/auth/accept-invite", post(handlers::auth::accept_invite));

    let hr_protected = Router::new()
        .route("/v1/interviews", post(handlers::interviews::create))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_hr,
        ));

    let admin_protected = Router::new()
        .route("/v1/admin/invites", post(handlers::auth::create_invite))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_admin,
        ));

    let rate_limit_layer = middleware::from_fn_with_state(state.clone(), crate::rate_limit::check_and_pass);

    Router::new()
        .merge(public)
        .merge(auth_routes)
        .merge(hr_protected)
        .merge(admin_protected)
        .layer(rate_limit_layer)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

fn build_cors(cfg: &Settings) -> CorsLayer {
    let origins: Vec<HeaderValue> = cfg
        .cors_origins
        .iter()
        .filter_map(|o| HeaderValue::from_str(o).ok())
        .collect();

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::PATCH])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .allow_credentials(true)
        .max_age(Duration::from_secs(600))
}

async fn healthz() -> impl IntoResponse {
    Json(json!({ "ok": true }))
}

async fn livez() -> impl IntoResponse {
    // Process is up. /readyz is the one that checks dependencies.
    Json(json!({ "ok": true }))
}

async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    let db_ok = sqlx::query("SELECT 1")
        .execute(&state.pools.primary)
        .await
        .is_ok();

    let redis_ok = match state.redis.as_ref() {
        Some(r) => r.ping().await.is_ok(),
        None => true,
    };

    let s3_ok = state.blob_store.is_some() || !state.cfg.feature_server_pdf;

    let ok = db_ok && redis_ok && s3_ok;
    let status = if ok { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
    (
        status,
        Json(json!({
            "ok": ok,
            "db": db_ok,
            "redis": redis_ok,
            "s3": s3_ok,
        })),
    )
}
