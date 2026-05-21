import { ENV, envNumber } from '../app/env.js';
import {
  getAccessToken,
  setSession,
  clearSession,
  registerRefresh,
} from '../app/auth-store.js';
import { parseNonTechBank } from '../interview/non-tech-bank.js';
import { serializeBank } from '../interview/bank-editor.js';

/** @typedef {import('./types.js').LoginResponse} LoginResponse */
/** @typedef {import('./types.js').RefreshResponse} RefreshResponse */
/** @typedef {import('./types.js').InviteCreateRequest} InviteCreateRequest */
/** @typedef {import('./types.js').InviteCreateResponse} InviteCreateResponse */
/** @typedef {import('./types.js').InviteListResponse} InviteListResponse */
/** @typedef {import('./types.js').AcceptInviteRequest} AcceptInviteRequest */
/** @typedef {import('./types.js').TranscriptExport} TranscriptExport */

export function baseUrl() {
  return (ENV.APP_API_BASE_URL || '').replace(/\/$/, '');
}

export class ApiError extends Error {
  constructor(status, message, body, retryAfter) {
    super(message);
    this.status = status;
    this.body = body;
    this.retryAfter = retryAfter;
  }
}

/** @param {unknown} resp */
function normalizeLoginResponse(resp) {
  if (!resp || typeof resp !== 'object') {
    throw new ApiError(0, 'Invalid login response from API', resp);
  }

  const body = /** @type {Record<string, unknown>} */ (resp);
  const expiresIn = Number(body.expires_in);
  if (
    typeof body.access_token !== 'string'
    || !Number.isFinite(expiresIn)
    || typeof body.account_id !== 'string'
    || typeof body.role !== 'string'
  ) {
    throw new ApiError(0, 'Invalid login response from API', resp);
  }

  /** @type {LoginResponse} */
  const normalized = {
    access_token: body.access_token,
    expires_in: expiresIn,
    account_id: body.account_id,
    role: /** @type {import('./types.js').Role} */ (body.role),
    display_name: typeof body.display_name === 'string' ? body.display_name : null,
  };
  return normalized;
}

async function parseError(res) {
  let body = null;
  let detail = '';
  try {
    body = await res.json();
    detail = body?.error?.message || JSON.stringify(body);
  } catch {
    detail = await res.text().catch(() => '');
  }
  const retryHeader = res.headers.get('Retry-After');
  const retryAfter = retryHeader ? Number(retryHeader) : undefined;
  return new ApiError(res.status, `API ${res.status}: ${detail || res.statusText}`, body, retryAfter);
}

/// Central fetch wrapper. Adds Authorization, handles 401-refresh-retry, surfaces 429.
/// `auth: 'required'` (default for protected calls) — attach bearer, retry on 401.
/// `auth: 'optional'` — attach bearer if present, do not retry on 401.
/// `auth: 'cookie'` — `credentials: 'include'`, no bearer (for /v1/auth/*).
async function apiFetch(path, { method = 'GET', body, headers = {}, auth = 'optional', _retried = false } = {}) {
  const url = `${baseUrl()}${path}`;
  const init = { method, headers: { ...headers } };
  if (body !== undefined) {
    init.headers['Content-Type'] = init.headers['Content-Type'] || 'application/json';
    init.body = typeof body === 'string' ? body : JSON.stringify(body);
  }
  if (auth === 'cookie') {
    init.credentials = 'include';
  } else {
    const token = getAccessToken();
    if (token) init.headers['Authorization'] = `Bearer ${token}`;
  }
  let res;
  try {
    res = await fetch(url, init);
  } catch (e) {
    const origin = typeof window !== 'undefined' ? window.location.origin : 'this frontend origin';
    throw new ApiError(
      0,
      `Could not reach API at ${baseUrl()}. Check that the backend is running and CORS_ORIGINS allows ${origin}.`,
      { cause: e?.message || String(e) },
    );
  }
  if (res.status === 401 && auth === 'required' && !_retried) {
    try {
      await refresh();
    } catch (e) {
      clearSession();
      throw await parseError(res);
    }
    return apiFetch(path, { method, body, headers, auth, _retried: true });
  }
  if (!res.ok) throw await parseError(res);
  if (res.status === 204) return null;
  const ctype = res.headers.get('content-type') || '';
  if (ctype.includes('application/json')) return res.json();
  return res.text();
}

// ─── Auth ────────────────────────────────────────────────────────────────────

/** @returns {Promise<LoginResponse>} */
export async function login(email, password) {
  /** @type {LoginResponse} */
  const resp = normalizeLoginResponse(await apiFetch('/v1/auth/login', {
    method: 'POST',
    body: {
      email: String(email).trim().toLowerCase(),
      password: String(password),
    },
    auth: 'cookie',
  }));
  setSession(resp);
  return resp;
}

/** @returns {Promise<RefreshResponse>} */
export async function refresh() {
  /** @type {RefreshResponse} */
  const resp = normalizeLoginResponse(await apiFetch('/v1/auth/refresh', {
    method: 'POST',
    auth: 'cookie',
  }));
  setSession(resp);
  return resp;
}

export async function logout() {
  try {
    await apiFetch('/v1/auth/logout', { method: 'POST', auth: 'cookie' });
  } finally {
    clearSession();
  }
}

/** @param {AcceptInviteRequest} req */
export async function acceptInvite(req) {
  return apiFetch('/v1/auth/accept-invite', {
    method: 'POST',
    body: req,
    auth: 'cookie',
  });
}

// ─── Admin ───────────────────────────────────────────────────────────────────

/** @param {InviteCreateRequest} req @returns {Promise<InviteCreateResponse>} */
export async function createInvite(req) {
  return apiFetch('/v1/admin/invites', {
    method: 'POST',
    body: req,
    auth: 'required',
  });
}

