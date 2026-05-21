import { useNavigate, useLocation, Navigate } from 'react-router-dom';
import { useAppState } from '../providers/AppStateProvider.jsx';

export function CandidateWelcomeScreen() {
  const navigate = useNavigate();
  const location = useLocation();
  const { interview } = useAppState();

  // Reached without a primed session (e.g. a refresh) — restart from the boot
  // dispatcher, preserving ?session= so it re-loads.
  if (!interview.joinNonce) {
    return <Navigate to={`/${location.search}`} replace />;
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

      <button
        className="btn-primary text-lg"
        onClick={() => navigate(`/interview${location.search}`)}
      >
        🎤 I'm Ready — Start Interview
      </button>
    </div>
  );
}
