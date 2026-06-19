# Frontend — AI Mock Interview Platform

The browser client for the AI mock interview platform: a React + Vite single
page app. HR uses it to save interview configuration, generate candidate
links, manage team invites, and review results; candidates use it to complete
browser checks, run a voice interview, and view the transcript/report flow.
It talks to the Rust backend over REST and one WebSocket channel.

## Architecture overview

- **React app.** `src/main.jsx` mounts the app with React 19 and
  `react-router-dom`. StrictMode is intentionally disabled because the
  interview WebSocket join nonce is single-use and StrictMode would consume it
  during the discarded development mount.
- **Route-based screens.** `src/App.jsx` owns the route table and guards:
  `/login`, `/accept-invite`, `/setup`, `/invites`, `/welcome`,
  `/interview`, `/results`, and `/transcript`.
- **Provider state.** Auth/session state, interview state, and toast messages
  live in React providers under `src/providers/`.
- **In-memory auth.** The JWT access token lives in memory only; the refresh
  token is an HttpOnly cookie the browser sends automatically.
- **Candidate links.** Legacy query-param links are still accepted. `Boot`
  routes `?invite=...` to invite acceptance and `?session=...` to the
  candidate flow.

### Directory layout (`src/`)

| Folder        | Responsibility                                                            |
|---------------|---------------------------------------------------------------------------|
| `api/`        | REST layer — `client.js` (`apiFetch()`), response helpers, DTO docs        |
| `app/`        | Env parsing and in-memory auth token store                                 |
| `candidate/`  | Candidate session loading and short-code handling                          |
| `components/` | Shared React UI: shell, tabs, HR config, link generation, PDF textarea     |
| `hooks/`      | React hooks for speech recognition and browser voices                      |
| `interview/`  | Behavioral bank parsing/editing and default non-technical questions        |
| `providers/`  | Auth, app state, and toast providers                                       |
| `realtime/`   | Live interview WebSocket client                                            |
| `reports/`    | Post-interview report polling                                              |
| `screens/`    | Route-level React screens                                                  |
| `services/`   | Resume/JD PDF text extraction with PDF.js                                  |
| `system/`     | Candidate pre-interview browser, mic, speaker, network, and voice checks   |
| `transcript/` | Transcript viewer helpers                                                  |
| `utils/`      | Clipboard and validation helpers                                           |
| `voice/`      | Speech synthesis and voice selection                                       |
| `styles/`     | CSS bundles loaded by `main.jsx`                                           |

## How it works

- **Boot.** `/` runs the `Boot` dispatcher in `App.jsx`. It handles invite
  deep links, candidate session links, and silent HR refresh before routing to
  the correct first screen.
- **API layer.** `api/client.js` attaches the access token, includes
  credentials for the refresh cookie, and surfaces API failures as `ApiError`.
  `app/auth-store.js` schedules silent refresh before access-token expiry.
- **HR config.** `/setup` saves account-scoped HR configuration through
  `GET/PUT /v1/me/config`: report recipient, question counts, pass threshold,
  interviewer voice, and the behavioral bank. The bank editor supports
  sectioned `## Section` text with limits mirrored from backend validation.
- **Candidate generation.** HR pastes or uploads JD/resume PDFs in the
  generate-link tab. PDF.js extracts text into editable textareas; the backend
  still owns question generation, grading, report rendering, and email.
- **Pre-interview checks.** Before joining, candidates run checks for browser
  support, microphone input level, speaker output, backend reachability, and
  voice synthesis. Device/browser failures are reported to the backend without
  consuming the candidate link.
- **Realtime interview.** `realtime/ws-client.js` opens a WebSocket to
  `/v1/ws/interview` using a single-use join nonce. The backend streams
  questions/follow-ups; the client streams candidate answers as transcripts.
- **Voice.** Browser Web Speech APIs provide speech recognition for answers
  and speech synthesis for interviewer prompts. Chrome or Edge is recommended.

## Local setup

**Prerequisites:** Node.js >= 18, npm, and Chrome or Edge. Safari and Firefox
do not reliably expose the required Web Speech APIs.

```bash
# 1. Enter the frontend folder
cd frontend

# 2. Create your env file; override APP_API_BASE_URL if the backend
#    is not on http://localhost:8080. ANSWER_TIME_LIMIT_MS controls
#    the per-question answer timer; the default is 180000 (3 minutes).
cp example.env .env

# 3. Install dependencies
npm install

# 4. Start the dev server
npm run dev
```

Open `http://localhost:4173` in Chrome or Edge.

> The backend must be running first — see [`../backend/README.md`](../backend/README.md).
> The SPA is non-functional without it.

### Other commands

| Command           | What it does                                          |
|-------------------|-------------------------------------------------------|
| `npm run build`   | Production bundle into `dist/`                        |
| `npm run preview` | Serve the built `dist/` bundle                        |
| `npm run check`   | `node --check` on `.js` files + import resolution     |

### Troubleshooting

- **An env var is ignored** — its prefix is not registered in
  `vite.config.js` → `envPrefix`.
- **CORS errors in the console** — add the dev origin to the backend's
  `CORS_ORIGINS`.
- **No microphone / no voice** — use Chrome or Edge, served over
  `localhost` or HTTPS, and allow microphone access.
- **Candidate check says no input** — speak during the 5-second mic-level
  check and confirm the browser is using the intended input device.
- **Invite link opens the token form** — paste the invite token manually; the
  screen accepts `invite`, `accept_invite`, and `token` query params.

## Running the full stack locally

1. Start backend dependencies and the API (see `backend/README.md`).
2. `npm run dev` here.
3. Open `http://localhost:4173`, sign in with the bootstrapped admin
   account, configure HR settings, create invites or candidate links, and run
   an interview.
