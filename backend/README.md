# Backend — AI Mock Interview Platform

The server side of the AI mock interview platform: a Rust + Axum HTTP/WebSocket
API. It generates tailored interview questions from a job description and
resume, runs the live voice interview, grades answers with Anthropic models,
builds a signed transcript, renders a PDF report, and emails it to HR.

## Architecture overview

A Cargo workspace of five crates:

| Crate               | Responsibility                                              |
|---------------------|-------------------------------------------------------------|
| `domain`            | Shared domain types — no I/O                                |
| `persistence`       | Postgres access via sqlx — repositories + schema migrations |
| `ai`                | Anthropic Messages API provider + prompt templates          |
| `api`               | The service binary — HTTP + WebSocket, workers, realtime    |
| `transcript-verify` | Offline CLI that re-verifies a transcript hash chain        |

### `api` crate layout (MVC)

| Module            | Responsibility                                                        |
|-------------------|-----------------------------------------------------------------------|
| `app/`            | Composition — `router.rs` route table, app state, health endpoints    |
| `config/`         | Typed settings, env loading, defaults, and validation                 |
| `controllers/`    | HTTP handlers — auth, config, interviews, sessions, reports, transcripts, shortlink |
| `models/`         | Request/response DTOs                                                 |
| `services/`       | Business logic — auth session handling and scoring                    |
| `infrastructure/` | Cross-cutting I/O — PDF, signing, metrics, observability, Redis backplane, streams |
| `realtime/`       | Live interview — `SessionActor`, WS handler, follow-ups, grading, nonce, locks |
| `workers/`        | Background loops — priming, outbox, mailer                            |
| `auth.rs`         | JWT extraction + `require_hr` / `require_admin` middleware             |
| `rate_limit.rs`   | Per-route rate limiting middleware                                    |

### Route table (`app/router.rs`)

| Group       | Route                                          | Notes                                |
|-------------|------------------------------------------------|--------------------------------------|
| Health      | `GET /healthz` `/livez` `/readyz`              | Liveness / readiness                 |
| Public      | `GET /s/{code}`                                | Shortlink -> frontend session link   |
| Public      | `GET /v1/sessions/by-code/{code}`              | Resolve candidate session state      |
| Public      | `GET\|POST /v1/sessions/by-code/{code}/join`   | Consume link and issue WS join nonce |
| Public      | `POST /v1/sessions/by-code/{code}/incompatible`| Track failed system checks           |
| Public      | `GET /v1/ws/interview`                         | Live interview WebSocket             |
| Public      | `POST /v1/interviews/{id}/finalize`            | Finalize + grade                     |
| Public      | `GET /v1/interviews/{id}/transcript`           | Signed transcript                    |
| Public      | `GET /v1/interviews/{id}/report.pdf`           | Server-rendered PDF report           |
| Auth        | `POST /v1/auth/login`                          | Password login                       |
| Auth        | `POST /v1/auth/refresh`                        | Refresh-cookie rotation              |
| Auth        | `POST /v1/auth/logout`                         | Revoke refresh token                 |
| Auth        | `POST /v1/auth/accept-invite`                  | Complete invite onboarding           |
| HR (JWT)    | `POST /v1/interviews`                          | Create candidate session             |
| HR (JWT)    | `GET\|PUT /v1/me/config`                       | Account-scoped HR config             |
| Admin (JWT) | `GET\|POST /v1/admin/invites`                  | List/create invites                  |
| Admin (JWT) | `POST /v1/admin/invites/{id}/revoke`           | Revoke invite                        |
| Admin (JWT) | `POST /v1/admin/invites/{id}/undo-revoke`      | Undo invite revocation               |
| Admin (JWT) | `POST /v1/admin/invites/{id}/resend`           | Issue a fresh invite link            |
| Admin (JWT) | `POST /v1/admin/accounts/{id}/disable`         | Disable account                      |

All routes pass through rate-limiting, HTTP tracing, and CORS layers.

## How it works

**Interview lifecycle:**

1. **Configure** — HR saves account-scoped setup in `accounts.hr_config` via
   `GET/PUT /v1/me/config`: recipient email, question counts, behavioral bank,
   pass threshold, and selected voice.
2. **Create** — `POST /v1/interviews` stores the job description, resume,
   config snapshot, and candidate session with a shortcode.
3. **Prime** — the `priming` worker generates technical and behavioral
   questions asynchronously with Anthropic; the frontend polls until ready.
