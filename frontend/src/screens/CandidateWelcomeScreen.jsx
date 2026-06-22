import { useMemo, useRef, useState } from 'react';
import { useNavigate, useLocation, Navigate } from 'react-router-dom';
import { useAppState } from '../providers/AppStateProvider.jsx';
import { useToast } from '../providers/ToastProvider.jsx';
import { PRE_INTERVIEW_CHECKS } from '../system/pre-interview-checks.js';
import { issueJoinNonce, reportSystemIncompatible } from '../api/client.js';

const initialChecks = PRE_INTERVIEW_CHECKS.map(check => ({
  id: check.id,
  label: check.label,
  status: 'idle',
  detail: 'Not checked yet.',
  retryable: false,
}));

function statusGlyph(status) {
  if (status === 'running') return '…';
  if (status === 'confirming') return '?';
  if (status === 'passed') return '✓';
  if (status === 'failed') return '!';
  return '•';
}

function statusLabel(status) {
  if (status === 'running') return 'Checking';
  if (status === 'confirming') return 'Confirm';
  if (status === 'passed') return 'Ready';
  if (status === 'failed') return 'Needs attention';
  return 'Pending';
}

function PreInterviewCheckPanel({ shortcode, onStart }) {
  const { updateInterview } = useAppState();
  const [checks, setChecks] = useState(initialChecks);
  const [running, setRunning] = useState(false);
  const [starting, setStarting] = useState(false);
  const [incompatNotice, setIncompatNotice] = useState('');
  const [confirmingCheckId, setConfirmingCheckId] = useState('');
  const confirmationResolveRef = useRef(null);
  const allPassed = useMemo(() => checks.every(check => check.status === 'passed'), [checks]);

  function patchCheck(id, patch) {
    setChecks(prev => prev.map(check => (
      check.id === id ? { ...check, ...patch } : check
    )));
  }

  /// Tell the backend a system check failed so it can count incompatibility
  /// attempts and expire the link once the limit is hit. Best-effort — a
  /// reporting failure must not block the candidate from retrying.
  async function reportIncompatibility(failure) {
    try {
      const res = await reportSystemIncompatible(shortcode, {
        check: failure.id,
        detail: failure.detail,
      });
      if (res?.expired) {
        updateInterview({ linkExpired: true });
        return;
      }
      const remaining = Number(res?.attempts_remaining);
      if (Number.isFinite(remaining) && remaining > 0) {
        setIncompatNotice(
          `This device failed a system check. ${remaining} more failed `
          + `${remaining === 1 ? 'attempt' : 'attempts'} will expire this `
          + `interview link, and you'll need a new one from HR.`
        );
      }
    } catch (e) {
      console.error('Failed to report system incompatibility:', e);
    }
  }

  async function runSingleCheck(check) {
    patchCheck(check.id, { status: 'running', detail: 'Checking…', retryable: false });
    try {
      const result = await check.run({
        shortcode,
        onProgress: detail => patchCheck(check.id, { detail }),
      });
      if (result.needsConfirmation) {
        patchCheck(check.id, {
          status: 'confirming',
          detail: result.needsConfirmation.question,
          retryable: false,
        });
        const confirmed = await new Promise(resolve => {
          confirmationResolveRef.current = resolve;
          setConfirmingCheckId(check.id);
        });
        confirmationResolveRef.current = null;
        setConfirmingCheckId('');
        const detail = confirmed
          ? result.needsConfirmation.passDetail
          : result.needsConfirmation.failDetail;
        patchCheck(check.id, {
          status: confirmed ? 'passed' : 'failed',
          detail,
          retryable: !confirmed,
        });
        return confirmed
          ? null
          : { id: check.id, systemCheck: check.systemCheck, detail, retryable: true };
      }

      patchCheck(check.id, {
        status: result.ok ? 'passed' : 'failed',
        detail: result.detail,
        retryable: Boolean(result.retryable),
      });
      return result.ok
        ? null
        : {
            id: check.id,
            systemCheck: check.systemCheck,
            detail: result.detail,
            retryable: Boolean(result.retryable),
          };
    } catch (e) {
      const detail = e?.message || 'Check failed.';
      patchCheck(check.id, { status: 'failed', detail, retryable: true });
      return { id: check.id, systemCheck: check.systemCheck, detail, retryable: true };
    }
  }

  async function runChecksFrom(startIndex, resetChecks) {
    setRunning(true);
    setConfirmingCheckId('');
    setIncompatNotice('');
    confirmationResolveRef.current = null;
    if (resetChecks) {
      setChecks(initialChecks.map(check => ({ ...check, status: 'idle', detail: 'Waiting…' })));
    } else {
      setChecks(prev => prev.map((check, index) => (
        index >= startIndex
          ? { ...check, status: 'idle', detail: 'Waiting…', retryable: false }
          : check
      )));
    }

    let failure = null;
    for (const check of PRE_INTERVIEW_CHECKS.slice(startIndex)) {
      failure = await runSingleCheck(check);
      if (failure) break;
    }

    setRunning(false);

    if (failure && failure.systemCheck && !failure.retryable) {
      await reportIncompatibility(failure);
    }
  }

  async function runChecks() {
    await runChecksFrom(0, true);
  }

  async function retryCheck(id) {
    const startIndex = PRE_INTERVIEW_CHECKS.findIndex(check => check.id === id);
    if (startIndex >= 0) {
      await runChecksFrom(startIndex, false);
    }
  }

  function resolveConfirmation(confirmed) {
    confirmationResolveRef.current?.(confirmed);
  }

  async function handleStartClick() {
    setStarting(true);
    try {
      await onStart();
    } finally {
      setStarting(false);
    }
  }

  return (
    <div className="bg-white border border-gray-200 rounded-lg p-5 mb-6 text-left max-w-2xl mx-auto">
      <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-3 mb-4">
        <div>
          <h3 className="font-bold text-gray-900">Pre-interview system check</h3>
          <p className="text-sm text-gray-600">Run this once before starting so audio, voice, and connectivity are ready.</p>
        </div>
        <button className="btn-secondary text-sm" disabled={running} onClick={runChecks}>
          {running ? 'Checking…' : allPassed ? 'Run Again' : 'Run Check'}
        </button>
      </div>

      {incompatNotice && (
        <div className="mb-4 rounded-lg border border-amber-200 bg-amber-50 p-3 text-sm text-amber-900">
          {incompatNotice}
        </div>
      )}

      <div className="space-y-2">
        {checks.map(check => (
          <div key={check.id} className="flex gap-3 rounded-lg border border-gray-100 bg-gray-50 p-3">
            <div className={`check-glyph check-${check.status}`}>{statusGlyph(check.status)}</div>
            <div className="min-w-0 flex-1">
              <div className="flex flex-col md:flex-row md:items-baseline md:justify-between gap-1">
                <span className="font-semibold text-sm text-gray-900">{check.label}</span>
                <span className="text-xs uppercase text-gray-500">{statusLabel(check.status)}</span>
              </div>
              <p className="text-sm text-gray-600 mt-1">{check.detail}</p>
              {confirmingCheckId === check.id && (
                <div className="mt-3 flex gap-2">
                  <button className="btn-primary text-sm" onClick={() => resolveConfirmation(true)}>
                    Yes, I heard it
                  </button>
                  <button className="btn-secondary text-sm" onClick={() => resolveConfirmation(false)}>
                    No, retry needed
                  </button>
                </div>
              )}
              {check.status === 'failed' && check.retryable && (
                <button
                  className="btn-secondary text-sm mt-3"
                  disabled={running}
                  onClick={() => retryCheck(check.id)}
                >
                  Retry {check.label}
                </button>
              )}
            </div>
          </div>
        ))}
      </div>

      <button
        className="btn-primary w-full text-lg mt-5"
        disabled={!allPassed || running || starting}
        onClick={handleStartClick}
      >
        {starting ? 'Starting…' : '🎤 I\'m Ready — Start Interview'}
      </button>
    </div>
  );
}

