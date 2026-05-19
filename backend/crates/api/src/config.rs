use std::env;

use anyhow::{anyhow, Context, Result};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Settings {
    pub database_url: String,
    pub database_replica_url: Option<String>,
    pub db_max_connections: u32,
    pub listen_addr: String,
    pub metrics_listen_addr: String,
    pub web_base_url: String,
    pub hr_dev_token: String,
    pub default_account_id: Uuid,
    pub session_ttl_hours: i64,
    pub cors_origins: Vec<String>,
    pub default_pass_threshold: i16,

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

    // Phase 5 — server PDF + S3
    pub feature_server_pdf: bool,
    pub s3_bucket: Option<String>,
    pub s3_region: String,
    pub s3_endpoint: Option<String>,
    pub s3_access_key_id: Option<String>,
    pub s3_secret_access_key: Option<String>,
    pub s3_use_path_style: bool,
    pub s3_signed_url_ttl_secs: u32,

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
    pub feature_legacy_hr_token: bool,
    pub access_token_ttl_minutes: i64,
    pub refresh_token_ttl_days: i64,
    pub invite_ttl_hours: i64,
    pub cookies_secure: bool,

    // Rate limit
    pub feature_rate_limit: bool,
    pub rate_limit_create_interview_per_hour: u32,
    pub rate_limit_login_per_min_ip: u32,
    pub rate_limit_login_per_hour_email: u32,
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

    // Phase 6 KMS
    pub kms_provider: String, // "env" | "aws" | "gcp" | "vault"
    pub kms_key_arn: Option<String>,
}

impl Settings {
    pub fn from_env() -> Result<Self> {
        let database_url =
            env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
        let database_replica_url = env::var("DATABASE_REPLICA_URL").ok().filter(|s| !s.is_empty());
        let db_max_connections = env_u32("DB_MAX_CONNECTIONS", 10)?;
        let listen_addr = env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
        let metrics_listen_addr =
            env::var("METRICS_LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:9090".into());
        let web_base_url =
            env::var("WEB_BASE_URL").unwrap_or_else(|_| "http://localhost:4173".into());
        let hr_dev_token = env::var("HR_DEV_TOKEN")
            .map_err(|_| anyhow!("HR_DEV_TOKEN must be set (legacy cutover token)"))?;
        let default_account_id = env::var("DEFAULT_ACCOUNT_ID")
            .unwrap_or_else(|_| "00000000-0000-0000-0000-000000000001".into())
            .parse()
            .context("DEFAULT_ACCOUNT_ID must be a UUID")?;
        let session_ttl_hours = env_i64("SESSION_TTL_HOURS", 72)?;
        let default_pass_threshold = env_i16("DEFAULT_PASS_THRESHOLD", 70)?;
        let cors_origins = env::var("CORS_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:4173,http://127.0.0.1:4173".into())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let anthropic_api_key = env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow!("ANTHROPIC_API_KEY must be set"))?;
        let anthropic_model_priming = env::var("ANTHROPIC_MODEL_PRIMING")
            .unwrap_or_else(|_| "claude-sonnet-4-6".into());
        let anthropic_model_scoring = env::var("ANTHROPIC_MODEL_SCORING")
            .unwrap_or_else(|_| "claude-sonnet-4-6".into());
        let anthropic_model_followup = env::var("ANTHROPIC_MODEL_FOLLOWUP")
            .unwrap_or_else(|_| "claude-sonnet-4-6".into());
        let anthropic_model_grading = env::var("ANTHROPIC_MODEL_GRADING")
            .unwrap_or_else(|_| "claude-haiku-4-5-20251001".into());

        let prime_poll_interval_ms = env_u64("PRIME_POLL_INTERVAL_MS", 1_000)?;
        let prime_batch_size = env_i64("PRIME_BATCH_SIZE", 4)?;

        let feature_transcript_chain = env_bool("FEATURE_TRANSCRIPT_CHAIN", true);
        let transcript_checkpoint_every = env_i32("TRANSCRIPT_CHECKPOINT_EVERY", 50)?;

        let feature_server_pdf = env_bool("FEATURE_SERVER_PDF", true);
        let s3_bucket = env::var("S3_BUCKET").ok().filter(|s| !s.is_empty());
        let s3_region = env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".into());
        let s3_endpoint = env::var("S3_ENDPOINT").ok().filter(|s| !s.is_empty());
        let s3_access_key_id = env::var("S3_ACCESS_KEY_ID").ok().filter(|s| !s.is_empty());
        let s3_secret_access_key = env::var("S3_SECRET_ACCESS_KEY").ok().filter(|s| !s.is_empty());
        let s3_use_path_style = env_bool("S3_PATH_STYLE", true);
        let s3_signed_url_ttl_secs = env_u32("S3_SIGNED_URL_TTL_SECS", 600)?;

        let outbox_poll_interval_ms = env_u64("OUTBOX_POLL_INTERVAL_MS", 2_000)?;
        let outbox_batch_size = env_i64("OUTBOX_BATCH_SIZE", 8)?;
        let outbox_max_attempts = env_i32("OUTBOX_MAX_ATTEMPTS", 5)?;

        let feature_backend_mailer = env_bool("FEATURE_BACKEND_MAILER", true);
        let resend_api_key = env::var("RESEND_API_KEY").ok().filter(|s| !s.is_empty());
        let mail_from_address =
            env::var("MAIL_FROM_ADDRESS").unwrap_or_else(|_| "reports@example.com".into());
        let mail_reply_to = env::var("MAIL_REPLY_TO").ok().filter(|s| !s.is_empty());