4. **Check** — candidates run browser/device checks before joining. Failed
   system checks call `/v1/sessions/by-code/{code}/incompatible`; after
   `SHORTLINK_INCOMPAT_LIMIT` failures the session is expired so HR can send a
   fresh link. These reports do not consume the one-use join link.
5. **Interview** — the candidate calls `/join` to consume the shortcode and
   get a single-use WebSocket nonce, then connects to `/v1/ws/interview`.
   `SessionActor` serves questions, generates AI follow-ups on technical
   answers, records transcript chunks, and grades each answer as it lands.
6. **Finalize** — `POST /v1/interviews/{id}/finalize` builds the report and
   computes the final score.
7. **Report** — the report PDF is rendered server-side with `printpdf` and
   served from `GET /v1/interviews/{id}/report.pdf`. An outbox event is
   enqueued; the `mailer` worker emails HR a link or attachment.

> No object storage is involved — the PDF is generated on demand and streamed
> directly from the API.

**Cross-cutting concerns:**

- **Auth** — JWT access tokens, rotating refresh tokens in HttpOnly cookies,
  Argon2id password hashing, invite-based onboarding, invite revocation,
  resend, and account disable.
- **HR config** — stored per account as JSONB on `accounts.hr_config`, with
  server-side validation for email, counts, pass threshold, and behavioral
  bank shape.
- **Transcript integrity** — an Ed25519 hash chain with periodic signed
  checkpoints; `transcript-verify` re-checks it offline.
- **Candidate safety checks** — shortlinks track incompatibility attempts in
  Postgres so repeated device/browser failures can expire the session without
  consuming the join link.
- **Rate limiting** — per-route limits (off by default in dev).
- **Redis backplane** — optional. When `REDIS_URL` is unset, the outbox,
  nonce store, and WS locks fall back to in-process implementations.
- **Observability** — Prometheus `/metrics` on a separate listener; OTLP
  tracing when `FEATURE_OTEL` is on.

## Local setup

**Prerequisites:** the Rust toolchain (stable, edition 2021) with `cargo`,
and Docker with Compose.

```bash
# 1. Enter the backend folder
cd backend

# 2. Start dependencies (Postgres + optional Redis).
#    The compose file also defines optional otel-collector and prometheus.
docker compose -f infra/docker-compose.yml up -d postgres redis

# 3. Create your env file and edit it.
#    At minimum set ANTHROPIC_API_KEY and JWT_SIGNING_SECRET (>= 32 bytes).
#    Set RESEND_API_KEY only if you exercise the email path.
cp .env.backend.example .env.backend

# 4. Load the env and run the API. Migrations run automatically on boot.
set -a; source .env.backend; set +a
cargo run -p api

# 5. Verify it is up
curl http://localhost:8080/healthz
```

The API listens on `:8080`; `/metrics` listens on `METRICS_LISTEN_ADDR`.

> **Minimal local mode:** leave `REDIS_URL` unset (or the `FEATURE_REDIS_*`
> flags off) and every Redis subsystem degrades to its in-process fallback —
> single-process dev works without Redis.

### Useful commands

| Command                                     | What it does                       |
|---------------------------------------------|------------------------------------|
| `cargo check --workspace`                   | Type-check every crate             |
| `cargo test --workspace`                    | Run the test suite                 |
| `cargo run -p api`                          | Run the API server                 |
| `cargo run -p transcript-verify -- --verify`| Offline transcript hash-chain check |

## Migrations and seed data

Migrations run automatically on API boot through the persistence crate.

| Migration | Purpose |
|-----------|---------|
| `0001_init.sql` | Consolidated schema, dev HR row, local admin row, dev signing key placeholder |
| `0002_shortlink_incompat.sql` | Adds `shortlinks.incompat_count` for pre-interview system-check failures |
| `0003_invite_management.sql` | Adds stable invite IDs plus revoke/resend metadata |
| `0004_account_hr_config.sql` | Adds `accounts.hr_config` for saved HR setup |

Fresh local databases include:

| Field    | Value                  |
|----------|------------------------|
| Email    | `admin@local.test`     |
| Password | `admin-local-password` |

The fixed `default-hr@local` row remains for legacy session references.

## Running the full stack locally

1. Start dependencies and the API with the steps above.
2. Start the frontend — see [`../frontend/README.md`](../frontend/README.md)
   (`npm install`, `npm run dev`).
3. Open `http://localhost:4173`, sign in with the bootstrapped local admin
   account, save HR configuration, create invites or candidate links, and run
   an interview.
