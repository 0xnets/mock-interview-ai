import { useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { acceptInvite } from '../api/client.js';

/// Read invite tokens from current and older invite-link query params.
function tokenFromParams(params) {
  return params.get('invite') || params.get('accept_invite') || params.get('token') || '';
}

export function AcceptInviteScreen() {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const urlToken = tokenFromParams(params);

  const [token, setToken] = useState(urlToken);
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [error, setError] = useState('');
  const [success, setSuccess] = useState(false);
  const [busy, setBusy] = useState(false);

  async function handleSubmit() {
    setError('');
    setSuccess(false);
    const effectiveToken = urlToken || token.trim();
    if (!effectiveToken) {
      setError('Enter the invite token from your email.');
      return;
    }
    if (password.length < 12) {
      setError('Password must be at least 12 characters.');
      return;
    }
    if (password !== confirmPassword) {
      setError('Passwords do not match.');
      return;
    }
    setBusy(true);
    try {
      const resp = await acceptInvite({ token: effectiveToken, password });
      setSuccess(true);
      setPassword('');
      setConfirmPassword('');
      setTimeout(() => navigate('/login', { state: { email: resp?.email } }), 1200);
    } catch (e) {
      setError(e.message || 'Failed to accept invite.');
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="card p-8 max-w-md mx-auto">
      <h2 className="text-2xl font-bold mb-4">Set up your account</h2>
      <p className="text-sm text-gray-600 mb-4">Choose a password to finish accepting your invite.</p>
      <div className="mb-4">
        <label className="block text-sm font-semibold mb-2">Invite token</label>
        <input
          type="text"
          className="input"
          placeholder="Paste the token from your invite email"
          autoComplete="off"
          readOnly={!!urlToken}
          value={token}
          onChange={e => setToken(e.target.value)}
        />
      </div>
      <div className="mb-4">
        <label className="block text-sm font-semibold mb-2">New Password</label>
        <input
          type="password"
          className="input"
          autoComplete="new-password"
          value={password}
          onChange={e => setPassword(e.target.value)}
        />
        <p className="text-xs text-gray-500 mt-1">Minimum 12 characters.</p>
      </div>
      <div className="mb-4">
        <label className="block text-sm font-semibold mb-2">Confirm Password</label>
        <input
          type="password"
          className="input"
          autoComplete="new-password"
          value={confirmPassword}
          onChange={e => setConfirmPassword(e.target.value)}
        />
      </div>
      <button className="btn-primary w-full" disabled={busy} onClick={handleSubmit}>Accept invite</button>
      {error && <p className="text-sm text-red-600 mt-3">{error}</p>}
      {success && <p className="text-sm text-green-600 mt-3">✓ Account set up — redirecting to sign in…</p>}
      <button
        type="button"
        className="text-xs text-indigo-600 underline mt-4"
        onClick={() => navigate('/login')}
      >
        ← Back to sign in
      </button>
    </div>
  );
}
