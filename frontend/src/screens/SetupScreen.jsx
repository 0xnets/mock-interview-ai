import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Tabs } from '../components/Tabs.jsx';
import { GenerateLinkTab } from '../components/GenerateLinkTab.jsx';
import { HrConfigTab } from '../components/HrConfigTab.jsx';
import { StartInterviewTab } from '../components/StartInterviewTab.jsx';
import { LoadingScreen } from './LoadingScreen.jsx';
import { useAppState } from '../providers/AppStateProvider.jsx';
import { parseNonTechBank } from '../interview/non-tech-bank.js';
import { createInterview, waitForPrimed, issueJoinNonce } from '../api/client.js';

const TABS = [
  { id: 'link', label: '📨 Generate Candidate Link' },
  { id: 'config', label: '🔧 HR Configuration' },
  { id: 'help', label: '❓ Help' },
];

function HelpTab() {
  return (
    <div>
      <h2 className="text-xl font-bold mb-4">Help & How to Share with Employees</h2>
      <div className="space-y-6 text-sm leading-relaxed">
        <details open className="p-4 bg-gray-50 rounded-lg">
          <summary className="font-semibold text-base">🚀 First-time setup (one-time, takes 2 min)</summary>
          <ol className="list-decimal ml-6 mt-3 space-y-2">
            <li>Go to <strong>HR Configuration</strong> tab.</li>
            <li>Enter the report recipient email and paste your non-tech questions (one per line, or group under <code>## Topic</code> headers).</li>
            <li>Click Save. These HR settings will be reused from this browser for future interviews.</li>
          </ol>
        </details>
        <details className="p-4 bg-gray-50 rounded-lg">
          <summary className="font-semibold text-base">📤 How to share with employees (recommended workflow)</summary>
          <div className="mt-3 space-y-3">
            <ol className="list-decimal ml-6 mt-1 space-y-1">
              <li>Set up the recipient email + non-tech questions in <strong>HR Configuration</strong></li>
              <li>Go to <strong>Generate Candidate Link</strong>, paste JD + resume for one candidate, click Generate</li>
              <li>The backend generates personalized questions in the background. Copy the short link, send it to the candidate (email, Slack, anywhere)</li>
              <li>The candidate opens the link → goes straight to their interview, no setup on their end</li>
            </ol>
          </div>
        </details>
        <details className="p-4 bg-gray-50 rounded-lg">
          <summary className="font-semibold text-base">🎙️ How the interview works for the candidate</summary>
          <ol className="list-decimal ml-6 mt-3 space-y-2">
            <li>They enter their name, paste the JD and their resume, click Start.</li>
            <li>Browser will ask for microphone permission — they must allow it.</li>
            <li>The AI interviewer speaks each question aloud (US accent).</li>
            <li>The candidate clicks 🎤 to answer verbally. Click again when done.</li>
            <li>After all questions, they get a full report with strengths, weaknesses, action items, and a percentage score.</li>
            <li>They can download the PDF and share it with you.</li>
          </ol>
        </details>
        <details className="p-4 bg-gray-50 rounded-lg">
          <summary className="font-semibold text-base">🛠️ Troubleshooting</summary>
          <ul className="list-disc ml-6 mt-3 space-y-2">
            <li><strong>"Microphone not working"</strong> → Use Chrome or Edge. Firefox/Safari speech recognition is unreliable.</li>
            <li><strong>"No US voice"</strong> → Different OS gives different voices. Pick the best US voice from the dropdown in Config and test it.</li>
            <li><strong>"API error"</strong> → Means the backend couldn't reach Anthropic or the request was rejected. Check the API server logs.</li>
            <li><strong>Want to reset?</strong> Open browser DevTools → Application → Local Storage → clear this site's storage.</li>
          </ul>
        </details>
        <details className="p-4 bg-gray-50 rounded-lg">
          <summary className="font-semibold text-base">🔒 Privacy & data</summary>
          <p className="mt-3">
            JD, resume, and answers are stored on the backend and sent only to Anthropic for
            question generation and scoring. The candidate's short link contains no personal
            data — only an opaque session code that resolves server-side. Reports are produced
            by the backend.
          </p>
        </details>
      </div>
    </div>
  );
}

export function SetupScreen() {
  const navigate = useNavigate();
  const { config, updateInterview } = useAppState();
  const [active, setActive] = useState('link');
  const [phase, setPhase] = useState('setup');
  const [loadingSub, setLoadingSub] = useState('The backend is generating personalized questions');

  async function handleStartInterview({ name, role, jd, resume }) {
    if (!config.hrEmail) { alert('Please set the report recipient email in HR Configuration first.'); return; }
    if (!config.nonTechBank) { alert('Please add non-tech questions in HR Configuration first.'); return; }
    if (!name || !role || !jd || !resume) { alert('Please fill in all fields: name, role, JD, and resume.'); return; }

    setPhase('starting');
    setLoadingSub('The backend is generating personalized questions');
    try {
      const created = await createInterview({
        candidate_name: name,
        role_title: role,
        jd_text: jd,
        resume_text: resume,
        hr_email: config.hrEmail,
        include_intro: true,
        tech_count: config.techCount,
        behavioral_count: config.nonTechCount,
        pass_threshold: config.passThreshold,
        behavioral_bank: parseNonTechBank(config.nonTechBank),
      });
      await waitForPrimed(created.shortcode, {
        onTick: ({ state, attempt }) => setLoadingSub(
          state === 'pending' ? `Preparing personalized questions (attempt ${attempt})…` : `Status: ${state}`
        ),
      });
      const nonceInfo = await issueJoinNonce(created.shortcode);
      updateInterview({
        sessionId: created.id,
        shortcode: created.shortcode,
        candidateName: name,
        role,
        joinNonce: nonceInfo.join_nonce,
        wsPath: nonceInfo.ws_path,
        results: null,
        reportPdfUrl: '',
        reportPdfError: '',
      });
      navigate('/interview');
    } catch (e) {
      alert('Failed to start interview: ' + e.message);
      setPhase('setup');
    }
  }

  if (phase === 'starting') {
    return <LoadingScreen title="Preparing your interview..." subtitle={loadingSub} />;
  }

  return (
    <div className="card p-8">
      <Tabs tabs={TABS} active={active} onChange={setActive} />
      {active === 'link' && <GenerateLinkTab />}
      {active === 'config' && <HrConfigTab />}
      {active === 'help' && <HelpTab />}
      {active === 'start' && <StartInterviewTab onStart={handleStartInterview} busy={false} />}
    </div>
  );
}
