// Phase 6 burst test: 10 RPS sustained for 60s against POST /v1/interviews.
// Validates the priming worker keeps up and the rate limiter holds the line.
//
//   k6 run burst.js \
//     --env BASE=http://localhost:8080 \
//     --env TOKEN=$HR_DEV_TOKEN

import http from 'k6/http';
import { check } from 'k6';

export const options = {
  scenarios: {
    burst: {
      executor: 'constant-arrival-rate',
      rate: 10,
      timeUnit: '1s',
      duration: '60s',
      preAllocatedVUs: 20,
      maxVUs: 40,
    },
  },
  thresholds: {
    'http_req_duration{endpoint:create}': ['p(50)<400', 'p(99)<1500'],
    // Phase 6 §22 SLO: p99 < 1.5s for POST /v1/interviews
    http_req_failed: ['rate<0.05'],
  },
};

const BASE = __ENV.BASE || 'http://localhost:8080';
const TOKEN = __ENV.TOKEN;

const payload = {
  candidate_name: 'Load Test',
  role_title: 'Senior Engineer',
  jd_text: 'Job description for load testing.',
  resume_text: 'Resume content for load testing.',
  hr_email: 'hr+loadtest@example.com',
  tech_count: 5,
  behavioral_count: 2,
  behavioral_bank: {
    leadership: ['Tell me about a time you led a team through a difficult migration.'],
    growth: ['How do you keep learning?'],
  },
};

export default function () {
  const res = http.post(`${BASE}/v1/interviews`, JSON.stringify(payload), {
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${TOKEN}`,
    },
    tags: { endpoint: 'create' },
  });
  check(res, {
    'create 201 or 429': (r) => r.status === 201 || r.status === 429,
    'retry-after on 429': (r) => r.status !== 429 || r.headers['Retry-After'] !== undefined,
  });
}
