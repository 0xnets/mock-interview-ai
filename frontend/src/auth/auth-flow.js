import { login, logout, refresh } from '../api/client.js';
import { isAuthenticated, getPrincipal, hasRole, subscribe, clearSession } from '../app/auth-store.js';
import { setActiveAuthNavButton, showScreen } from '../ui/screens.js';

function setText(id, value) {
  const el = document.getElementById(id);
  if (el) el.textContent = value;
}

function setHidden(id, hidden) {
  const el = document.getElementById(id);
  if (el) el.classList.toggle('hidden', hidden);
}

export function renderAuthChrome() {
  const authed = isAuthenticated();
  const p = getPrincipal();
  setHidden('logoutBtn', !authed);
  setHidden('settingsBtn', !authed);
  setHidden('authPrincipal', !authed);
  setHidden('adminInvitesBtn', !authed || !hasRole(['admin', 'super_admin']));
  if (authed && p) {
    setText('authPrincipal', `${p.display_name || p.account_id} (${p.role})`);
  } else {
    setText('authPrincipal', '');
    setActiveAuthNavButton(null);
  }
}

export async function attemptLogin() {
  const email = (document.getElementById('loginEmail') || {}).value || '';
  const password = (document.getElementById('loginPassword') || {}).value || '';
  const errEl = document.getElementById('loginError');
  if (errEl) errEl.classList.add('hidden');
  if (!email || !password) {
    if (errEl) { errEl.textContent = 'Email and password are required.'; errEl.classList.remove('hidden'); }
    return;
  }
  const btn = document.getElementById('loginBtn');
  if (btn) btn.disabled = true;
  try {
    await login(email, password);
    showScreen('setup');
  } catch (e) {
    if (errEl) {
      const detail = e?.status === 429 && e.retryAfter
        ? `Too many attempts. Try again in ${e.retryAfter}s.`
        : (e?.status === 401 ? 'Invalid email or password.' : e.message);
      errEl.textContent = detail;
      errEl.classList.remove('hidden');
    }
  } finally {
    if (btn) btn.disabled = false;
  }
}

export async function attemptLogout() {
  setActiveAuthNavButton('logout');
  try {
    await logout();
  } catch {
    clearSession();
  }
  showScreen('login');
}

/// Try a silent refresh at boot — if the HttpOnly cookie is still valid the
/// user lands on the HR setup screen without typing credentials.
export async function tryResumeSession() {
  try {
    await refresh();
    return true;
  } catch {
    return false;
  }
}

export function initAuthChrome() {
  subscribe(renderAuthChrome);
  renderAuthChrome();
}
