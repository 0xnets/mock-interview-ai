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
    if (expEl) expEl.textContent = `Expires in ${resp.expires_in_hours} hour(s).`;
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

export async function copyInviteLink() {
  const urlEl = document.getElementById('inviteAcceptUrl');
  if (!urlEl || !urlEl.value) return;
  try {
    await navigator.clipboard.writeText(urlEl.value);
  } catch {
    urlEl.select();
    document.execCommand && document.execCommand('copy');
  }
}
