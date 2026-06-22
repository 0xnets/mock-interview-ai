//! Environment-variable loading for [`Settings`].
//!
//! Default values come from [`defaults`](super::defaults); this module only
//! reads env vars and falls back to those constants. Env var names are part of
//! the deployment contract — do not rename them.

use std::env;

use anyhow::{anyhow, Context, Result};

use super::{defaults, validation, Settings};

impl Settings {
    pub fn from_env() -> Result<Self> {
        let database_url = env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
        let database_replica_url = env::var("DATABASE_REPLICA_URL")
            .ok()
            .filter(|s| !s.is_empty());
        let db_max_connections = env_u32("DB_MAX_CONNECTIONS", defaults::DB_MAX_CONNECTIONS)?;
        let listen_addr = env::var("LISTEN_ADDR").unwrap_or_else(|_| defaults::LISTEN_ADDR.into());
        let metrics_listen_addr = env::var("METRICS_LISTEN_ADDR")
            .unwrap_or_else(|_| defaults::METRICS_LISTEN_ADDR.into());
        let web_base_url =
            env::var("WEB_BASE_URL").unwrap_or_else(|_| defaults::WEB_BASE_URL.into());
        let session_ttl_hours = env_i64("SESSION_TTL_HOURS", defaults::SESSION_TTL_HOURS)?;
        let default_pass_threshold =
            env_i16("DEFAULT_PASS_THRESHOLD", defaults::DEFAULT_PASS_THRESHOLD)?;
        let shortlink_incompat_limit = env_i32(
            "SHORTLINK_INCOMPAT_LIMIT",
            defaults::SHORTLINK_INCOMPAT_LIMIT,
        )?;
        let cors_origins = env::var("CORS_ORIGINS")
            .unwrap_or_else(|_| defaults::CORS_ORIGINS.into())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let anthropic_api_key =
            env::var("ANTHROPIC_API_KEY").map_err(|_| anyhow!("ANTHROPIC_API_KEY must be set"))?;
        let anthropic_model_priming = env::var("ANTHROPIC_MODEL_PRIMING")
            .unwrap_or_else(|_| defaults::ANTHROPIC_MODEL_PRIMING.into());
        let anthropic_model_scoring = env::var("ANTHROPIC_MODEL_SCORING")
            .unwrap_or_else(|_| defaults::ANTHROPIC_MODEL_SCORING.into());
        let anthropic_model_followup = env::var("ANTHROPIC_MODEL_FOLLOWUP")
            .unwrap_or_else(|_| defaults::ANTHROPIC_MODEL_FOLLOWUP.into());
        let anthropic_model_grading = env::var("ANTHROPIC_MODEL_GRADING")
            .unwrap_or_else(|_| defaults::ANTHROPIC_MODEL_GRADING.into());

        let assemblyai_api_key = env::var("ASSEMBLYAI_API_KEY")
            .map_err(|_| anyhow!("ASSEMBLYAI_API_KEY must be set"))?;
        let assemblyai_streaming_url = env::var("ASSEMBLYAI_STREAMING_URL")
            .unwrap_or_else(|_| defaults::ASSEMBLYAI_STREAMING_URL.into());
        let assemblyai_sample_rate =
            env_u32("ASSEMBLYAI_SAMPLE_RATE", defaults::ASSEMBLYAI_SAMPLE_RATE)?;
        let assemblyai_speech_model = env::var("ASSEMBLYAI_SPEECH_MODEL")
            .unwrap_or_else(|_| defaults::ASSEMBLYAI_SPEECH_MODEL.into());
        let assemblyai_format_turns =
            env_bool("ASSEMBLYAI_FORMAT_TURNS", defaults::ASSEMBLYAI_FORMAT_TURNS);
        let assemblyai_audio_channel_capacity = env_usize(
            "ASSEMBLYAI_AUDIO_CHANNEL_CAPACITY",
            defaults::ASSEMBLYAI_AUDIO_CHANNEL_CAPACITY,
        )?;
        let stt_event_channel_capacity = env_usize(
            "STT_EVENT_CHANNEL_CAPACITY",
            defaults::STT_EVENT_CHANNEL_CAPACITY,
        )?;

        let prime_poll_interval_ms =
            env_u64("PRIME_POLL_INTERVAL_MS", defaults::PRIME_POLL_INTERVAL_MS)?;
        let prime_batch_size = env_i64("PRIME_BATCH_SIZE", defaults::PRIME_BATCH_SIZE)?;

        let feature_transcript_chain = env_bool(
            "FEATURE_TRANSCRIPT_CHAIN",
            defaults::FEATURE_TRANSCRIPT_CHAIN,
        );
        let transcript_checkpoint_every = env_i32(
            "TRANSCRIPT_CHECKPOINT_EVERY",
            defaults::TRANSCRIPT_CHECKPOINT_EVERY,
        )?;

        let outbox_poll_interval_ms =
            env_u64("OUTBOX_POLL_INTERVAL_MS", defaults::OUTBOX_POLL_INTERVAL_MS)?;
        let outbox_batch_size = env_i64("OUTBOX_BATCH_SIZE", defaults::OUTBOX_BATCH_SIZE)?;
        let outbox_max_attempts = env_i32("OUTBOX_MAX_ATTEMPTS", defaults::OUTBOX_MAX_ATTEMPTS)?;

        let feature_backend_mailer =
            env_bool("FEATURE_BACKEND_MAILER", defaults::FEATURE_BACKEND_MAILER);
        let resend_api_key = env::var("RESEND_API_KEY").ok().filter(|s| !s.is_empty());
        let mail_from_address =
            env::var("MAIL_FROM_ADDRESS").unwrap_or_else(|_| defaults::MAIL_FROM_ADDRESS.into());
        let mail_reply_to = env::var("MAIL_REPLY_TO").ok().filter(|s| !s.is_empty());

        let feature_retention_worker = env_bool(
            "FEATURE_RETENTION_WORKER",
            defaults::FEATURE_RETENTION_WORKER,
        );
        let retention_poll_interval_secs = env_u64(
            "RETENTION_POLL_INTERVAL_SECS",
            defaults::RETENTION_POLL_INTERVAL_SECS,
        )?;
        let retention_transcript_days = env_i64(
            "RETENTION_TRANSCRIPT_DAYS",
            defaults::RETENTION_TRANSCRIPT_DAYS,
        )?;
        let retention_answers_days =
            env_i64("RETENTION_ANSWERS_DAYS", defaults::RETENTION_ANSWERS_DAYS)?;
        let retention_pii_days = env_i64("RETENTION_PII_DAYS", defaults::RETENTION_PII_DAYS)?;
        let retention_outbox_days =
            env_i64("RETENTION_OUTBOX_DAYS", defaults::RETENTION_OUTBOX_DAYS)?;

        // ─── Phase 6 ───────────────────────────────────────────────────────

        let feature_jwt_auth = env_bool("FEATURE_JWT_AUTH", defaults::FEATURE_JWT_AUTH);
        let access_token_ttl_minutes = env_i64(
            "ACCESS_TOKEN_TTL_MINUTES",
            defaults::ACCESS_TOKEN_TTL_MINUTES,
        )?;
        let refresh_token_ttl_days =
            env_i64("REFRESH_TOKEN_TTL_DAYS", defaults::REFRESH_TOKEN_TTL_DAYS)?;
        let invite_ttl_hours = env_i64("INVITE_TTL_HOURS", defaults::INVITE_TTL_HOURS)?;
        let cookies_secure = env_bool("COOKIES_SECURE", defaults::COOKIES_SECURE);

        let feature_rate_limit = env_bool("FEATURE_RATE_LIMIT", defaults::FEATURE_RATE_LIMIT);
        let rate_limit_create_interview_per_hour = env_u32(
            "RATE_LIMIT_CREATE_INTERVIEW_PER_HOUR",
            defaults::RATE_LIMIT_CREATE_INTERVIEW_PER_HOUR,
        )?;
        let rate_limit_login_per_min_ip = env_u32(
            "RATE_LIMIT_LOGIN_PER_MIN_IP",
            defaults::RATE_LIMIT_LOGIN_PER_MIN_IP,
        )?;
        let rate_limit_session_by_code_per_min = env_u32(
            "RATE_LIMIT_SESSION_BY_CODE_PER_MIN",
            defaults::RATE_LIMIT_SESSION_BY_CODE_PER_MIN,
        )?;
        let rate_limit_ws_per_min =
            env_u32("RATE_LIMIT_WS_PER_MIN", defaults::RATE_LIMIT_WS_PER_MIN)?;

        let redis_url = env::var("REDIS_URL").ok().filter(|s| !s.is_empty());
        let feature_redis_outbox = env_bool("FEATURE_REDIS_OUTBOX", defaults::FEATURE_REDIS_OUTBOX);
        let feature_redis_nonces = env_bool("FEATURE_REDIS_NONCES", defaults::FEATURE_REDIS_NONCES);
        let feature_redis_ws_locks =
            env_bool("FEATURE_REDIS_WS_LOCKS", defaults::FEATURE_REDIS_WS_LOCKS);

        let feature_otel = env_bool("FEATURE_OTEL", defaults::FEATURE_OTEL);
        let otlp_endpoint = env::var("OTLP_ENDPOINT").ok().filter(|s| !s.is_empty());
        let otel_service_name =
            env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| defaults::OTEL_SERVICE_NAME.into());

