import { useRef, useState } from 'react';
import { useAppState } from '../providers/AppStateProvider.jsx';
import { createInterview } from '../api/client.js';
import { PdfTextarea } from './PdfTextarea.jsx';
import { copyToClipboard } from '../utils/clipboard.js';

export function GenerateLinkTab() {
  const { configStatus, hasSavedConfig } = useAppState();
  const configLoading = configStatus === 'loading' || configStatus === 'idle';
  const linkRef = useRef(null);
  const [name, setName] = useState('');
  const [role, setRole] = useState('');
  const [jd, setJd] = useState('');
  const [resume, setResume] = useState('');
  const [showBox, setShowBox] = useState(false);
  const [linkValue, setLinkValue] = useState('');
  const [info, setInfo] = useState(null);
  const [busy, setBusy] = useState(false);
  const [copyLabel, setCopyLabel] = useState('📋 Copy URL');

  async function handleGenerate() {
    const n = name.trim();
    const r = role.trim();
    const j = jd.trim();
    const res = resume.trim();
    if (!n || !r || !j || !res) { alert('Please fill in all four fields.'); return; }
    if (configLoading) {
      alert('HR configuration is still loading. Please wait a moment and try again.');
      return;
    }
    if (!hasSavedConfig) {
      alert('Please set up HR Configuration (recipient email and behavioral questions) before generating a link.');
      return;
    }

    setShowBox(true);
    setLinkValue('⏳ Creating session...');
    setInfo({ tone: 'gray', text: 'The backend is generating personalized questions. This usually takes 10-20 seconds — the candidate experience will be instant.' });
    setBusy(true);
    try {
      const created = await createInterview({
        candidate_name: n,
        role_title: r,
        jd_text: j,
        resume_text: res,
        include_intro: true,
      });
      setLinkValue(created.share_url);
      setInfo({ tone: 'green', shortcode: created.shortcode, expiresAt: created.expires_at });
    } catch (e) {
      setInfo({ tone: 'red', text: `✗ Failed to create session: ${e.message}` });
      setLinkValue('');
    } finally {
      setBusy(false);
    }
  }

  async function handleCopy() {
    const ok = await copyToClipboard(linkRef.current);
    setCopyLabel(ok ? '✓ Copied!' : 'Copy failed');
    setTimeout(() => setCopyLabel('📋 Copy URL'), 2000);
  }

  return (
    <div>
      <h2 className="text-xl font-bold mb-2">Generate a Candidate Link</h2>
      <p className="text-sm text-gray-600 mb-4">
        Fill in the JD and resume. The AI will pre-generate personalized questions when you
        click Generate (takes ~10-15 sec). You'll get a short unique URL to send the candidate —
        they open it, click Start, and the interview begins instantly.
      </p>

      <div className="grid md:grid-cols-2 gap-4 mb-4">
        <div>
          <label className="block text-sm font-semibold mb-2">Candidate Name</label>
          <input className="input" placeholder="e.g., Priya Sharma" value={name} onChange={e => setName(e.target.value)} />
        </div>
        <div>
          <label className="block text-sm font-semibold mb-2">Role / Position</label>
          <input className="input" placeholder="e.g., Senior Backend Engineer" value={role} onChange={e => setRole(e.target.value)} />
        </div>
      </div>
      <PdfTextarea
        className="mb-4"
        label="Job Description (JD)"
        value={jd}
        onChange={setJd}
        rows={5}
        placeholder="Paste the JD here, or upload a PDF above..."
      />
      <PdfTextarea
        className="mb-4"
        label="Candidate's Resume"
        value={resume}
        onChange={setResume}
        rows={5}
        placeholder="Paste the resume here, or upload a PDF above..."
      />

      <button className="btn-primary" disabled={busy || configLoading} onClick={handleGenerate}>
        {configLoading ? '⏳ Loading configuration…' : '🔗 Generate Link'}
      </button>

      {showBox && (
        <div className="mt-6 p-4 bg-green-50 border border-green-200 rounded-lg">
          <div className="text-sm font-semibold text-green-900 mb-2">✓ Link ready — send it to the candidate:</div>
          <div className="flex gap-2 mb-2">
            <input ref={linkRef} className="input bg-white text-xs" readOnly value={linkValue} />
            <button className="btn-secondary whitespace-nowrap" onClick={handleCopy}>{copyLabel}</button>
          </div>
          <p className="text-xs mb-3">
            {info?.tone === 'gray' && <span className="text-gray-600">{info.text}</span>}
            {info?.tone === 'red' && <span className="text-red-700">{info.text}</span>}
            {info?.tone === 'green' && (
              <span className="text-green-800">
                ✓ Link ready — expires {new Date(info.expiresAt).toLocaleString()}. Code:{' '}
                <code>{info.shortcode}</code>. The candidate will see a brief loading screen
                until the AI finishes priming.
              </span>
            )}
          </p>
          <p className="text-xs text-gray-600 mt-2 italic">
            The URL works in any chat app or email — its length doesn't matter to recipients.
          </p>
        </div>
      )}
    </div>
  );
}
