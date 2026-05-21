import { useState } from 'react';
import { Tabs } from '../components/Tabs.jsx';
import { GenerateLinkTab } from '../components/GenerateLinkTab.jsx';
import { HrConfigTab } from '../components/HrConfigTab.jsx';

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
            <li>Click Save. These HR settings are saved to your account and reused for future interviews — on any device you sign in from.</li>
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
            <li><strong>Want to change settings?</strong> Open the HR Configuration tab and click Edit to update your account's saved configuration.</li>
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
  const [active, setActive] = useState('link');

  return (
    <div className="card p-8">
      <Tabs tabs={TABS} active={active} onChange={setActive} />
      {active === 'link' && <GenerateLinkTab />}
      {active === 'config' && <HrConfigTab />}
      {active === 'help' && <HelpTab />}
    </div>
  );
}
