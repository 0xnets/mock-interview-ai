import { state } from '../app/state.js';
import { ENV, envNumber } from '../app/env.js';

export async function emailReportToHR(results) {
  const hrEmail = state.config.hrEmail;
  if (!hrEmail) return { sent: false, reason: 'No HR email configured' };

  const passed = results.overall_percentage >= (state.config.passThreshold || envNumber('DEFAULT_PASS_THRESHOLD'));
  const bullets = (arr) => Array.isArray(arr) ? arr.map(s => '• ' + s).join('\n') : '';

  const formData = new URLSearchParams();
  formData.append('form-name', ENV.NETLIFY_FORM_NAME);
  formData.append(ENV.NETLIFY_HONEYPOT_FIELD, '');
  formData.append('hr_email', hrEmail);
  formData.append('candidate_name', state.session.candidateName);
  formData.append('role', state.session.role);
  formData.append('overall_score', `${Math.round(results.overall_percentage)}%`);
  formData.append('pass_fail', passed ? 'PASSED' : 'FAILED');
  formData.append('technical_score', `${Math.round(results.technical_score)}%`);
  formData.append('behavioral_score', `${Math.round(results.behavioral_score)}%`);
  formData.append('summary', results.summary || '');
  formData.append('strengths', bullets(results.strengths));
  formData.append('weaknesses', bullets(results.weaknesses));
  formData.append('action_items', bullets(results.action_items));
  formData.append('full_report_json', JSON.stringify(results, null, 2));

  try {
    const resp = await fetch(ENV.NETLIFY_FORM_POST_URL, {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: formData.toString()
    });
    if (resp.ok || resp.status === 200) return { sent: true };
    return { sent: false, reason: 'Netlify returned ' + resp.status };
  } catch (e) {
    return { sent: false, reason: e.message };
  }
}

// =============== NON-TECH QUESTION PICKING ===============
// Parse a non-tech bank text into categories.
// "## Category Name" lines start a new category. Lines below are that category's questions.
// If no "##" headers exist at all, returns {_default: [...all lines...]}.
