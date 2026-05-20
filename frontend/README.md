# Frontend — AI Mock Interview Platform

The browser client for the AI mock interview platform: a framework-free
Vite single-page app. HR uses it to configure interviews and generate
candidate links; candidates use it to run a voice interview; everyone uses
it to review graded transcripts. It talks to the Rust backend over REST and
one WebSocket channel.

## Architecture overview

- **No framework.** Plain ES modules bundled by Vite. There is no React/Vue
  runtime — DOM updates are done directly.
- **Single HTML document.** `index.html` contains the markup for every
  screen. `src/main.js` is the entry point: it wires global event handlers
  and runs `bootstrap()` on `window load`.
- **Screen-based navigation.** `ui/screens.js` (`showScreen()`) shows one
  screen at a time by toggling visibility — there is no router.
- **In-memory auth.** The JWT access token lives in memory only; the refresh
  token is an HttpOnly cookie the browser sends automatically.

### Directory layout (`src/`)

| Folder        | Responsibility                                                            |
|---------------|---------------------------------------------------------------------------|
| `app/`        | App lifecycle — `bootstrap.js`, `events.js`, `state.js`, `config.js`, `env.js`, `auth-store.js` |
| `api/`        | REST layer — `client.js` (`apiFetch()`), `types.js`                       |
| `auth/`       | Login + invite acceptance — `auth-flow.js`, `accept-invite.js`            |
| `admin/`      | Admin invite management — `admin-invites.js`                              |
| `candidate/`  | Candidate link generation + session encoding — `candidate-link.js`, `session-codec.js` |
| `interview/`  | Interview run + question bank — `interview-flow.js`, `candidate-mode.js`, `non-tech-bank.js` |
| `realtime/`   | Live interview WebSocket client — `ws-client.js`                          |
| `reports/`    | Post-interview report polling — `report-waiting.js`                       |
| `services/`   | Resume PDF parsing — `pdf-parser.service.js`                              |
| `transcript/` | Transcript viewer — `transcript-viewer.js`                                |
| `ui/`         | Rendering helpers — `screens.js`, `render-results.js`, `tabs.js`          |
| `utils/`      | Small helpers — `html.js`                                                 |
| `voice/`      | Web Speech wrappers — `speech-recognition.js`, `speech-synthesis.js`, `voice-selection.js` |
| `styles/`     | CSS — `base.css`, `app.css`, `components.css`, `screens.css`, `animations.css` |

## How it works

- **Bootstrap.** `main.js` calls `attachEventHandlers()` and registers
  `bootstrap()` for `window load`. `bootstrap()` loads config, restores any
  session, and shows the appropriate first screen.
- **API layer.** `api/client.js` exposes `apiFetch()`, which attaches the
  access token, sends credentials (for the refresh cookie), and surfaces
  `ApiError`. `app/auth-store.js` keeps the access token in memory and
  schedules a silent refresh before it expires.
- **Realtime interview.** `realtime/ws-client.js` opens a single WebSocket
  to `/v1/ws/interview`. The backend streams questions/follow-ups; the
  client streams the candidate's answers as they are transcribed.
- **Voice.** `voice/` wraps the browser **Web Speech API** — speech
  recognition (STT) for answers and speech synthesis (TTS) for questions.
- **Resume parsing.** `services/pdf-parser.service.js` uses PDF.js to
  extract text from an uploaded resume **only**. The graded report PDF is
  rendered server-side and downloaded from the backend.
- **Config.** Settings come from `.env`. Vite only exposes variables whose
  prefix is listed in `vite.config.js` (`APP_`, `SPEECH_`, `VOICE_`, …).

## Local setup

**Prerequisites:** Node.js >= 18, npm, and Chrome or Edge — the Web Speech
API is unreliable in Safari and Firefox.

```bash
# 1. Enter the frontend folder
cd frontend

# 2. Create your env file; override APP_API_BASE_URL if the backend
#    is not on http://localhost:8080
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
| `npm run check`   | `node --check` on every `.js` + import resolution     |

### Troubleshooting

- **An env var is ignored** — its prefix is not registered in
  `vite.config.js` → `envPrefix`.
- **CORS errors in the console** — add the dev origin to the backend's
  `CORS_ORIGINS`.
- **No microphone / no voice** — use Chrome or Edge, served over
  `localhost` (Web Speech requires a secure context).

## Running the full stack locally

1. Start backend dependencies and the API (see `backend/README.md`).
2. `npm run dev` here.
3. Open `http://localhost:4173`, sign in with the bootstrapped admin
   account, create invites, and run an interview.
