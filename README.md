# AI Mock Interview Platform

A voice-driven mock interview platform. HR pastes a job description and the candidate's resume; the backend generates tailored technical and behavioral questions, the candidate runs through them in a browser-based voice interview, and HR receives a graded report by email.

## Repository layout

```
.
├── backend/      # Rust workspace — Axum API, Postgres, Anthropic, transcript chain, server PDF
└── frontend/    # Vite SPA — HR setup, candidate interview, transcript viewer, admin invites
```

The two halves are developed and deployed independently. The frontend talks to the backend over REST and a single WebSocket channel for the live interview.

## Tech stack

**Backend** (`backend/`)
- Rust workspace: `domain`, `persistence`, `ai`, `api`, `transcript-verify`
- Axum 0.8 + Tokio + sqlx 0.8 (Postgres) + optional Redis (rate limit / nonce / streams)
- Anthropic Messages API — Sonnet 4.6 for priming/scoring/follow-ups, Haiku 4.5 for per-question grading
- Ed25519 transcript hash chain with rotating key versions, offline `transcript-verify` CLI
- `printpdf` server-rendered PDF report uploaded to S3-compatible storage
- Outbox + worker pool for PDF render and Resend email dispatch
- JWT auth with rotating refresh tokens (HttpOnly cookie), Argon2id password hashing
- Prometheus `/metrics` on a separate listener, OTLP tracing

**Frontend** (`frontend/`)
- Vite ES module SPA
- Web Speech API (browser STT + TTS)
- PDF.js for resume upload parsing only — the report PDF is rendered server-side
- WebSocket client for the live interview channel
- JWT access token kept in memory; refresh handled silently via the HttpOnly cookie

## Local development

### 1. Start Postgres, Redis, and MinIO

```bash
cd backend
docker compose -f infra/docker-compose.yml up -d postgres redis minio
```

### 2. Run the backend

```bash
cd backend
cp .env.backend.example .env.backend
# edit .env.backend — set ANTHROPIC_API_KEY, JWT_SIGNING_SECRET, S3 creds, etc.
set -a; source .env.backend; set +a
cargo run -p api
```

The API listens on `http://localhost:8080`; `/metrics` listens on the address set by `METRICS_LISTEN_ADDR`. Migrations run automatically on boot.

### 3. Run the frontend

```bash
cd frontend
cp example.env .env       # override APP_API_BASE_URL if your backend is elsewhere
npm install
npm run dev
```

The dev server runs on `http://localhost:4173`. Open it in Chrome or Edge — Web Speech is unreliable in Safari/Firefox.

### 4. Sign in

The first account is bootstrapped by migration `0002_bootstrap_default_account.sql`. From there:

1. The first admin signs in at the login screen.
2. They open **Invites** and create invite links for the rest of the team.
3. Each teammate opens their invite link, sets a password, and signs in.

### 5. Run an interview

1. **HR Configuration** — set the report recipient email and behavioral question bank.
2. **Generate Candidate Link** — paste JD + resume, click Generate. The backend creates the session and primes questions in the background; the frontend polls until ready, then shows a shareable short URL.
3. The candidate opens the link → goes through the voice interview → the backend grades each answer live, finalizes the report, renders the PDF, uploads it to S3, and emails HR.

## Useful commands

```bash
# Backend
cd backend
cargo check --workspace
cargo test --workspace
cargo run -p api
cargo run -p transcript-verify -- --verify  # offline transcript verification

# Frontend
cd frontend
npm run check     # node --check on every .js + import resolution
npm run build     # production bundle to frontend/dist/
npm run preview   # serve frontend/dist/
```

## Phase status

- [x] Phase 1 — Remove deploy-HTML feature, kill third-party shorteners
- [x] Phase 2 — Backend scaffolding, `POST /v1/interviews`, shortcodes, candidate payload endpoint
- [x] Phase 3 — Server-side AI (Anthropic), async priming, `POST /v1/interviews/{id}/finalize`
- [x] Phase 4 — WebSocket realtime, AI follow-ups on technicals, per-question grading worker
- [x] Phase 5 — Transcript hash chain + checkpoint signing, server-rendered PDF on S3, backend mailer worker, event outbox
- [x] Phase 6 — JWT auth + invites, rate limits, audit log, Prometheus + OTLP, Redis backplane (nonce / outbox / WS locks)

## License

Private — for internal HR use.
