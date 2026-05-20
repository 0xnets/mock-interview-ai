# Backend — AI Mock Interview Platform

The server side of the AI mock interview platform: a Rust + Axum HTTP/WebSocket
API. It generates tailored interview questions from a job description and
resume, runs the live voice interview, grades each answer with Anthropic
models, builds a signed transcript, renders a PDF report, and emails it to HR.

## Architecture overview

A Cargo workspace of five crates:

| Crate              | Responsibility                                                       |
|--------------------|----------------------------------------------------------------------|
| `domain`           | Shared domain types — no I/O                                         |
| `persistence`      | Postgres access via sqlx — repositories + schema migrations          |
| `ai`               | Anthropic Messages API provider + prompt templates                   |
| `api`              | The service binary — HTTP + WebSocket, workers, realtime             |
| `transcript-verify`| Offline CLI that re-verifies a transcript hash chain                 |

### `api` crate layout (MVC)

| Module            | Responsibility                                                        |
|-------------------|-----------------------------------------------------------------------|
| `app/`            | Composition — `router.rs` (route table), `state.rs`, `health.rs`      |
| `config/`         | Typed settings loaded + validated from environment variables          |
| `controllers/`    | HTTP handlers — auth, interviews, sessions, reports, transcripts, shortlink |
| `models/`         | Request/response DTOs                                                 |
| `services/`       | Business logic — `auth_session.rs`, `scoring.rs`                      |
| `infrastructure/` | Cross-cutting I/O — `pdf`, `signing`, `kms`, `metrics`, `observability`, `redis_backplane`, `streams` |
| `realtime/`       | Live interview — `SessionActor`, WS handler, follow-ups, grading, nonce, locks |
| `workers/`        | Background loops — `priming`, `outbox`, `mailer`                      |
| `auth.rs`         | JWT extraction + `require_hr` / `require_admin` middleware             |
| `rate_limit.rs`   | Per-route rate limiting middleware                                    |

### Route table (`app/router.rs`)

| Group           | Route                                          | Notes                       |
|-----------------|------------------------------------------------|-----------------------------|
| Health          | `GET /healthz` `/livez` `/readyz`              | Liveness / readiness        |
| Public          | `GET /s/{code}`                                | Shortlink → session redirect|
| Public          | `GET /v1/sessions/by-code/{code}`              | Resolve a candidate session |
| Public          | `GET\|POST /v1/sessions/by-code/{code}/join`   | Issue a WS join nonce       |
| Public          | `GET /v1/ws/interview`                         | Live interview WebSocket    |
| Public          | `POST /v1/interviews/{id}/finalize`            | Finalize + grade            |
| Public          | `GET /v1/interviews/{id}/transcript`           | Signed transcript           |
| Public          | `GET /v1/interviews/{id}/report.pdf`           | Server-rendered PDF report  |
| Auth            | `POST /v1/auth/login` `/refresh` `/logout` `/accept-invite` | Rate-limited     |
| HR (JWT)        | `POST /v1/interviews`                          | `require_hr`                |
| Admin (JWT)     | `POST /v1/admin/invites`                       | `require_admin`             |

All routes pass through rate-limiting, HTTP tracing, and CORS layers.

## How it works

**Interview lifecycle:**

1. **Create** — `POST /v1/interviews` (HR-authenticated) stores the job
   description + resume and a candidate session with a shortcode.
2. **Prime** — the `priming` worker generates technical and behavioral
   questions asynchronously with Anthropic; the frontend polls until ready.
3. **Interview** — the candidate connects to `/v1/ws/interview`. A
   `SessionActor` runs the session: it serves questions, generates AI
   follow-ups on technical answers, and grades each answer as it lands.
4. **Finalize** — `POST /v1/interviews/{id}/finalize` builds the report and
   computes the final score.
5. **Report** — the report PDF is rendered server-side with `printpdf` and
   served from `GET /v1/interviews/{id}/report.pdf`. An outbox event is
   enqueued; the `mailer` worker emails HR a link or attachment.

> No object storage (S3/MinIO) is involved — the PDF is generated on demand
> and streamed directly from the API.

**Cross-cutting concerns:**

- **Auth** — JWT access tokens, rotating refresh tokens in HttpOnly cookies,
  Argon2id password hashing, invite-based onboarding.
- **Transcript integrity** — an Ed25519 hash chain with periodic signed
  checkpoints; `transcript-verify` re-checks it offline.
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

# 2. Start dependencies (Postgres + Redis).
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

| Command                                       | What it does                          |
|------------------------------------------------|----------------------------------------|
| `cargo check --workspace`                      | Type-check every crate                 |
| `cargo test --workspace`                       | Run the test suite                     |
| `cargo run -p api`                             | Run the API server                     |
| `cargo run -p transcript-verify -- --verify`   | Offline transcript hash-chain check    |

## Running the full stack locally

1. Start dependencies and the API with the steps above.
2. Start the frontend — see [`../frontend/README.md`](../frontend/README.md)
   (`npm install`, `npm run dev`).
3. Open `http://localhost:4173`, sign in with the bootstrapped local admin
   account, create invites, and run an interview.

   | Field    | Value                  |
   |----------|------------------------|
   | Email    | `admin@local.test`     |
   | Password | `admin-local-password` |

   The local admin row is created by the consolidated migration
   `0001_init.sql`, alongside the fixed dev HR account used by legacy session
   references.
