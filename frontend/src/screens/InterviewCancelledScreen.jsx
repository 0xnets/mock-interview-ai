import { useState } from 'react';
import { Navigate, useLocation } from 'react-router-dom';
import { useAppState } from '../providers/AppStateProvider.jsx';

export function InterviewCancelledScreen() {
  const location = useLocation();
  const { interview } = useAppState();
  const [closeNotice, setCloseNotice] = useState('');

  if (!interview.shortcode && !interview.sessionId) {
    return <Navigate to={`/${location.search}`} replace />;
  }

  function handleExit() {
    window.close();
    window.setTimeout(() => {
      setCloseNotice('Your browser blocked automatic tab closing. You can close this tab manually.');
    }, 250);
  }

  return (
    <div className="card p-10 text-center">
      <div className="text-6xl mb-4">✕</div>
      <h2 className="text-3xl font-bold mb-3">Interview cancelled</h2>
      <p className="text-gray-600 max-w-xl mx-auto mb-6">
        This interview session was stopped before completion. No final score or report
        will be generated from the cancelled attempt.
      </p>

      <div className="bg-amber-50 border border-amber-200 rounded-lg p-5 text-left max-w-xl mx-auto mb-6">
        <h3 className="font-bold text-amber-900 mb-2">What happens next?</h3>
        <ul className="text-sm text-amber-900 space-y-1 list-disc ml-5">
          <li>If you cannot restart, contact your HR or recruiter for a new interview link.</li>
          <li>Any answers from this cancelled attempt should not be treated as a completed interview.</li>
        </ul>
      </div>

      <div className="flex flex-col items-center gap-3">
        <button className="btn-secondary" onClick={handleExit}>
          Exit
        </button>
        {closeNotice && <p className="text-sm text-gray-500">{closeNotice}</p>}
      </div>
    </div>
  );
}
