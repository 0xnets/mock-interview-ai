import { state } from '../app/state.js';
import { ENV, envNumber } from '../app/env.js';
import { showScreen } from './screens.js';
import { escapeHtml } from '../utils/html.js';

export function showResults() {
  showScreen('results');
  const r = state.session.results;
  const passed = r.overall_percentage >= state.config.passThreshold;
  const ringColor = passed ? ENV.RESULT_PASS_COLOR : (r.overall_percentage >= envNumber('RESULT_WARNING_SCORE_THRESHOLD') ? ENV.RESULT_WARNING_COLOR : ENV.RESULT_FAIL_COLOR);
  const circ = 2 * Math.PI * envNumber('RESULT_RING_RADIUS');
  const offset = circ - (r.overall_percentage / 100) * circ;

  document.getElementById('resultsContent').innerHTML = `
    <div class="text-center mb-8">
      <h2 class="text-3xl font-bold mb-1">Interview Results</h2>
      <p class="text-gray-600">${state.session.candidateName} — ${state.session.role}</p>
    </div>

    <div class="flex flex-col md:flex-row items-center gap-8 mb-8 justify-center">
      <div class="relative">
        <svg width="180" height="180" class="score-ring">
          <circle cx="90" cy="90" r="${envNumber('RESULT_RING_RADIUS')}" stroke="#e5e7eb" stroke-width="14" fill="none" />
          <circle cx="90" cy="90" r="${envNumber('RESULT_RING_RADIUS')}" stroke="${ringColor}" stroke-width="14" fill="none"
            stroke-dasharray="${circ}" stroke-dashoffset="${offset}" stroke-linecap="round"
            style="transition: stroke-dashoffset 1.5s ease-out;" />
        </svg>
        <div class="absolute inset-0 flex flex-col items-center justify-center">
          <div class="text-4xl font-bold">${Math.round(r.overall_percentage)}%</div>
          <div class="text-xs text-gray-500 uppercase tracking-wide">Overall</div>
        </div>
      </div>
      <div class="text-center md:text-left">
        <span class="inline-block px-4 py-2 rounded-full text-sm font-semibold ${passed ? 'badge-pass' : 'badge-fail'} mb-3">
          ${passed ? '✓ PASSED — Advance to human round' : `✗ Below ${state.config.passThreshold}% threshold`}
        </span>
        <div class="grid grid-cols-2 gap-4">
          <div class="bg-gray-50 rounded-lg p-3">
            <div class="text-xs text-gray-500 uppercase">Technical</div>
            <div class="text-2xl font-bold">${Math.round(r.technical_score)}%</div>
          </div>
          <div class="bg-gray-50 rounded-lg p-3">
            <div class="text-xs text-gray-500 uppercase">Behavioral</div>
            <div class="text-2xl font-bold">${Math.round(r.behavioral_score)}%</div>
          </div>
        </div>
      </div>
    </div>

    <div class="bg-blue-50 border border-blue-200 rounded-lg p-4 mb-6">
      <p class="text-sm text-blue-900">${r.summary}</p>
    </div>

    ${state.session.emailResult ? `
      <div class="text-center text-xs ${state.session.emailResult.sent ? 'text-green-700' : 'text-gray-500'} mb-4">
        ${state.session.emailResult.sent
          ? '✓ Report emailed to HR'
          : `ℹ️ Email to HR not sent (${state.session.emailResult.reason}). Report is still visible above.`}
      </div>` : ''}

    <div class="grid md:grid-cols-3 gap-4 mb-6">
      <div class="bg-green-50 border border-green-200 rounded-lg p-4">
        <h3 class="font-bold text-green-900 mb-2">💪 Strengths</h3>
        <ul class="text-sm text-green-800 space-y-2 list-disc ml-4">
          ${r.strengths.map(s => `<li>${escapeHtml(s)}</li>`).join('')}
        </ul>
      </div>
      <div class="bg-amber-50 border border-amber-200 rounded-lg p-4">
        <h3 class="font-bold text-amber-900 mb-2">⚠️ Weaknesses</h3>
        <ul class="text-sm text-amber-800 space-y-2 list-disc ml-4">
          ${r.weaknesses.map(s => `<li>${escapeHtml(s)}</li>`).join('')}
        </ul>
      </div>
      <div class="bg-indigo-50 border border-indigo-200 rounded-lg p-4">
        <h3 class="font-bold text-indigo-900 mb-2">🎯 Action Items for Next Round</h3>
        <ul class="text-sm text-indigo-800 space-y-2 list-disc ml-4">
          ${r.action_items.map(s => `<li>${escapeHtml(s)}</li>`).join('')}
        </ul>
      </div>
    </div>

    <details class="mb-6 bg-gray-50 rounded-lg p-4">
      <summary class="font-semibold cursor-pointer">📝 Per-question breakdown</summary>
      <div class="mt-4 space-y-3">
        ${r.per_question.map((pq, i) => `
          <div class="bg-white rounded-lg p-3 border border-gray-200">
            <div class="flex justify-between items-start mb-1">
              <span class="text-xs uppercase font-bold text-gray-500">${pq.section} • Q${pq.q}</span>
              <span class="text-sm font-bold ${pq.score >= 85 ? 'text-green-600' : pq.score >= 65 ? 'text-amber-600' : 'text-red-600'}">${Math.round(pq.score)}%</span>
            </div>
            <div class="text-sm font-medium mb-1">${escapeHtml(state.session.allQuestions[i]?.question || '')}</div>
            <div class="text-xs text-gray-600 italic mb-2">Your answer: ${escapeHtml((state.session.allQuestions[i]?.answer || '').substring(0, 200))}${(state.session.allQuestions[i]?.answer||'').length > 200 ? '...' : ''}</div>
            <div class="text-xs text-gray-700">💬 ${escapeHtml(pq.feedback)}</div>
          </div>
        `).join('')}
      </div>
    </details>

    <div class="flex flex-wrap gap-3 justify-center">
      <button id="downloadPdfBtn" class="btn-primary">📄 Download PDF Report</button>
      <button id="newInterviewBtn" class="btn-secondary">🔄 New Interview</button>
    </div>
  `;
}
