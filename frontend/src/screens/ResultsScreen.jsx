import { useLocation, Navigate } from 'react-router-dom';
import { ENV, envNumber } from '../app/env.js';
import { useAppState } from '../providers/AppStateProvider.jsx';

export function ResultsScreen() {
  const location = useLocation();
  const { interview, config } = useAppState();
  const r = interview.results;

  // No results in context (e.g. a direct visit or refresh) — restart.
  if (!r) return <Navigate to={`/${location.search}`} replace />;

  const passed = r.overall_percentage >= config.passThreshold;
  const ringColor = passed
    ? ENV.RESULT_PASS_COLOR
    : (r.overall_percentage >= envNumber('RESULT_WARNING_SCORE_THRESHOLD')
        ? ENV.RESULT_WARNING_COLOR
        : ENV.RESULT_FAIL_COLOR);
  const radius = envNumber('RESULT_RING_RADIUS');
  const circ = 2 * Math.PI * radius;
  const offset = circ - (r.overall_percentage / 100) * circ;
  const pdfReady = Boolean(interview.reportPdfUrl);
  const pdfError = interview.reportPdfError;

  function handleDownload() {
    if (interview.reportPdfUrl) {
      window.open(interview.reportPdfUrl, '_blank', 'noopener');
    } else if (interview.reportPdfError) {
      alert(interview.reportPdfError);
    } else {
      alert('PDF is still being generated. Please try again in a few seconds.');
    }
  }

  return (
    <div className="card p-8">
      <div className="text-center mb-8">
        <h2 className="text-3xl font-bold mb-1">Interview Results</h2>
        <p className="text-gray-600">{interview.candidateName} — {interview.role}</p>
      </div>

      <div className="flex flex-col md:flex-row items-center gap-8 mb-8 justify-center">
        <div className="relative">
          <svg width="180" height="180" className="score-ring">
            <circle cx="90" cy="90" r={radius} stroke="#e5e7eb" strokeWidth="14" fill="none" />
            <circle
              cx="90" cy="90" r={radius} stroke={ringColor} strokeWidth="14" fill="none"
              strokeDasharray={circ} strokeDashoffset={offset} strokeLinecap="round"
              style={{ transition: 'stroke-dashoffset 1.5s ease-out' }}
            />
          </svg>
          <div className="absolute inset-0 flex flex-col items-center justify-center">
            <div className="text-4xl font-bold">{Math.round(r.overall_percentage)}%</div>
            <div className="text-xs text-gray-500 uppercase tracking-wide">Overall</div>
          </div>
        </div>
        <div className="text-center md:text-left">
          <span className={`inline-block px-4 py-2 rounded-full text-sm font-semibold ${passed ? 'badge-pass' : 'badge-fail'} mb-3`}>
            {passed ? '✓ PASSED — Advance to human round' : `✗ Below ${config.passThreshold}% threshold`}
          </span>
          <div className="grid grid-cols-2 gap-4">
            <div className="bg-gray-50 rounded-lg p-3">
              <div className="text-xs text-gray-500 uppercase">Technical</div>
              <div className="text-2xl font-bold">{Math.round(r.technical_score)}%</div>
            </div>
            <div className="bg-gray-50 rounded-lg p-3">
              <div className="text-xs text-gray-500 uppercase">Behavioral</div>
              <div className="text-2xl font-bold">{Math.round(r.behavioral_score)}%</div>
            </div>
          </div>
        </div>
      </div>

      <div className="bg-blue-50 border border-blue-200 rounded-lg p-4 mb-6">
        <p className="text-sm text-blue-900">{r.summary}</p>
      </div>

      <div className="grid md:grid-cols-3 gap-4 mb-6">
        <div className="bg-green-50 border border-green-200 rounded-lg p-4">
          <h3 className="font-bold text-green-900 mb-2">💪 Strengths</h3>
          <ul className="text-sm text-green-800 space-y-2 list-disc ml-4">
            {r.strengths.map((s, i) => <li key={i}>{s}</li>)}
          </ul>
        </div>
        <div className="bg-amber-50 border border-amber-200 rounded-lg p-4">
          <h3 className="font-bold text-amber-900 mb-2">⚠️ Weaknesses</h3>
          <ul className="text-sm text-amber-800 space-y-2 list-disc ml-4">
            {r.weaknesses.map((s, i) => <li key={i}>{s}</li>)}
          </ul>
        </div>
        <div className="bg-indigo-50 border border-indigo-200 rounded-lg p-4">
          <h3 className="font-bold text-indigo-900 mb-2">🎯 Action Items for Next Round</h3>
          <ul className="text-sm text-indigo-800 space-y-2 list-disc ml-4">
            {r.action_items.map((s, i) => <li key={i}>{s}</li>)}
          </ul>
        </div>
      </div>

      <details className="mb-6 bg-gray-50 rounded-lg p-4">
        <summary className="font-semibold cursor-pointer">📝 Per-question breakdown</summary>
        <div className="mt-4 space-y-3">
          {r.per_question.map((pq, i) => (
            <div key={i} className="bg-white rounded-lg p-3 border border-gray-200">
              <div className="flex justify-between items-start mb-1">
                <span className="text-xs uppercase font-bold text-gray-500">{pq.section} • Q{pq.q}</span>
                <span className={`text-sm font-bold ${pq.score >= 85 ? 'text-green-600' : pq.score >= 65 ? 'text-amber-600' : 'text-red-600'}`}>
                  {Math.round(pq.score)}%
                </span>
              </div>
              <div className="text-sm font-medium mb-1">{pq.question || ''}</div>
              <div className="text-xs text-gray-700">💬 {pq.feedback}</div>
            </div>
          ))}
        </div>
      </details>

      <div className="flex flex-wrap gap-3 justify-center">
        <button className="btn-primary" disabled={!pdfReady} onClick={handleDownload}>
          📄 Download PDF Report
        </button>
        {!pdfReady && (
          <p className={`basis-full text-center text-sm ${pdfError ? 'text-red-600' : 'text-gray-500'}`}>
            {pdfError || 'PDF report is still being generated.'}
          </p>
        )}
      </div>
    </div>
  );
}
