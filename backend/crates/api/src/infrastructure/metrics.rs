//! Phase 6: Prometheus metrics exported on a separate listener.
//!
//! Mounted at `GET /metrics` on `METRICS_LISTEN_ADDR` (default 0.0.0.0:9090).
//! Kept off the main API listener so it is not exposed via CORS and so the
//! collector can scrape it without going through the application LB.

use std::sync::OnceLock;

use axum::{response::IntoResponse, routing::get, Router};
use once_cell::sync::Lazy;
use prometheus::{
    register_counter_vec, register_gauge, register_histogram_vec, register_int_counter_vec,
    CounterVec, Encoder, Gauge, HistogramVec, IntCounterVec, Registry, TextEncoder,
};

static REGISTRY: OnceLock<Registry> = OnceLock::new();

fn registry() -> &'static Registry {
    REGISTRY.get_or_init(Registry::new)
}

pub static HTTP_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "http_requests_total",
        "HTTP requests, partitioned by route/method/status",
        &["route", "method", "status"]
    )
    .expect("register http_requests_total")
});

pub static HTTP_REQUEST_DURATION_SECONDS: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "http_request_duration_seconds",
        "HTTP request latency",
        &["route", "method"],
        vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    )
    .expect("register http_request_duration_seconds")
});

pub static AI_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "ai_requests_total",
        "Anthropic completions issued",
        &["model", "kind", "status"]
    )
    .expect("register ai_requests_total")
});

pub static AI_INPUT_TOKENS_TOTAL: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "ai_input_tokens_total",
        "Input tokens consumed by Anthropic completions",
        &["model", "kind"]
    )
    .expect("register ai_input_tokens_total")
});

pub static AI_OUTPUT_TOKENS_TOTAL: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "ai_output_tokens_total",
        "Output tokens produced by Anthropic completions",
        &["model", "kind"]
    )
    .expect("register ai_output_tokens_total")
});

pub static AI_LATENCY_MS: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "ai_latency_ms",
        "Anthropic completion latency (ms)",
        &["model", "kind"],
        vec![50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10_000.0, 20_000.0, 60_000.0]
    )
    .expect("register ai_latency_ms")
});

pub static OUTBOX_PENDING: Lazy<Gauge> = Lazy::new(|| {
    register_gauge!("outbox_pending", "Pending event_outbox rows").expect("register outbox_pending")
});

pub static OUTBOX_ATTEMPTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "outbox_attempts_total",
        "Outbox dispatch attempts",
        &["topic", "status"]
    )
    .expect("register outbox_attempts_total")
});

pub static OUTBOX_DISPATCH_LATENCY_MS: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "outbox_dispatch_latency_ms",
        "Time spent dispatching an outbox event",
        &["topic"],
        vec![5.0, 25.0, 100.0, 500.0, 2_500.0, 10_000.0, 60_000.0]
    )
    .expect("register outbox_dispatch_latency_ms")
});

pub static WS_SESSIONS_ACTIVE: Lazy<Gauge> = Lazy::new(|| {
    register_gauge!("ws_sessions_active", "Live WebSocket sessions")
        .expect("register ws_sessions_active")
});

pub static WS_UTTERANCES_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!("ws_utterances_total", "Utterances received", &["final"])
        .expect("register ws_utterances_total")
});

pub static TRANSCRIPT_CHUNKS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "transcript_chunks_total",
        "Transcript chunks appended to the hash chain",
        &["final"]
    )
    .expect("register transcript_chunks_total")
});

pub static PDF_RENDER_DURATION_MS: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "pdf_render_duration_ms",
        "Server-side PDF render latency (ms)",
        &["status"],
        vec![50.0, 100.0, 250.0, 500.0, 1_000.0, 2_500.0, 5_000.0, 15_000.0]
    )
    .expect("register pdf_render_duration_ms")
});

pub static MAIL_SEND_DURATION_MS: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "mail_send_duration_ms",
        "Resend POST latency (ms)",
        &["status"],
        vec![50.0, 100.0, 250.0, 500.0, 1_000.0, 2_500.0, 5_000.0]
    )
    .expect("register mail_send_duration_ms")
});

pub static RATE_LIMITED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "rate_limited_total",
        "Requests rejected by the rate limiter",
        &["route"]
    )
    .expect("register rate_limited_total")
});

pub fn inc_rate_limited(route: &str) {
    RATE_LIMITED_TOTAL.with_label_values(&[route]).inc();
}

/// Lazily forces registration so the /metrics endpoint reports every series
/// at zero from the first scrape (Prometheus best-practice).
pub fn init() {
    Lazy::force(&HTTP_REQUESTS_TOTAL);
    Lazy::force(&HTTP_REQUEST_DURATION_SECONDS);
    Lazy::force(&AI_REQUESTS_TOTAL);
    Lazy::force(&AI_INPUT_TOKENS_TOTAL);
    Lazy::force(&AI_OUTPUT_TOKENS_TOTAL);
    Lazy::force(&AI_LATENCY_MS);
    Lazy::force(&OUTBOX_PENDING);
    Lazy::force(&OUTBOX_ATTEMPTS_TOTAL);
    Lazy::force(&OUTBOX_DISPATCH_LATENCY_MS);
    Lazy::force(&WS_SESSIONS_ACTIVE);
    Lazy::force(&WS_UTTERANCES_TOTAL);
    Lazy::force(&TRANSCRIPT_CHUNKS_TOTAL);
    Lazy::force(&PDF_RENDER_DURATION_MS);
    Lazy::force(&MAIL_SEND_DURATION_MS);
    Lazy::force(&RATE_LIMITED_TOTAL);

    // Mirror our prometheus registry's gathered metrics into the global one
    // so /metrics returns everything.
    let r = registry();
    for collector in [
        Box::new(HTTP_REQUESTS_TOTAL.clone()) as Box<dyn prometheus::core::Collector>,
        Box::new(HTTP_REQUEST_DURATION_SECONDS.clone()),
        Box::new(AI_REQUESTS_TOTAL.clone()),
        Box::new(AI_INPUT_TOKENS_TOTAL.clone()),
        Box::new(AI_OUTPUT_TOKENS_TOTAL.clone()),
        Box::new(AI_LATENCY_MS.clone()),
        Box::new(OUTBOX_PENDING.clone()),
        Box::new(OUTBOX_ATTEMPTS_TOTAL.clone()),
        Box::new(OUTBOX_DISPATCH_LATENCY_MS.clone()),
        Box::new(WS_SESSIONS_ACTIVE.clone()),
        Box::new(WS_UTTERANCES_TOTAL.clone()),
        Box::new(TRANSCRIPT_CHUNKS_TOTAL.clone()),
        Box::new(PDF_RENDER_DURATION_MS.clone()),
        Box::new(MAIL_SEND_DURATION_MS.clone()),
        Box::new(RATE_LIMITED_TOTAL.clone()),
    ] {
        let _ = r.register(collector);
    }
}

pub fn router() -> Router {
    Router::new().route("/metrics", get(handler))
}

async fn handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = registry().gather();
    let mut buf = Vec::with_capacity(4096);
    if let Err(e) = encoder.encode(&metric_families, &mut buf) {
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("encode: {e}"),
        )
            .into_response();
    }
    // Also include the default global registry so we don't miss anything
    // libraries register there.
    let default = prometheus::gather();
    if let Err(e) = encoder.encode(&default, &mut buf) {
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("encode: {e}"),
        )
            .into_response();
    }
    (
        [(axum::http::header::CONTENT_TYPE, encoder.format_type())],
        buf,
    )
        .into_response()
}
