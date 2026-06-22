import { useEffect, useState } from 'react';
import { useNavigate, useLocation, Navigate } from 'react-router-dom';
import { getTranscript } from '../api/client.js';
import { useAppState } from '../providers/AppStateProvider.jsx';

function decodeBase64Utf8(b64) {
  try {
    const bin = atob(b64.replace(/-/g, '+').replace(/_/g, '/'));
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    return new TextDecoder('utf-8', { fatal: false }).decode(bytes);
  } catch {
    return '';
  }
}

export function TranscriptScreen() {
  const navigate = useNavigate();
  const location = useLocation();
  const { interview } = useAppState();
  const sid = interview.sessionId;

  const [data, setData] = useState(null);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!sid) return undefined;
    let active = true;
    setLoading(true);
    getTranscript(sid)
      .then(t => { if (active) { setData(t); setLoading(false); } })
      .catch(e => { if (active) { setError(e.message || 'Failed to load transcript.'); setLoading(false); } });
    return () => { active = false; };
  }, [sid]);

  if (!sid) return <Navigate to={`/${location.search}`} replace />;

  const chunks = data?.chunks || [];
  const checkpoints = data?.checkpoints || [];

  return (
    <div className="card p-8">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-2xl font-bold">Signed Transcript</h2>
        <button className="btn-secondary text-sm" onClick={() => navigate(`/results${location.search}`)}>← Back</button>
      </div>
      <p className="text-sm text-gray-600 mb-4">
        Chain-hashed transcript export.{' '}
        <span>{data ? `${chunks.length} chunk(s), ${checkpoints.length} checkpoint(s).` : ''}</span>
      </p>
      <div className="mb-4 text-xs text-gray-500">
        <span>Session: <code>{sid}</code></span> ·{' '}
        <span>Public key (base64): <code className="break-all">{data ? (data.public_key_b64 || '—') : '…'}</code></span>
      </div>
      <div className="space-y-3 mb-6">
        {loading && <div className="text-sm text-gray-500">Loading…</div>}
        {chunks.map(c => (
          <div key={c.seq} className="p-3 bg-gray-50 border border-gray-200 rounded">
            <div className="text-xs text-gray-500 mb-1">#{c.seq} · {c.ordinal_or_section || ''}</div>
            <pre className="text-xs whitespace-pre-wrap break-words">
              {decodeBase64Utf8(c.canonical_row_b64) || c.canonical_row_b64}
            </pre>
            <div className="text-[10px] font-mono text-gray-400 mt-1">chain: {c.chain_hash_b64 || ''}</div>
          </div>
        ))}
      </div>
      <h3 className="text-lg font-semibold mt-6 mb-2">Checkpoints</h3>
      <div className="space-y-2 text-xs font-mono">
        {checkpoints.map(cp => (
          <div key={cp.seq} className="p-2 bg-gray-50 border border-gray-200 rounded">
            <div>#{cp.seq} · signed_at <span className="text-gray-500">{cp.signed_at || ''}</span></div>
            <div className="text-gray-500">chain: {cp.chain_hash_b64 || ''}</div>
            <div className="text-gray-500 break-all">sig: {cp.signature_b64 || ''}</div>
          </div>
        ))}
      </div>
      {error && <p className="text-sm text-red-600 mt-3">{error}</p>}
    </div>
  );
}