function ExpiredLinkPanel() {
  return (
    <div className="card p-10 text-center">
      <div className="text-6xl mb-4">⛔</div>
      <h2 className="text-3xl font-bold mb-2">This interview link has expired</h2>
      <p className="text-gray-600 mb-2 max-w-xl mx-auto">
        The interview could not be started on this device after several attempts,
        so the link is no longer valid.
      </p>
      <p className="text-gray-600 max-w-xl mx-auto">
        Please contact your HR or recruiter to request a new interview link.
      </p>
    </div>
  );
}

export function CandidateWelcomeScreen() {
  const navigate = useNavigate();
  const location = useLocation();
  const { interview, updateInterview } = useAppState();
  const toast = useToast();

  // Reached without a loaded session (e.g. a refresh) — restart from the boot
  // dispatcher, preserving ?session= so it re-loads.
  if (!interview.shortcode) {
    return <Navigate to={`/${location.search}`} replace />;
  }

  if (interview.linkExpired) {
    return <ExpiredLinkPanel />;
  }

  // Mint the single-use join nonce only now that the candidate is actually
  // starting — a failed pre-interview check never consumes the link.
  async function handleStart() {
    try {
      const nonceInfo = await issueJoinNonce(interview.shortcode);
      updateInterview({
        joinNonce: nonceInfo.join_nonce,
        wsPath: nonceInfo.ws_path || '/v1/ws/interview',
        answerTimeLimitMs: Number(nonceInfo.answer_time_limit_ms) || interview.answerTimeLimitMs,
      });
      navigate(`/interview${location.search}`);
    } catch (e) {
      console.error('Failed to start interview:', e);
      toast.error(
        'This interview link can no longer be used to start the interview. '
        + 'Please contact your HR or recruiter for a new link.'
      );
    }
  }

  return (
    <div className="card p-10 text-center">
      <div className="text-6xl mb-4">👋</div>
      <h2 className="text-3xl font-bold mb-2">Welcome, {interview.candidateName || 'Candidate'}</h2>
      <p className="text-gray-600 mb-2">You're about to begin your mock interview for:</p>
      <p className="text-lg font-semibold text-indigo-600 mb-8">{interview.role || '—'}</p>

      <div className="bg-blue-50 border border-blue-200 rounded-lg p-5 mb-6 text-left max-w-xl mx-auto">
        <h3 className="font-bold text-blue-900 mb-2">📋 Before you start:</h3>
        <ul className="text-sm text-blue-900 space-y-1 list-disc ml-5">
          <li>Use <strong>Chrome or Edge</strong> for the best experience</li>
          <li>Find a <strong>quiet space</strong> — your answers will be transcribed by voice</li>
          <li>The AI will speak each question aloud (US accent)</li>
          <li>Click 🎤 to answer, click again when done</li>
          <li>You'll have <span>a personalized set of</span> questions in total — about <strong>20-30 min</strong></li>
          <li>At the end, you'll get a detailed report you can download as PDF</li>
        </ul>
      </div>

      <PreInterviewCheckPanel
        shortcode={interview.shortcode}
        onStart={handleStart}
      />
    </div>
  );
}
