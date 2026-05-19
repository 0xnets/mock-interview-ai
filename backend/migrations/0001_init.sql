-- Initial schema for the mock interview platform.
-- Forward-only. Aligns with the Phase 2-5 plan; later phases reuse this baseline.

CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE EXTENSION IF NOT EXISTS citext;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE TABLE accounts (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    email           citext NOT NULL UNIQUE,
    password_hash   text,
    display_name    text,
    org_id          uuid,
    role            text NOT NULL CHECK (role IN ('hr', 'admin')),
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

CREATE TABLE interview_configs (
    id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id          uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name                text NOT NULL,
    tech_count          smallint NOT NULL DEFAULT 10,
    behavioral_count    smallint NOT NULL DEFAULT 3,
    include_intro       boolean NOT NULL DEFAULT true,
    pass_threshold      smallint NOT NULL DEFAULT 70,
    behavioral_bank     jsonb NOT NULL,
    created_at          timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE interview_sessions (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id      uuid NOT NULL REFERENCES accounts(id),
    config_id       uuid REFERENCES interview_configs(id) ON DELETE SET NULL,
    candidate_name  text NOT NULL,
    candidate_email citext,
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
    audio_object_key text,
    submitted_at     timestamptz NOT NULL DEFAULT now()
);

-- Append-only utterance log, hash-chained for tamper-evidence. The chain is
-- populated in Phase 4/5; the table is created now so app DB grants can be
-- locked down to INSERT/SELECT only on day one.
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
    pdf_object_key text,
    model          text NOT NULL,
    prompt_version text NOT NULL,
    generated_at   timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE event_outbox (
    id            bigserial PRIMARY KEY,
    topic         text NOT NULL,
    payload       jsonb NOT NULL,
    created_at    timestamptz NOT NULL DEFAULT now(),
    dispatched_at timestamptz
);
CREATE INDEX idx_outbox_pending ON event_outbox(id) WHERE dispatched_at IS NULL;

CREATE TABLE shortlinks (
    code       text PRIMARY KEY,
    session_id uuid NOT NULL REFERENCES interview_sessions(id) ON DELETE CASCADE,
    hit_count  integer NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE audit_log (
    id         bigserial PRIMARY KEY,
    actor_id   uuid,
    session_id uuid,
    action     text NOT NULL,
    metadata   jsonb,
    at         timestamptz NOT NULL DEFAULT now()
);
