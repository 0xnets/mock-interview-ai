// Phase 6 24-hour soak. Mixes 100 concurrent interview creations with a
// trickle of WS sessions and a steady probe of /readyz. Designed to be killed
// gracefully and resumed.
//
//   k6 run soak.js --env BASE=http://localhost:8080 --env TOKEN=$HR_DEV_TOKEN

import http from 'k6/http';
import { check, sleep } from 'k6';

export const options = {
  scenarios: {
    create_steady: {
      executor: 'constant-vus',
      vus: 100,
      duration: '24h',
      exec: 'create',
    },
    readiness_probe: {
      executor: 'constant-arrival-rate',
      rate: 1,
      timeUnit: '5s',
      duration: '24h',
      preAllocatedVUs: 2,
      exec: 'readiness',
    },
  },
  thresholds: {
    http_req_failed: ['rate<0.01'],
    // Phase 6 §22 SLO: outbox must drain to 0 between windows. Tracked
    // server-side via outbox_pending; this guard just keeps overall health.
  },
};

const BASE = __ENV.BASE || 'http://localhost:8080';
const TOKEN = __ENV.TOKEN;

export function create() {
  const res = http.post(
    `${BASE}/v1/interviews`,
    JSON.stringify({
      candidate_name: 'Soak Test',
      role_title: 'Backend Engineer',
      jd_text: 'Soak JD',
      resume_text: 'Soak resume',
      hr_email: 'soak@example.com',
      tech_count: 3,
      behavioral_count: 1,
      behavioral_bank: { growth: ['How do you learn new tech?'] },
    }),
    {
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${TOKEN}` },
    },
  );
  check(res, { 'create 201 or 429': (r) => r.status === 201 || r.status === 429 });
  sleep(Math.random() * 30 + 30);
}

export function readiness() {
  const res = http.get(`${BASE}/readyz`);
  check(res, { 'readyz 200': (r) => r.status === 200 });
}
