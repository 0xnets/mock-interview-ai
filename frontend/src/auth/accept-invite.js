import { acceptInvite } from '../api/client.js';
import { showScreen } from '../ui/screens.js';

/// Read `?invite=<token>` (or `?token=<token>`) from the URL.
export function readInviteTokenFromUrl() {
  const sp = new URLSearchParams(window.location.search);
  return sp.get('invite') || sp.get('token') || '';
}

export function openAcceptInviteScreen() {
  showScreen('accept-invite');
  const urlToken = readInviteTokenFromUrl();
  const tokenInput = document.getElementById('acceptInviteToken');
  if (tokenInput) {
    if (urlToken) {
      tokenInput.value = urlToken;
      tokenInput.readOnly = true;
    } else {
      tokenInput.readOnly = false;
    }
  }
}

export async function submitAcceptInvite() {
  const tokenInput = document.getElementById('acceptInviteToken');
  const token = readInviteTokenFromUrl() || (tokenInput ? tokenInput.value.trim() : '');
  const pwEl = document.getElementById('acceptPassword');
  const cfEl = document.getElementById('acceptPasswordConfirm');
  const err = document.getElementById('acceptInviteError');
  const ok = document.getElementById('acceptInviteSuccess');
  if (err) err.classList.add('hidden');
  if (ok) ok.classList.add('hidden');

  const password = pwEl ? pwEl.value : '';
  const confirm = cfEl ? cfEl.value : '';
  if (!token) {
    if (err) { err.textContent = 'Enter the invite token from your email.'; err.classList.remove('hidden'); }
    return;
  }
  if (password.length < 12) {
    if (err) { err.textContent = 'Password must be at least 12 characters.'; err.classList.remove('hidden'); }
    return;
  }
  if (password !== confirm) {
    if (err) { err.textContent = 'Passwords do not match.'; err.classList.remove('hidden'); }
    return;
  }

  const btn = document.getElementById('acceptInviteBtn');
  if (btn) btn.disabled = true;
  try {
    const resp = await acceptInvite({ token, password });
    if (ok) ok.classList.remove('hidden');
    if (pwEl) pwEl.value = '';
    if (cfEl) cfEl.value = '';
    const loginEmail = document.getElementById('loginEmail');
    if (loginEmail && resp && resp.email) loginEmail.value = resp.email;
    setTimeout(() => showScreen('login'), 1200);
  } catch (e) {
    if (err) {
      err.textContent = e.message || 'Failed to accept invite.';
      err.classList.remove('hidden');
    }
  } finally {
    if (btn) btn.disabled = false;
  }
}
