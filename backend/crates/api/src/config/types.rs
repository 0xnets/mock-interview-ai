//! Typed configuration shape. See [`env`](super::env) for environment loading
//! and [`defaults`](super::defaults) for fallback values.

/// Fully-resolved API configuration, built once at startup by
/// [`Settings::from_env`](super::Settings::from_env).
#[derive(Debug, Clone)]
pub struct Settings {
    pub database_url: String,
    pub database_replica_url: Option<String>,
    pub db_max_connections: u32,
    pub listen_addr: String,
    pub metrics_listen_addr: String,
    pub web_base_url: String,
    pub session_ttl_hours: i64,
    pub cors_origins: Vec<String>,
    pub default_pass_threshold: i16,
    pub shortlink_incompat_limit: i32,

    // AI provider
    pub anthropic_api_key: String,
    pub anthropic_model_priming: String,
    pub anthropic_model_scoring: String,
    pub anthropic_model_followup: String,
    pub anthropic_model_grading: String,

    // Priming worker
    pub prime_poll_interval_ms: u64,
    pub prime_batch_size: i64,

    // Phase 5 — transcript chain
    pub feature_transcript_chain: bool,
    pub transcript_checkpoint_every: i32,

    // Phase 5 — outbox dispatcher
    pub outbox_poll_interval_ms: u64,
    pub outbox_batch_size: i64,
    pub outbox_max_attempts: i32,

    // Phase 5 — backend mailer
    pub feature_backend_mailer: bool,
    pub resend_api_key: Option<String>,
    pub mail_from_address: String,
    pub mail_reply_to: Option<String>,

    // ─── Phase 6 ───────────────────────────────────────────────────────────

    // Auth + cookies
    pub feature_jwt_auth: bool,
    pub access_token_ttl_minutes: i64,
    pub refresh_token_ttl_days: i64,
    pub invite_ttl_hours: i64,
    pub cookies_secure: bool,

    // Rate limit
    pub feature_rate_limit: bool,
    pub rate_limit_create_interview_per_hour: u32,
    pub rate_limit_login_per_min_ip: u32,
    pub rate_limit_session_by_code_per_min: u32,
    pub rate_limit_ws_per_min: u32,

    // Redis backplane (shared by rate limit / streams / nonces / Redlock)
    pub redis_url: Option<String>,
    pub feature_redis_outbox: bool,
    pub feature_redis_nonces: bool,
    pub feature_redis_ws_locks: bool,

    // Observability
    pub feature_otel: bool,
    pub otlp_endpoint: Option<String>,
    pub otel_service_name: String,
}
