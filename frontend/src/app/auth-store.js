/// In-memory auth state. Access token is held in a module variable (not localStorage)
/// to limit XSS exfiltration; the refresh token is an HttpOnly cookie set by the backend.

/** @typedef {import('../api/types.js').Principal} Principal */
/** @typedef {import('../api/types.js').LoginResponse} LoginResponse */

let accessToken = '';
let expiresAtMs = 0;
/** @type {Principal|null} */
let principal = null;
/** @type {ReturnType<typeof setTimeout>|null} */
let refreshTimer = null;
/** @type {(() => Promise<void>)|null} */
let refreshFn = null;
/** @type {Set<() => void>} */
const listeners = new Set();

const REFRESH_LEAD_MS = 60_000; // refresh ~1 min before expiry

function notify() {
  for (const cb of listeners) {
    try { cb(); } catch (e) { console.error('auth listener error', e); }
  }
}

/// Register the function the store should call to silently refresh the access token.
/// Wired by the API client to avoid a circular import.
export function registerRefresh(fn) {
  refreshFn = fn;
}

export function subscribe(cb) {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

/** @param {LoginResponse} resp */
export function setSession(resp) {
  accessToken = resp.access_token;
  expiresAtMs = Date.now() + (resp.expires_in * 1000);
  principal = {
    account_id: resp.account_id,
    role: resp.role,
    display_name: resp.display_name,
  };
  scheduleRefresh();
  notify();
}

export function clearSession() {
  accessToken = '';
  expiresAtMs = 0;
  principal = null;
  if (refreshTimer) { clearTimeout(refreshTimer); refreshTimer = null; }
  notify();
}

export function getAccessToken() {
  return accessToken;
}

export function getPrincipal() {
  return principal;
}

export function isAuthenticated() {
  return !!accessToken && Date.now() < expiresAtMs;
}

/** @param {import('../api/types.js').Role|import('../api/types.js').Role[]} role */
export function hasRole(role) {
  if (!principal) return false;
  const roles = Array.isArray(role) ? role : [role];
  return roles.includes(principal.role);
}

function scheduleRefresh() {
  if (refreshTimer) clearTimeout(refreshTimer);
  if (!refreshFn) return;
  const delay = Math.max(1000, expiresAtMs - Date.now() - REFRESH_LEAD_MS);
  refreshTimer = setTimeout(() => {
    refreshFn && refreshFn().catch(err => {
      console.warn('Silent refresh failed', err);
      clearSession();
    });
  }, delay);
}