        let settings = Self {
            database_url,
            database_replica_url,
            db_max_connections,
            listen_addr,
            metrics_listen_addr,
            web_base_url,
            session_ttl_hours,
            cors_origins,
            default_pass_threshold,
            shortlink_incompat_limit,
            anthropic_api_key,
            anthropic_model_priming,
            anthropic_model_scoring,
            anthropic_model_followup,
            anthropic_model_grading,
            assemblyai_api_key,
            assemblyai_streaming_url,
            assemblyai_sample_rate,
            assemblyai_speech_model,
            assemblyai_format_turns,
            assemblyai_audio_channel_capacity,
            stt_event_channel_capacity,
            prime_poll_interval_ms,
            prime_batch_size,
            feature_transcript_chain,
            transcript_checkpoint_every,
            outbox_poll_interval_ms,
            outbox_batch_size,
            outbox_max_attempts,
            feature_backend_mailer,
            resend_api_key,
            mail_from_address,
            mail_reply_to,
            feature_retention_worker,
            retention_poll_interval_secs,
            retention_transcript_days,
            retention_answers_days,
            retention_pii_days,
            retention_outbox_days,
            feature_jwt_auth,
            access_token_ttl_minutes,
            refresh_token_ttl_days,
            invite_ttl_hours,
            cookies_secure,
            feature_rate_limit,
            rate_limit_create_interview_per_hour,
            rate_limit_login_per_min_ip,
            rate_limit_session_by_code_per_min,
            rate_limit_ws_per_min,
            redis_url,
            feature_redis_outbox,
            feature_redis_nonces,
            feature_redis_ws_locks,
            feature_otel,
            otlp_endpoint,
            otel_service_name,
        };
        validation::validate(&settings)?;
        Ok(settings)
    }
}

fn env_bool(key: &str, default: bool) -> bool {
    match env::var(key) {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
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

fn env_usize(key: &str, default: usize) -> Result<usize> {
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
