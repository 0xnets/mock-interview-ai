-- Consolidated initial schema for the mock interview platform.
-- Forward-only. This file represents the final schema previously produced by
-- migrations 0001 through 0007.

CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE EXTENSION IF NOT EXISTS citext;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE TABLE accounts (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    email           citext NOT NULL UNIQUE,
    password_hash   text,
    display_name    text,
    org_id          uuid,
    role            text NOT NULL CHECK (role IN ('hr', 'admin', 'super_admin')),
    status          text NOT NULL DEFAULT 'invited' CHECK (status IN ('invited', 'active', 'disabled')),
    invited_by      uuid REFERENCES accounts(id),
    last_login_at   timestamptz,
    created_at      timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE account_invites (
    token_hash      bytea PRIMARY KEY,
    account_id      uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    invited_by      uuid NOT NULL REFERENCES accounts(id),
    expires_at      timestamptz NOT NULL,
    consumed_at     timestamptz,
    created_at      timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX idx_invites_account ON account_invites(account_id);

CREATE TABLE interview_sessions (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id      uuid NOT NULL REFERENCES accounts(id),
    candidate_name  text NOT NULL,
    role_title      text NOT NULL,
    jd_text         text NOT NULL,
    resume_text     text NOT NULL,
    state           text NOT NULL CHECK (state IN
                       ('pending', 'primed', 'active', 'paused', 'completed', 'scored', 'expired', 'aborted')),
    shortcode       text NOT NULL UNIQUE,
    expires_at      timestamptz NOT NULL,
    started_at      timestamptz,
    completed_at    timestamptz,
    pass_threshold  smallint NOT NULL,
    hr_email        citext NOT NULL,
    config_snapshot jsonb NOT NULL,
    scoring_context text,
    created_at      timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_sessions_state ON interview_sessions(state);
CREATE INDEX idx_sessions_acct_created ON interview_sessions(account_id, created_at DESC);

CREATE TABLE questions (
    id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id         uuid NOT NULL REFERENCES interview_sessions(id) ON DELETE CASCADE,
    ordinal            smallint NOT NULL,
    kind               text NOT NULL CHECK (kind IN ('intro', 'technical', 'behavioral', 'followup')),
    parent_question_id uuid REFERENCES questions(id),
    topic              text,
    prompt_text        text NOT NULL,
    generated_by       text NOT NULL,
    generated_at       timestamptz NOT NULL DEFAULT now(),
    UNIQUE (session_id, ordinal)
);

CREATE INDEX idx_q_session_ordinal ON questions(session_id, ordinal);

CREATE TABLE answers (
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    question_id      uuid NOT NULL UNIQUE REFERENCES questions(id) ON DELETE CASCADE,
    session_id       uuid NOT NULL REFERENCES interview_sessions(id) ON DELETE CASCADE,
    transcript_text  text NOT NULL,
    duration_ms      integer NOT NULL,
    submitted_at     timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE transcript_chunks (
    id           bigserial PRIMARY KEY,
    session_id   uuid NOT NULL REFERENCES interview_sessions(id) ON DELETE CASCADE,
    question_id  uuid NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
    seq          integer NOT NULL,
    text         text NOT NULL,
    is_final     boolean NOT NULL,
    client_ts_ms bigint NOT NULL,
    server_ts    timestamptz NOT NULL DEFAULT now(),
    prev_hash    bytea NOT NULL,
    row_hash     bytea NOT NULL,
    UNIQUE (session_id, seq)
);

CREATE INDEX idx_chunks_session_seq ON transcript_chunks(session_id, seq);

CREATE TABLE transcript_checkpoints (
    id           bigserial PRIMARY KEY,
    session_id   uuid NOT NULL REFERENCES interview_sessions(id) ON DELETE CASCADE,
    last_seq     integer NOT NULL,
    last_row_id  bigint NOT NULL REFERENCES transcript_chunks(id),
    row_hash     bytea NOT NULL,
    signature    bytea NOT NULL,
    key_id       text NOT NULL,
    chunk_count  integer NOT NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (session_id, last_seq)
);

CREATE INDEX idx_checkpoints_session ON transcript_checkpoints(session_id, last_seq);

CREATE TABLE question_grades (
    question_id    uuid PRIMARY KEY REFERENCES questions(id) ON DELETE CASCADE,
    score          smallint NOT NULL,
    reasoning      text NOT NULL,
    model          text NOT NULL,
    prompt_version text NOT NULL,
    graded_at      timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE reports (
    id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id     uuid NOT NULL UNIQUE REFERENCES interview_sessions(id) ON DELETE CASCADE,
    overall        smallint NOT NULL,
    technical      smallint NOT NULL,
    behavioral     smallint NOT NULL,
    passed         boolean NOT NULL,
    summary        text NOT NULL,
    strengths      jsonb NOT NULL,
    weaknesses     jsonb NOT NULL,
    action_items   jsonb NOT NULL,
    raw_ai_output  jsonb NOT NULL,
    model          text NOT NULL,
    prompt_version text NOT NULL,
    mail_status    text NOT NULL DEFAULT 'pending'
        CHECK (mail_status IN ('pending', 'sent', 'failed')),
    mail_sent_at   timestamptz,
    mail_error     text,
    generated_at   timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE event_outbox (
    id            bigserial PRIMARY KEY,
    topic         text NOT NULL,
    payload       jsonb NOT NULL,
    attempts      integer NOT NULL DEFAULT 0,
    locked_until  timestamptz,
    last_error    text,
    created_at    timestamptz NOT NULL DEFAULT now(),
    dispatched_at timestamptz
);

CREATE INDEX idx_outbox_pending ON event_outbox(id) WHERE dispatched_at IS NULL;
CREATE INDEX idx_outbox_lease ON event_outbox(id) WHERE dispatched_at IS NULL;

CREATE TABLE shortlinks (
    code        text PRIMARY KEY,
    session_id  uuid NOT NULL REFERENCES interview_sessions(id) ON DELETE CASCADE,
    hit_count   integer NOT NULL DEFAULT 0,
    consumed_at timestamptz,
    created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE audit_log (
    id         bigserial PRIMARY KEY,
    actor_id   uuid,
    session_id uuid,
    action     text NOT NULL,
    metadata   jsonb,
    event_id   uuid,
    at         timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_event ON audit_log(event_id) WHERE event_id IS NOT NULL;
CREATE INDEX idx_audit_session_at ON audit_log(session_id, at DESC);
CREATE INDEX idx_audit_actor_at ON audit_log(actor_id, at DESC);

CREATE TABLE refresh_tokens (
    token_hash    bytea PRIMARY KEY,
    account_id    uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    issued_at     timestamptz NOT NULL DEFAULT now(),
    expires_at    timestamptz NOT NULL,
    revoked_at    timestamptz,
    last_used_at  timestamptz,
    parent_hash   bytea REFERENCES refresh_tokens(token_hash),
    user_agent    text,
    ip_inet       inet
);

CREATE INDEX idx_refresh_account_active
    ON refresh_tokens(account_id)
    WHERE revoked_at IS NULL;
CREATE INDEX idx_refresh_expires
    ON refresh_tokens(expires_at)
    WHERE revoked_at IS NULL;

CREATE TABLE key_versions (
    key_id      text PRIMARY KEY,
    public_key  bytea NOT NULL,
    status      text NOT NULL CHECK (status IN ('active', 'retired', 'compromised')),
    created_at  timestamptz NOT NULL DEFAULT now(),
    retired_at  timestamptz,
    notes       text
);

CREATE TABLE rate_limit_overrides (
    id           bigserial PRIMARY KEY,
    account_id   uuid REFERENCES accounts(id) ON DELETE CASCADE,
    route        text,
    limit_count  integer NOT NULL CHECK (limit_count > 0),
    window_secs  integer NOT NULL CHECK (window_secs > 0),
    note         text,
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (account_id, route)
);

-- Fixed dev HR account used by legacy session references.
INSERT INTO accounts (id, email, role, status, display_name)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    'default-hr@local',
    'hr',
    'active',
    'Default HR (dev)'
)
ON CONFLICT (id) DO NOTHING;

-- Local admin account for fresh development databases.
-- Password is "admin-local-password".
INSERT INTO accounts (id, email, password_hash, role, status, display_name)
VALUES (
    '00000000-0000-0000-0000-000000000007',
    'admin@local.test',
    '$argon2id$v=19$m=19456,t=2,p=1$bG9jYWxhZG1pbnNhbHQwMQ$r4wrcuVkLIIcm7uHd6RR0lszwIzlO1AZ5l+YtRKIfUU',
    'admin',
    'active',
    'Local Admin'
)
ON CONFLICT (id) DO UPDATE
SET email = EXCLUDED.email,
    password_hash = EXCLUDED.password_hash,
    role = EXCLUDED.role,
    status = EXCLUDED.status,
    display_name = EXCLUDED.display_name;

-- Seed the dev key catalog so the verifier can pick it up via the API.
-- Public bytes derive from the deterministic dev seed in api/src/signing.rs;
-- the API refreshes this placeholder on startup from the active signer.
INSERT INTO key_versions (key_id, public_key, status, notes)
VALUES (
    'dev-v1',
    '\x00'::bytea,
    'active',
    'Dev key. Replaced by KMS in production.'
) ON CONFLICT (key_id) DO NOTHING;
