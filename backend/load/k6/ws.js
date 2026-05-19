// Phase 6 WS scale test: 1000 concurrent realtime sessions.
//
// Pre-requirement: a fleet of `interview_sessions` already in state=`primed`
// with shortcodes listed in CODES (newline-separated, one per VU). Generate
// with a small bash loop that hits POST /v1/interviews + waits for priming.
//
//   k6 run ws.js \
//     --env BASE=http://localhost:8080 \
//     --env CODES=$(jq -Rsr 'split("\n")[:-1] | @csv' codes.txt)

import ws from 'k6/ws';
import http from 'k6/http';
import { check } from 'k6';

export const options = {
  scenarios: {
    ws_soak: {
      executor: 'constant-vus',
      vus: 1000,
      duration: '10m',
    },
  },
  thresholds: {
    ws_session_duration: ['p(95)>30000'],
    // Phase 6 §22 SLO: p50 WS frame handle < 50ms — observed server-side, here
    // we just track session length to make sure they don't churn.
  },
};

const BASE = __ENV.BASE || 'http://localhost:8080';
const CODES = (__ENV.CODES || '').split(',').map((s) => s.replace(/"/g, '').trim()).filter(Boolean);

export default function () {
  if (CODES.length === 0) {
    throw new Error('Set CODES env var to a comma-separated list of shortcodes.');
  }
  const code = CODES[(__VU - 1) % CODES.length];
  const joinRes = http.get(`${BASE}/v1/sessions/by-code/${code}/join`);
  check(joinRes, { 'join 200': (r) => r.status === 200 });
  if (joinRes.status !== 200) return;

  const join = joinRes.json();
  const wsUrl = `${BASE.replace(/^http/, 'ws')}${join.ws_path}?token=${join.join_nonce}`;

  ws.connect(wsUrl, {}, function (socket) {
    socket.on('open', () => {
      socket.send(JSON.stringify({ t: 'hello', client_version: 'k6-1.0' }));
    });
    socket.on('message', (raw) => {
      const msg = JSON.parse(raw);
      if (msg.t === 'question' || msg.t === 'followup') {
        // Send a tiny final utterance + submit, simulating a brisk candidate.
        socket.send(
          JSON.stringify({
            t: 'utterance',
            seq: 1,
            ordinal: msg.ordinal,
            text: 'k6 synthetic answer',
            is_final: true,
          }),
        );
        socket.send(JSON.stringify({ t: 'submit', ordinal: msg.ordinal, duration_ms: 500 }));
      }
      if (msg.t === 'results_ready') socket.close();
    });
    socket.setTimeout(() => socket.close(), 60_000);
  });
}
