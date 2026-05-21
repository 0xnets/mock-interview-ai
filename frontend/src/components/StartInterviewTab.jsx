import { useState } from 'react';
import { useAppState } from '../providers/AppStateProvider.jsx';
import { hasBehavioralQuestions } from '../interview/bank-editor.js';
import { PdfTextarea } from './PdfTextarea.jsx';

/// The HR-side "Start a Mock Interview" panel. Retained from the original app;
/// it has no tab button, so it is not reachable through the tab bar.
export function StartInterviewTab({ onStart, busy }) {
  const { config } = useAppState();
  const [name, setName] = useState('');
  const [role, setRole] = useState('');
  const [jd, setJd] = useState('');
  const [resume, setResume] = useState('');
  const configOk = Boolean(config.hrEmail && hasBehavioralQuestions(config.nonTechBank));

  return (
    <div>
      {!configOk && (
        <div className="mb-4 p-4 bg-amber-50 border border-amber-200 rounded-lg text-amber-800">
          ⚠️ HR Setup is not complete. Please go to <strong>HR Configuration</strong> and add the
          report recipient email + non-tech questions first.
        </div>
      )}

      <h2 className="text-xl font-bold mb-4">Start a Mock Interview</h2>
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
        label="Job Description (JD) — include tech stack & expectations"
        value={jd}
        onChange={setJd}
        rows={6}
        placeholder="Paste the JD here, or upload a PDF above..."
      />
      <PdfTextarea
        className="mb-6"
        label="Candidate's Resume"
        value={resume}
        onChange={setResume}
        rows={6}
        placeholder="Paste the resume here, or upload a PDF above..."
      />
      <div className="mb-6 p-4 bg-blue-50 rounded-lg border border-blue-200 text-sm text-blue-900">
        <strong>📋 What happens next:</strong> The AI interviewer (US accent) will ask{' '}
        <span>{config.techCount}</span> technical questions tailored to the JD + resume, then{' '}
        <span>{config.nonTechCount}</span> behavioral questions. The candidate must score{' '}
        <strong>90% or higher</strong> to advance to the human round.
      </div>
      <button
        className="btn-primary w-full text-lg"
        disabled={busy}
        onClick={() => onStart({ name: name.trim(), role: role.trim(), jd: jd.trim(), resume: resume.trim() })}
      >
        🎤 Start Interview
      </button>
    </div>
  );
}
