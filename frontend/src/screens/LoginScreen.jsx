import { useState } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import { login } from '../api/client.js';

export function LoginScreen() {
  const navigate = useNavigate();
  const location = useLocation();
  const [email, setEmail] = useState(location.state?.email || '');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  async function handleSubmit() {
    setError('');
    if (!email || !password) {
      setError('Email and password are required.');
      return;
    }
    setBusy(true);
    try {
      await login(email, password);
      navigate('/setup');
    } catch (e) {
      const detail = e?.status === 429 && e.retryAfter
        ? `Too many attempts. Try again in ${e.retryAfter}s.`
        : (e?.status === 401 ? 'Invalid email or password.' : e.message);
      setError(detail);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="card p-8 max-w-md mx-auto">
      <h2 className="text-2xl font-bold mb-4">HR Sign in</h2>
      <div className="mb-4">
        <label className="block text-sm font-semibold mb-2">Email</label>
        <input
          type="email"
          className="input"
          autoComplete="username"
          value={email}
          onChange={e => setEmail(e.target.value)}
        />
      </div>
      <div className="mb-4">
        <label className="block text-sm font-semibold mb-2">Password</label>
        <input
          type="password"
          className="input"
          autoComplete="current-password"
          value={password}
          onChange={e => setPassword(e.target.value)}
        />
      </div>
      <button className="btn-primary w-full" disabled={busy} onClick={handleSubmit}>Sign in</button>
      {error && <p className="text-sm text-red-600 mt-3">{error}</p>}
      <p className="text-xs text-gray-500 mt-4">
        Have an invite?{' '}
        <button
          type="button"
          className="text-indigo-600 underline"
          onClick={() => navigate('/accept-invite')}
        >
          Set up your account →
        </button>
      </p>
    </div>
  );
}
