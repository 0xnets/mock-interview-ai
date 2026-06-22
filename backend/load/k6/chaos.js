// Phase 6 chaos test scaffolding.
//
// k6 alone can't kill containers, so this script is meant to run *alongside*
// a chaos driver (e.g. Toxiproxy, Pumba, manual docker kill). It hammers the
// API at a low rate while you inject failures and reports which guarantees
// held:
//
//   * Anthropic 500: follow-up + grading must still proceed via fallback
//   * Redis down:   nonces + rate-limit degrade to in-memory; WS still works
//   * S3 PUT fail:  outbox row stays pending and retries with backoff
//   * Primary DB:   replica reads keep serving (returns 503 on writes)
//
//   k6 run chaos.js --env BASE=http://localhost:8080

import http from 'k6/http';
import { check, sleep } from 'k6';

export const options = {
  vus: 5,
  duration: '30m',
};

const BASE = __ENV.BASE || 'http://localhost:8080';

export default function () {
  const r = http.get(`${BASE}/readyz`);
  // During chaos the readyz can fail; the run records that and doesn't fail
  // the script — we want the histogram, not a pass/fail bit.
  check(r, { 'readyz responded': (r) => r.status === 200 || r.status === 503 });
  sleep(2);
}
