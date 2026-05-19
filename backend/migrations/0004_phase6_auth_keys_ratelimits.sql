-- Phase 6: real auth, key rotation history, rate-limit overrides.
--
--   * refresh_tokens         — server-side store for rotating refresh tokens
--   * key_versions           — public-key catalog for transcript signing rotation
--   * rate_limit_overrides   — per-account/route overrides for the limiter
--   * accounts.role          — broaden to include 'super_admin'
--   * audit_log.event_id     — request-correlation id stamped on every entry
--   * audit_log indexes      — speed up the recent-events lookups Phase 6 adds

-- accounts.role check broadened. CHECK constraints can't be edited in place;
-- drop and re-add. The 0002 seed only uses 'hr' so no rows need migrating.
ALTER TABLE accounts DROP CONSTRAINT IF EXISTS accounts_role_check;
ALTER TABLE accounts
    ADD CONSTRAINT accounts_role_check
    CHECK (role IN ('hr', 'admin', 'super_admin'));

-- Rotating refresh tokens. The cookie holds the raw token, we keep a sha256
-- so a leaked DB row can't be replayed. last_used_at lets us prune idle ones.
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

-- Catalog of every signing key ever used for transcript checkpoints. The
-- transcript_checkpoints table stores `key_id`; this gives the verifier the
-- corresponding public bytes without trusting an externally supplied value.
CREATE TABLE key_versions (
    key_id      text PRIMARY KEY,
    public_key  bytea NOT NULL,
    status      text NOT NULL CHECK (status IN ('active', 'retired', 'compromised')),
    created_at  timestamptz NOT NULL DEFAULT now(),
    retired_at  timestamptz,
    notes       text
);

-- Seed the dev key catalog so the verifier can pick it up via the API.
-- Public bytes derive from the deterministic dev seed in api/src/signing.rs;
-- a real deploy with KMS replaces this row via /v1/admin/keys/rotate.
INSERT INTO key_versions (key_id, public_key, status, notes)
VALUES (
    'dev-v1',
    '\x00'::bytea,  -- placeholder; api refreshes on startup from the active signer
    'active',
    'Phase 5 dev key. Replaced by KMS in Phase 6 prod.'
) ON CONFLICT (key_id) DO NOTHING;

-- Per-account / per-route overrides for the rate limiter. NULL account_id is
-- a global override; otherwise per-account. NULL route means all routes.
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

-- Request-correlation id stamped on every audit_log row so a single HTTP
-- request (and its async fan-out) can be reconstructed end-to-end.
ALTER TABLE audit_log
    ADD COLUMN event_id   uuid;
CREATE INDEX idx_audit_event ON audit_log(event_id) WHERE event_id IS NOT NULL;
CREATE INDEX idx_audit_session_at ON audit_log(session_id, at DESC);
CREATE INDEX idx_audit_actor_at ON audit_log(actor_id, at DESC);
