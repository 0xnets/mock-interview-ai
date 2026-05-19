import { createInvite } from '../api/client.js';
import { hasRole } from '../app/auth-store.js';
import { showScreen } from '../ui/screens.js';

export function openAdminInvites() {
  if (!hasRole(['admin', 'super_admin'])) {
    alert('Admin role required.');
    return;
  }
  showScreen('admin-invite');
}

function formatExpiryHours(hours) {
  const value = Number(hours);
  if (!Number.isFinite(value)) return `${hours} hour(s)`;
  if (value >= 24 && value % 24 === 0) {
    const days = value / 24;
    return `${days} ${days === 1 ? 'day' : 'days'}`;
  }
  return `${value} ${value === 1 ? 'hour' : 'hours'}`;
}

export async function submitCreateInvite() {
  const emailEl = document.getElementById('inviteEmail');
  const nameEl = document.getElementById('inviteDisplayName');
  const roleEl = document.getElementById('inviteRole');
  const err = document.getElementById('createInviteError');
  const ok = document.getElementById('createInviteStatus');
  const box = document.getElementById('inviteResultBox');
  const urlEl = document.getElementById('inviteAcceptUrl');
  const expEl = document.getElementById('inviteExpiryNote');
  if (err) err.classList.add('hidden');
  if (ok) ok.classList.add('hidden');
  if (box) box.classList.add('hidden');

  const email = emailEl ? emailEl.value.trim() : '';
  const display_name = nameEl ? nameEl.value.trim() : '';
  const role = roleEl ? roleEl.value : 'hr';

  if (!email.includes('@')) {
    if (err) { err.textContent = 'Enter a valid email.'; err.classList.remove('hidden'); }
    return;
  }
  if (role !== 'hr' && role !== 'admin') {
    if (err) { err.textContent = 'Role must be hr or admin.'; err.classList.remove('hidden'); }
    return;
  }

  const btn = document.getElementById('createInviteBtn');
  if (btn) btn.disabled = true;
  try {
    const resp = await createInvite({ email, display_name: display_name || undefined, role });
    if (urlEl) urlEl.value = resp.accept_url;
    if (expEl) expEl.textContent = `Expires in ${formatExpiryHours(resp.expires_in_hours)}.`;
    if (box) box.classList.remove('hidden');
    if (ok) ok.classList.remove('hidden');
  } catch (e) {
    if (err) {
      err.textContent = e.message || 'Failed to create invite.';
      err.classList.remove('hidden');
    }
  } finally {
    if (btn) btn.disabled = false;
  }
}

function showCopyButtonStatus(text, restoreAfterMs = 2000) {
  const btn = document.getElementById('copyInviteLinkBtn');
  if (!btn) return;
  const original = btn.dataset.originalText || btn.textContent;
  btn.dataset.originalText = original;
  btn.textContent = text;
  setTimeout(() => {
    btn.textContent = btn.dataset.originalText || original;
  }, restoreAfterMs);
}

function copySelectedInputValue(input) {
  input.focus();
  input.select();
  input.setSelectionRange(0, input.value.length);
  return document.execCommand && document.execCommand('copy');
}

export async function copyInviteLink() {
  const urlEl = document.getElementById('inviteAcceptUrl');
  if (!urlEl || !urlEl.value) return;
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(urlEl.value);
    } else if (!copySelectedInputValue(urlEl)) {
      throw new Error('Clipboard copy failed.');
    }
    showCopyButtonStatus('✓ Copied');
  } catch {
    if (copySelectedInputValue(urlEl)) {
      showCopyButtonStatus('✓ Copied');
    } else {
      showCopyButtonStatus('Copy failed');
    }
  }
}