/** @param {string} [status] @returns {Promise<InviteListResponse>} */
export async function listInvites(status = 'all') {
  const qs = status && status !== 'all' ? `?status=${encodeURIComponent(status)}` : '';
  return apiFetch(`/v1/admin/invites${qs}`, { auth: 'required' });
}

export async function revokeInvite(inviteId) {
  return apiFetch(`/v1/admin/invites/${encodeURIComponent(inviteId)}/revoke`, {
    method: 'POST',
    auth: 'required',
  });
}

export async function undoRevokeInvite(inviteId) {
  return apiFetch(`/v1/admin/invites/${encodeURIComponent(inviteId)}/undo-revoke`, {
    method: 'POST',
    auth: 'required',
  });
}

/** @returns {Promise<InviteCreateResponse>} */
export async function resendInvite(inviteId) {
  return apiFetch(`/v1/admin/invites/${encodeURIComponent(inviteId)}/resend`, {
    method: 'POST',
    auth: 'required',
  });
}

export async function disableAccount(accountId) {
  return apiFetch(`/v1/admin/accounts/${encodeURIComponent(accountId)}/disable`, {
    method: 'POST',
    auth: 'required',
  });
}

// ─── Interviews ──────────────────────────────────────────────────────────────

export async function createInterview(payload) {
  return apiFetch('/v1/interviews', {
    method: 'POST',
    body: payload,
    auth: 'required',
  });
}

// ─── HR config ───────────────────────────────────────────────────────────────

/// Translate the backend HR config (snake_case, structured behavioral_bank)
/// into the camelCase shape the app provider/components use. The behavioral
/// bank is kept as `## Section` textarea text on the frontend.
function fromApiConfig(resp) {
  const c = (resp && resp.config) || {};
  return {
    config: {
      hrEmail: c.hr_email || '',
      techCount: Number(c.tech_count) || 0,
      nonTechCount: Number(c.behavioral_count) || 0,
      nonTechBank: serializeBank(c.behavioral_bank || {}),
      passThreshold: Number(c.pass_threshold) || 0,
      voiceName: c.voice_name || '',
    },
    saved: Boolean(resp && resp.saved),
  };
}

/** @param {object} config camelCase provider config */
function toApiConfig(config) {
  return {
    hr_email: String(config.hrEmail || '').trim(),
    tech_count: Number(config.techCount) || 0,
    behavioral_count: Number(config.nonTechCount) || 0,
    behavioral_bank: parseNonTechBank(config.nonTechBank || ''),
    pass_threshold: Number(config.passThreshold) || 0,
    voice_name: config.voiceName || '',
  };
}

/// Load the current account's HR config. Returns `{ config, saved }` —
/// `saved` is false when the account is receiving backend defaults.
export async function getHrConfig() {
  return fromApiConfig(await apiFetch('/v1/me/config', { auth: 'required' }));
}

/// Persist the current account's HR config. Returns the saved, normalized
/// config from the backend as `{ config, saved }`.
export async function updateHrConfig(config) {
  return fromApiConfig(await apiFetch('/v1/me/config', {
    method: 'PUT',
    body: toApiConfig(config),
    auth: 'required',
  }));
}

/// Public state lookup. Returns `{id, state, candidate_name, role_title, expires_at, shortcode}`.
export async function fetchSessionByCode(code) {
  return apiFetch(`/v1/sessions/by-code/${encodeURIComponent(code)}`, { auth: 'optional' });
}

/// Poll the session-by-code endpoint until priming flips state from `pending`.
export async function waitForPrimed(
  code,
  {
    onTick,
    intervalMs = envNumber('SESSION_PRIMING_POLL_INTERVAL_MS'),
    timeoutMs = envNumber('SESSION_PRIMING_TIMEOUT_MS'),
  } = {},
) {
  const started = Date.now();
  let attempt = 0;
  while (true) {
    attempt += 1;
    const session = await fetchSessionByCode(code);
    if (onTick) onTick({ state: session.state, attempt });
    if (session.state && session.state !== 'pending') return session;
    if (Date.now() - started > timeoutMs) {
      throw new Error(`Timed out waiting for session to be ready (state=${session.state})`);
    }
    await new Promise(r => setTimeout(r, intervalMs));
  }
}

/// Single-use join nonce for `WSS /v1/ws/interview?token=…`.
export async function issueJoinNonce(code) {
  return apiFetch(`/v1/sessions/by-code/${encodeURIComponent(code)}/join`, { auth: 'optional' });
}

/// Report a failed pre-interview system check so the backend can count
/// system-incompatibility attempts and expire the link once the limit is hit.
/// Returns `{ incompat_count, limit, attempts_remaining, expired }`.
export async function reportSystemIncompatible(code, { check, detail }) {
  return apiFetch(`/v1/sessions/by-code/${encodeURIComponent(code)}/incompatible`, {
    method: 'POST',
    body: { check, detail },
    auth: 'optional',
  });
}

/// Roll the in-interview per-question grades into a final report.
export async function finalizeInterview(sessionId) {
  return apiFetch(`/v1/interviews/${encodeURIComponent(sessionId)}/finalize`, {
    method: 'POST',
    auth: 'optional',
  });
}

/** @returns {Promise<TranscriptExport>} */
export async function getTranscript(sessionId) {
  return apiFetch(`/v1/interviews/${encodeURIComponent(sessionId)}/transcript`, { auth: 'optional' });
}

export function reportPdfUrl(sessionId) {
  return `${baseUrl()}/v1/interviews/${encodeURIComponent(sessionId)}/report.pdf`;
}

// Wire the auth store's silent-refresh hook (avoids circular import).
registerRefresh(refresh);
