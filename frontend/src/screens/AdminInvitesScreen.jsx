import { useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { createInvite } from '../api/client.js';
import { copyToClipboard } from '../utils/clipboard.js';

function formatExpiryHours(hours) {
  const value = Number(hours);
  if (!Number.isFinite(value)) return `${hours} hour(s)`;
  if (value >= 24 && value % 24 === 0) {
    const days = value / 24;
    return `${days} ${days === 1 ? 'day' : 'days'}`;
  }
  return `${value} ${value === 1 ? 'hour' : 'hours'}`;
}

export function AdminInvitesScreen() {
  const navigate = useNavigate();
  const urlRef = useRef(null);
  const [email, setEmail] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [role, setRole] = useState('hr');
  const [result, setResult] = useState(null);
  const [error, setError] = useState('');
  const [created, setCreated] = useState(false);
  const [copyLabel, setCopyLabel] = useState('📋 Copy');
  const [busy, setBusy] = useState(false);

  async function handleCreate() {
    setError('');
    setCreated(false);
    setResult(null);
    const trimmedEmail = email.trim();
    if (!trimmedEmail.includes('@')) {
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
      setCreated(true);
    } catch (e) {
      setError(e.message || 'Failed to create invite.');
    } finally {
      setBusy(false);
    }
  }

  async function handleCopy() {
    const ok = await copyToClipboard(urlRef.current);
    setCopyLabel(ok ? '✓ Copied' : 'Copy failed');
    setTimeout(() => setCopyLabel('📋 Copy'), 2000);
  }

  return (
    <div className="card p-8 max-w-xl mx-auto">
      <h2 className="text-2xl font-bold mb-4">Invite a teammate</h2>
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
      <div className="mb-4">
        <label className="block text-sm font-semibold mb-2">Role</label>
        <select className="input" value={role} onChange={e => setRole(e.target.value)}>
          <option value="hr">HR</option>
          <option value="admin">Admin</option>
        </select>
      </div>
      <button className="btn-primary" disabled={busy} onClick={handleCreate}>Create invite</button>
      {created && <span className="ml-3 text-sm text-green-600">✓ Created</span>}
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
      {error && <p className="text-sm text-red-600 mt-3">{error}</p>}
      <div className="mt-6">
        <button className="btn-secondary text-sm" onClick={() => navigate('/setup')}>← Back</button>
      </div>
    </div>
  );
}
