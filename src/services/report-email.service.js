import { state } from '../app/state.js';
import { ENV, envNumber } from '../app/env.js';

export async function emailReportToHR(results) {
  const hrEmail = state.config.hrEmail;
  if (!hrEmail) return { sent: false, reason: 'No HR email configured' };

  const passed = results.overall_percentage >= (state.config.passThreshold || envNumber('DEFAULT_PASS_THRESHOLD'));
  const bullets = (arr) => Array.isArray(arr) ? arr.map(s => '* ' + s).join('\n') : '';

  const reportPayload = {
    hrEmail,
    candidateName: state.session.candidateName,
    role: state.session.role,
    overallScore: `${Math.round(results.overall_percentage)}%`,
    passFail: passed ? 'PASSED' : 'FAILED',
    technicalScore: `${Math.round(results.technical_score)}%`,
    behavioralScore: `${Math.round(results.behavioral_score)}%`,
    summary: results.summary || '',
    strengths: bullets(results.strengths),
    weaknesses: bullets(results.weaknesses),
    actionItems: bullets(results.action_items),
    fullReportJson: JSON.stringify(results, null, 2)
  };

  let resendFailureReason = '';
  try {
    const resp = await fetch(ENV.REPORT_EMAIL_FUNCTION_URL, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(reportPayload)
    });
    const data = await resp.json().catch(() => ({}));
    if (resp.ok) return { sent: true, recipient: hrEmail };
    resendFailureReason = data.error || `Email function returned ${resp.status}`;
  } catch (e) {
    resendFailureReason = e.message;
  }

  try {
    const fallbackStored = await submitNetlifyFallback(reportPayload);
    return {
      sent: false,
      recipient: hrEmail,
      fallbackStored,
      reason: fallbackStored
        ? `Resend failed (${resendFailureReason}); submitted to Netlify Forms fallback`
        : `Resend failed (${resendFailureReason}); Netlify Forms fallback also failed`
    };
  } catch (e) {
    return {
      sent: false,
      recipient: hrEmail,
      fallbackStored: false,
      reason: `Resend failed (${resendFailureReason}); Netlify Forms fallback failed (${e.message})`
    };
  }
}

async function submitNetlifyFallback(reportPayload) {
  const formData = new URLSearchParams();
  formData.append('form-name', ENV.NETLIFY_FORM_NAME);
  formData.append(ENV.NETLIFY_HONEYPOT_FIELD, '');
  formData.append('hr_email', reportPayload.hrEmail);
  formData.append('candidate_name', reportPayload.candidateName);
  formData.append('role', reportPayload.role);
  formData.append('overall_score', reportPayload.overallScore);
  formData.append('pass_fail', reportPayload.passFail);
  formData.append('technical_score', reportPayload.technicalScore);
  formData.append('behavioral_score', reportPayload.behavioralScore);
  formData.append('summary', reportPayload.summary);
  formData.append('strengths', reportPayload.strengths);
  formData.append('weaknesses', reportPayload.weaknesses);
  formData.append('action_items', reportPayload.actionItems);
  formData.append('full_report_json', reportPayload.fullReportJson);

  const resp = await fetch(ENV.NETLIFY_FORM_POST_URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: formData.toString()
  });
  return resp.ok || resp.status === 200;
}
