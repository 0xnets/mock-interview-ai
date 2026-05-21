import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  createInvite,
  disableAccount,
  listInvites,
  resendInvite,
  revokeInvite,
  undoRevokeInvite,
} from '../api/client.js';
import { useAuth } from '../providers/AuthProvider.jsx';
import { copyToClipboard } from '../utils/clipboard.js';
import { isValidEmail } from '../utils/validation.js';

const FILTERS = [
  { id: 'all', label: 'All' },
  { id: 'active', label: 'Active' },
  { id: 'expired', label: 'Expired' },
  { id: 'accepted', label: 'Accepted' },
  { id: 'revoked', label: 'Revoked' },
];

function formatExpiryHours(hours) {
  const value = Number(hours);
  if (!Number.isFinite(value)) return `${hours} hour(s)`;
  if (value >= 24 && value % 24 === 0) {
    const days = value / 24;
    return `${days} ${days === 1 ? 'day' : 'days'}`;
  }
  return `${value} ${value === 1 ? 'hour' : 'hours'}`;
}

function formatDate(value) {
  if (!value) return '-';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '-';
  return date.toLocaleString([], {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

function statusClass(status) {
  if (status === 'active') return 'bg-blue-50 text-blue-700 border-blue-200';
  if (status === 'accepted') return 'bg-green-50 text-green-700 border-green-200';
  if (status === 'revoked') return 'bg-red-50 text-red-700 border-red-200';
  return 'bg-gray-50 text-gray-700 border-gray-200';
}

async function copyText(text) {
  const input = document.createElement('input');
  input.value = text;
  input.setAttribute('readonly', '');
  input.style.position = 'fixed';
  input.style.opacity = '0';
  document.body.appendChild(input);
  const ok = await copyToClipboard(input);
  input.remove();
  return ok;
}

export function AdminInvitesScreen() {
  const navigate = useNavigate();
  const { principal } = useAuth();
  const urlRef = useRef(null);
  const isSuperAdmin = principal?.role === 'super_admin';
  const [email, setEmail] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [role, setRole] = useState('hr');
  const [result, setResult] = useState(null);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [copyLabel, setCopyLabel] = useState('Copy');
  const [busy, setBusy] = useState(false);
  const [actionBusy, setActionBusy] = useState('');
  const [filter, setFilter] = useState('all');
  const [invites, setInvites] = useState([]);
  const [loadingInvites, setLoadingInvites] = useState(false);

  async function refreshInvites(nextFilter = filter) {
    setLoadingInvites(true);
    setError('');
    try {
      const resp = await listInvites(nextFilter);
      setInvites(Array.isArray(resp?.invites) ? resp.invites : []);
    } catch (e) {
      setError(e.message || 'Failed to load invites.');
    } finally {
      setLoadingInvites(false);
    }
  }

  useEffect(() => {
    refreshInvites();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!isSuperAdmin && role === 'admin') setRole('hr');
  }, [isSuperAdmin, role]);

  async function handleCreate() {
    setError('');
    setNotice('');
    setResult(null);
    const trimmedEmail = email.trim();
    if (!isValidEmail(trimmedEmail)) {
      setError('Enter a valid email.');
      return;
    }
    if (role !== 'hr' && role !== 'admin') {
      setError('Role must be hr or admin.');
      return;
    }
    setBusy(true);
    try {
      const resp = await createInvite({
        email: trimmedEmail,
        display_name: displayName.trim() || undefined,
        role,
      });
      setResult(resp);
      setNotice('Invite created.');
      await refreshInvites();
    } catch (e) {
      setError(e.message || 'Failed to create invite.');
    } finally {
      setBusy(false);
    }
  }

  async function handleCopy() {
    const ok = await copyToClipboard(urlRef.current);
    setCopyLabel(ok ? 'Copied' : 'Copy failed');
    setTimeout(() => setCopyLabel('Copy'), 2000);
  }

  async function runAction(actionKey, fn) {
    setError('');
    setNotice('');
    setActionBusy(actionKey);
    try {
      const message = await fn();
      if (message) setNotice(message);
      await refreshInvites();
    } catch (e) {
      setError(e.message || 'Action failed.');
    } finally {
      setActionBusy('');
    }
  }

  function canManage(invite) {
    if (invite.role === 'hr') return true;
    return invite.role === 'admin' && isSuperAdmin;
  }

  async function handleResend(invite) {
    await runAction(`resend:${invite.id}`, async () => {
      const resp = await resendInvite(invite.id);
      const copied = await copyText(resp.accept_url);
      setResult(resp);
      return copied
        ? `Fresh invite link for ${invite.email} copied.`
        : `Fresh invite link for ${invite.email} generated.`;
    });
  }

  async function handleDisable(invite) {
    const confirmed = window.confirm(`Disable ${invite.email}? They will no longer be able to log in.`);
    if (!confirmed) return;
    await runAction(`disable:${invite.account_id}`, async () => {
      await disableAccount(invite.account_id);
      return `${invite.email} disabled.`;
    });
  }

  async function changeFilter(nextFilter) {
    setFilter(nextFilter);
    await refreshInvites(nextFilter);
  }

  return (
    <div className="space-y-6">
      <div className="card p-8">
        <div className="flex items-start justify-between gap-4 mb-6">
          <div>
            <h2 className="text-2xl font-bold">Invite management</h2>
            <p className="text-sm text-gray-600 mt-1">Create, revoke, restore, and resend teammate invite links.</p>
          </div>
          <button className="btn-secondary text-sm" onClick={() => navigate('/setup')}>Back</button>
        </div>

        <div className="grid md:grid-cols-2 gap-4 mb-4">
          <div>
            <label className="block text-sm font-semibold mb-2">Email <span className="text-red-500">*</span></label>
            <input
              type="email"
              className="input"
              placeholder="teammate@yourcompany.com"
              value={email}
              onChange={e => setEmail(e.target.value)}
            />
          </div>
          <div>
            <label className="block text-sm font-semibold mb-2">Display Name</label>
            <input
              type="text"
              className="input"
              placeholder="Jane Doe"
              value={displayName}
              onChange={e => setDisplayName(e.target.value)}
            />
          </div>
        </div>

        <div className="grid md:grid-cols-[1fr_auto] gap-4 items-end">
          <div>
            <label className="block text-sm font-semibold mb-2">Role</label>
            <select className="input" value={role} onChange={e => setRole(e.target.value)}>
              <option value="hr">HR</option>
              {isSuperAdmin && <option value="admin">Admin</option>}
            </select>
          </div>
          <button className="btn-primary" disabled={busy} onClick={handleCreate}>Create invite</button>
        </div>

        {result && (
          <div className="mt-6 p-4 bg-green-50 border border-green-200 rounded-lg">
            <div className="text-sm font-semibold text-green-900 mb-2">Share this accept link with the invitee:</div>
            <div className="flex gap-2 mb-2">
              <input ref={urlRef} className="input bg-white text-xs" readOnly value={result.accept_url} />
              <button className="btn-secondary whitespace-nowrap" onClick={handleCopy}>{copyLabel}</button>
            </div>
            <p className="text-xs text-green-800">Expires in {formatExpiryHours(result.expires_in_hours)}.</p>
          </div>
        )}
        {notice && <p className="text-sm text-green-700 mt-3">{notice}</p>}
        {error && <p className="text-sm text-red-600 mt-3">{error}</p>}
      </div>

      <div className="card p-8">
        <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-4 mb-5">
          <h3 className="text-xl font-bold">Generated invites</h3>
          <div className="flex flex-wrap gap-2">
            {FILTERS.map(item => (
              <button
                key={item.id}
                className={`btn-secondary text-sm px-4 py-2 ${filter === item.id ? 'is-active' : ''}`}
                onClick={() => changeFilter(item.id)}
                aria-pressed={filter === item.id}
              >
                {item.label}
              </button>
            ))}
          </div>
        </div>

        <div className="overflow-x-auto">
          <table className="min-w-full text-sm">
            <thead>
              <tr className="border-b text-left text-xs uppercase tracking-wide text-gray-500">
                <th className="py-3 pr-4">Invitee</th>
                <th className="py-3 pr-4">Role</th>
                <th className="py-3 pr-4">Invite</th>
                <th className="py-3 pr-4">Account</th>
                <th className="py-3 pr-4">Dates</th>
                <th className="py-3">Actions</th>
              </tr>
            </thead>
            <tbody>
              {loadingInvites && (
                <tr><td colSpan="6" className="py-6 text-center text-gray-500">Loading invites...</td></tr>
              )}
              {!loadingInvites && invites.length === 0 && (
                <tr><td colSpan="6" className="py-6 text-center text-gray-500">No invites found.</td></tr>
              )}
              {!loadingInvites && invites.map(invite => {
                const manageAllowed = canManage(invite);
                const isOwnAccount = invite.account_id === principal?.account_id;
                return (
                  <tr key={invite.id} className="border-b last:border-0 align-top">
                    <td className="py-4 pr-4">
                      <div className="font-semibold text-gray-900">{invite.display_name || invite.email}</div>
                      <div className="text-xs text-gray-500">{invite.email}</div>
                      {invite.invited_by_email && <div className="text-xs text-gray-400 mt-1">By {invite.invited_by_email}</div>}
                    </td>
                    <td className="py-4 pr-4">
                      <span className="inline-flex border rounded-full px-2 py-1 text-xs font-semibold bg-gray-50 text-gray-700">
                        {invite.role}
                      </span>
                    </td>
                    <td className="py-4 pr-4">
                      <span className={`inline-flex border rounded-full px-2 py-1 text-xs font-semibold ${statusClass(invite.status)}`}>
                        {invite.status}
                      </span>
                    </td>
                    <td className="py-4 pr-4">
                      <span className="text-gray-700">{invite.account_status}</span>
                    </td>
                    <td className="py-4 pr-4 text-xs text-gray-600 min-w-48">
                      <div>Created: {formatDate(invite.created_at)}</div>
                      <div>Expires: {formatDate(invite.expires_at)}</div>
                      {invite.consumed_at && <div>Accepted: {formatDate(invite.consumed_at)}</div>}
                      {invite.revoked_at && <div>Revoked: {formatDate(invite.revoked_at)}</div>}
                    </td>
                    <td className="py-4">
                      <div className="flex flex-wrap gap-2 min-w-56">
                        {invite.status === 'active' && manageAllowed && (
                          <button
                            className="btn-danger text-xs px-3 py-2"
                            disabled={Boolean(actionBusy)}
                            onClick={() => runAction(`revoke:${invite.id}`, async () => {
                              await revokeInvite(invite.id);
                              return `${invite.email} invite revoked.`;
                            })}
                          >
                            {actionBusy === `revoke:${invite.id}` ? 'Revoking...' : 'Revoke'}
                          </button>
                        )}
                        {invite.status === 'revoked' && manageAllowed && (
                          <button
                            className="btn-secondary text-xs px-3 py-2"
                            disabled={Boolean(actionBusy)}
                            onClick={() => runAction(`undo:${invite.id}`, async () => {
                              await undoRevokeInvite(invite.id);
                              return `${invite.email} invite restored.`;
                            })}
                          >
                            {actionBusy === `undo:${invite.id}` ? 'Restoring...' : 'Undo revoke'}
                          </button>
                        )}
                        {invite.status !== 'accepted' && manageAllowed && invite.account_status !== 'disabled' && (
                          <button
                            className="btn-secondary text-xs px-3 py-2"
                            disabled={Boolean(actionBusy)}
                            onClick={() => handleResend(invite)}
                          >
                            {actionBusy === `resend:${invite.id}` ? 'Resending...' : 'Resend / Copy'}
                          </button>
                        )}
                        {manageAllowed && !isOwnAccount && invite.account_status !== 'disabled' && (
                          <button
                            className="btn-danger text-xs px-3 py-2"
                            disabled={Boolean(actionBusy)}
                            onClick={() => handleDisable(invite)}
                          >
                            {actionBusy === `disable:${invite.account_id}` ? 'Disabling...' : 'Disable account'}
                          </button>
                        )}
                        {!manageAllowed && <span className="text-xs text-gray-400">No permitted actions</span>}
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