        // ─── Phase 6 ───────────────────────────────────────────────────────

        let feature_jwt_auth = env_bool("FEATURE_JWT_AUTH", true);
        let feature_legacy_hr_token = env_bool("FEATURE_LEGACY_HR_TOKEN", true);
        let access_token_ttl_minutes = env_i64("ACCESS_TOKEN_TTL_MINUTES", 15)?;
        let refresh_token_ttl_days = env_i64("REFRESH_TOKEN_TTL_DAYS", 14)?;
        let invite_ttl_hours = env_i64("INVITE_TTL_HOURS", 168)?; // 7 days
        let cookies_secure = env_bool("COOKIES_SECURE", false);

        let feature_rate_limit = env_bool("FEATURE_RATE_LIMIT", false);
        let rate_limit_create_interview_per_hour =
            env_u32("RATE_LIMIT_CREATE_INTERVIEW_PER_HOUR", 60)?;
        let rate_limit_login_per_min_ip = env_u32("RATE_LIMIT_LOGIN_PER_MIN_IP", 10)?;
        let rate_limit_login_per_hour_email = env_u32("RATE_LIMIT_LOGIN_PER_HOUR_EMAIL", 30)?;
        let rate_limit_session_by_code_per_min = env_u32("RATE_LIMIT_SESSION_BY_CODE_PER_MIN", 60)?;
        let rate_limit_ws_per_min = env_u32("RATE_LIMIT_WS_PER_MIN", 10)?;

        let redis_url = env::var("REDIS_URL").ok().filter(|s| !s.is_empty());
        let feature_redis_outbox = env_bool("FEATURE_REDIS_OUTBOX", false);
        let feature_redis_nonces = env_bool("FEATURE_REDIS_NONCES", false);
        let feature_redis_ws_locks = env_bool("FEATURE_REDIS_WS_LOCKS", false);

        let feature_otel = env_bool("FEATURE_OTEL", false);
        let otlp_endpoint = env::var("OTLP_ENDPOINT").ok().filter(|s| !s.is_empty());
        let otel_service_name =
            env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "mock-interview-api".into());

        let kms_provider = env::var("KMS_PROVIDER").unwrap_or_else(|_| "env".into());
        let kms_key_arn = env::var("KMS_KEY_ARN").ok().filter(|s| !s.is_empty());

        Ok(Self {
            database_url,
            database_replica_url,
            db_max_connections,
            listen_addr,
            metrics_listen_addr,
            web_base_url,
            hr_dev_token,
            default_account_id,
            session_ttl_hours,
            cors_origins,
            default_pass_threshold,
            anthropic_api_key,
            anthropic_model_priming,
            anthropic_model_scoring,
            anthropic_model_followup,
            anthropic_model_grading,
            prime_poll_interval_ms,
            prime_batch_size,
            feature_transcript_chain,
            transcript_checkpoint_every,
            feature_server_pdf,
            s3_bucket,
            s3_region,
            s3_endpoint,
            s3_access_key_id,
            s3_secret_access_key,
            s3_use_path_style,
            s3_signed_url_ttl_secs,
            outbox_poll_interval_ms,
            outbox_batch_size,
            outbox_max_attempts,
            feature_backend_mailer,
            resend_api_key,
            mail_from_address,
            mail_reply_to,
            feature_jwt_auth,
            feature_legacy_hr_token,
            access_token_ttl_minutes,
            refresh_token_ttl_days,
            invite_ttl_hours,
            cookies_secure,
            feature_rate_limit,
            rate_limit_create_interview_per_hour,
            rate_limit_login_per_min_ip,
            rate_limit_login_per_hour_email,
            rate_limit_session_by_code_per_min,
            rate_limit_ws_per_min,
            redis_url,
            feature_redis_outbox,
            feature_redis_nonces,
            feature_redis_ws_locks,
            feature_otel,
            otlp_endpoint,
            otel_service_name,
            kms_provider,
            kms_key_arn,
        })
    }
}

fn env_bool(key: &str, default: bool) -> bool {
    match env::var(key) {
        Ok(v) => matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        Err(_) => default,
    }
}

fn env_u32(key: &str, default: u32) -> Result<u32> {
    env::var(key)
        .ok()
        .map(|v| v.parse())
        .transpose()
        .with_context(|| format!("{key} must be a number"))
        .map(|v| v.unwrap_or(default))
}

fn env_u64(key: &str, default: u64) -> Result<u64> {
    env::var(key)
        .ok()
        .map(|v| v.parse())
        .transpose()
        .with_context(|| format!("{key} must be a number"))
        .map(|v| v.unwrap_or(default))
}

fn env_i32(key: &str, default: i32) -> Result<i32> {
    env::var(key)
        .ok()
        .map(|v| v.parse())
        .transpose()
        .with_context(|| format!("{key} must be a number"))
        .map(|v| v.unwrap_or(default))
}

fn env_i16(key: &str, default: i16) -> Result<i16> {
    env::var(key)
        .ok()
        .map(|v| v.parse())
        .transpose()
        .with_context(|| format!("{key} must be a number"))
        .map(|v| v.unwrap_or(default))
}

fn env_i64(key: &str, default: i64) -> Result<i64> {
    env::var(key)
        .ok()
        .map(|v| v.parse())
        .transpose()
        .with_context(|| format!("{key} must be a number"))
        .map(|v| v.unwrap_or(default))
}
