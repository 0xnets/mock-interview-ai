//! HTTP router composition: route table, auth/rate-limit middleware, CORS.

use std::time::Duration;

use axum::{
    http::{header, HeaderValue, Method},
    middleware,
    routing::{get, post},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{auth, config::Settings, controllers, realtime};

use super::health::{healthz, livez, readyz};
use super::AppState;

pub fn router(state: AppState) -> Router {
    let cors = build_cors(&state.cfg);

    let public = Router::new()
        .route("/healthz", get(healthz))
        .route("/livez", get(livez))
        .route("/readyz", get(readyz))
        .route(
            "/s/{code}",
            get(controllers::shortlink_controller::redirect_to_session),
        )
        .route(
            "/v1/sessions/by-code/{code}",
            get(controllers::sessions_controller::get_by_code),
        )
        .route(
            "/v1/sessions/by-code/{code}/join",
            get(controllers::sessions_controller::issue_join_nonce)
                .post(controllers::sessions_controller::issue_join_nonce),
        )
        .route("/v1/ws/interview", get(realtime::ws_handler))
        .route(
            "/v1/interviews/{id}/finalize",
            post(controllers::interviews_controller::finalize),
        )
        .route(
            "/v1/interviews/{id}/transcript",
            get(controllers::transcripts_controller::get_transcript),
        )
        .route(
            "/v1/interviews/{id}/report.pdf",
            get(controllers::reports_controller::get_report_pdf),
        );

    // Phase 6: auth surface. Login + refresh + invite are public-ish (they
    // accept anonymous traffic but are rate-limited); logout authenticates
    // via the refresh cookie.
    let auth_routes = Router::new()
        .route("/v1/auth/login", post(controllers::auth_controller::login))
        .route(
            "/v1/auth/refresh",
            post(controllers::auth_controller::refresh),
        )
        .route(
            "/v1/auth/logout",
            post(controllers::auth_controller::logout),
        )
        .route(
            "/v1/auth/accept-invite",
            post(controllers::auth_controller::accept_invite),
        );

    let hr_protected = Router::new()
        .route(
            "/v1/interviews",
            post(controllers::interviews_controller::create),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_hr,
        ));

    let admin_protected = Router::new()
        .route(
            "/v1/admin/invites",
            post(controllers::auth_controller::create_invite),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_admin,
        ));

    let rate_limit_layer =
        middleware::from_fn_with_state(state.clone(), crate::rate_limit::check_and_pass);

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
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .allow_credentials(true)
        .max_age(Duration::from_secs(600))
}
