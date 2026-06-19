//! Centralized default values for [`Settings`](super::Settings).
//!
//! Every config field that has a fallback gets its default here as a typed
//! constant, so defaults can be audited and changed in one place. Required
//! fields (`DATABASE_URL`, `ANTHROPIC_API_KEY`) and optional fields with no
//! fallback are intentionally absent — they have no default to centralize.

// ─── Core server ───────────────────────────────────────────────────────────
pub const DB_MAX_CONNECTIONS: u32 = 10;
pub const LISTEN_ADDR: &str = "0.0.0.0:8080";
pub const METRICS_LISTEN_ADDR: &str = "0.0.0.0:9090";
pub const WEB_BASE_URL: &str = "http://localhost:4173";
pub const SESSION_TTL_HOURS: i64 = 72;
pub const DEFAULT_PASS_THRESHOLD: i16 = 70;

// ─── Validation policy ─────────────────────────────────────────────────────
pub const MAX_CANDIDATE_NAME_CHARS: usize = 120;
pub const MAX_ROLE_TITLE_CHARS: usize = 160;
pub const MIN_JD_WORDS: usize = 20;
pub const MIN_RESUME_WORDS: usize = 30;
pub const MAX_JD_BYTES: usize = 64 * 1024;
pub const MAX_RESUME_BYTES: usize = 256 * 1024;
pub const MIN_INVITE_PASSWORD_CHARS: usize = 12;

// ─── HR configuration policy ───────────────────────────────────────────────
pub const MIN_TECH_COUNT: u16 = 1;
pub const MAX_TECH_COUNT: u16 = 20;
pub const MAX_BEHAVIORAL_COUNT: u16 = 20;
pub const MIN_PASS_THRESHOLD: i16 = 0;
pub const MAX_PASS_THRESHOLD: i16 = 100;
pub const MIN_ANSWER_TIME_LIMIT_MS: u32 = 30_000;
pub const MAX_ANSWER_TIME_LIMIT_MS: u32 = 600_000;
pub const DEFAULT_ANSWER_TIME_LIMIT_MS: u32 = 180_000;
pub const MAX_BANK_SECTIONS: usize = 20;
pub const MAX_BANK_QUESTIONS: usize = 400;
pub const MAX_BANK_SECTION_CHARS: usize = 80;
pub const MAX_BANK_QUESTION_CHARS: usize = 500;
pub const DEFAULT_TECH_COUNT: u16 = 5;

/// Failed pre-interview system checks a candidate may rack up before the
/// interview link is expired and they are sent to HR for a new one.
pub const SHORTLINK_INCOMPAT_LIMIT: i32 = 5;
pub const CORS_ORIGINS: &str =
    "http://localhost:4173,http://127.0.0.1:4173,http://localhost:5173,http://127.0.0.1:5173";

// ─── AI provider ───────────────────────────────────────────────────────────
pub const ANTHROPIC_MODEL_PRIMING: &str = "claude-sonnet-4-6";
pub const ANTHROPIC_MODEL_SCORING: &str = "claude-sonnet-4-6";
pub const ANTHROPIC_MODEL_FOLLOWUP: &str = "claude-sonnet-4-6";
pub const ANTHROPIC_MODEL_GRADING: &str = "claude-haiku-4-5-20251001";

// ─── Priming worker ────────────────────────────────────────────────────────
pub const PRIME_POLL_INTERVAL_MS: u64 = 1_000;
pub const PRIME_BATCH_SIZE: i64 = 4;

// ─── Phase 5 — transcript chain ────────────────────────────────────────────
pub const FEATURE_TRANSCRIPT_CHAIN: bool = true;
pub const TRANSCRIPT_CHECKPOINT_EVERY: i32 = 50;

// ─── Phase 5 — outbox dispatcher ───────────────────────────────────────────
pub const OUTBOX_POLL_INTERVAL_MS: u64 = 2_000;
pub const OUTBOX_BATCH_SIZE: i64 = 8;
pub const OUTBOX_MAX_ATTEMPTS: i32 = 5;

// ─── Phase 5 — backend mailer ──────────────────────────────────────────────
pub const FEATURE_BACKEND_MAILER: bool = true;
pub const MAIL_FROM_ADDRESS: &str = "reports@example.com";

// ─── Retention worker ──────────────────────────────────────────────────────
pub const FEATURE_RETENTION_WORKER: bool = true;
pub const RETENTION_POLL_INTERVAL_SECS: u64 = 86_400;
pub const RETENTION_TRANSCRIPT_DAYS: i64 = 7;
pub const RETENTION_ANSWERS_DAYS: i64 = 30;
pub const RETENTION_PII_DAYS: i64 = 30;
pub const RETENTION_OUTBOX_DAYS: i64 = 30;

// ─── Phase 6 — auth + cookies ──────────────────────────────────────────────
pub const FEATURE_JWT_AUTH: bool = true;
pub const ACCESS_TOKEN_TTL_MINUTES: i64 = 15;
pub const REFRESH_TOKEN_TTL_DAYS: i64 = 14;
pub const INVITE_TTL_HOURS: i64 = 168; // 7 days
pub const COOKIES_SECURE: bool = false;

// ─── Phase 6 — rate limit ──────────────────────────────────────────────────
pub const FEATURE_RATE_LIMIT: bool = false;
pub const RATE_LIMIT_CREATE_INTERVIEW_PER_HOUR: u32 = 60;
pub const RATE_LIMIT_LOGIN_PER_MIN_IP: u32 = 10;
pub const RATE_LIMIT_SESSION_BY_CODE_PER_MIN: u32 = 60;
pub const RATE_LIMIT_WS_PER_MIN: u32 = 10;

// ─── Phase 6 — Redis backplane ─────────────────────────────────────────────
pub const FEATURE_REDIS_OUTBOX: bool = false;
pub const FEATURE_REDIS_NONCES: bool = false;
pub const FEATURE_REDIS_WS_LOCKS: bool = false;

// ─── Phase 6 — observability ───────────────────────────────────────────────
pub const FEATURE_OTEL: bool = false;
pub const OTEL_SERVICE_NAME: &str = "mock-interview-api";
