// Phase 6 smoke test. Hits /healthz + /readyz only — runs in CI to catch
// total breakage. Real load is in burst.js / soak.js / ws.js.
//
//   k6 run smoke.js --env BASE=http://localhost:8080

import http from 'k6/http';
import { check, sleep } from 'k6';

export const options = {
  vus: 5,
  duration: '30s',
  thresholds: {
    http_req_failed: ['rate<0.01'],
    http_req_duration: ['p(95)<400'],
  },
};

const BASE = __ENV.BASE || 'http://localhost:8080';

export default function () {
  const live = http.get(`${BASE}/livez`);
  check(live, { 'livez 200': (r) => r.status === 200 });

  const ready = http.get(`${BASE}/readyz`);
  check(ready, { 'readyz 200': (r) => r.status === 200 });
  sleep(1);
}
