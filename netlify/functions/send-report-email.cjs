const RESEND_API_URL = 'https://api.resend.com/emails';

function json(statusCode, body) {
  return {
    statusCode,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body)
  };
}

function escapeHtml(value) {
  return String(value ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function plainLines(value) {
  return String(value || '')
    .split('\n')
    .map(line => line.trim())
    .filter(Boolean);
}

function listHtml(title, value) {
  const items = plainLines(value);
  if (!items.length) return '';
  return `
    <h3>${escapeHtml(title)}</h3>
    <ul>${items.map(item => `<li>${escapeHtml(item.replace(/^•\s*/, ''))}</li>`).join('')}</ul>
  `;
}

function buildEmailHtml(report) {
  return `
    <div style="font-family:Arial,sans-serif;color:#1f2937;line-height:1.5">
      <h2>Mock Interview Report</h2>
      <p><strong>Candidate:</strong> ${escapeHtml(report.candidateName)}</p>
      <p><strong>Role:</strong> ${escapeHtml(report.role)}</p>
      <p><strong>Overall:</strong> ${escapeHtml(report.overallScore)} (${escapeHtml(report.passFail)})</p>
      <p><strong>Technical:</strong> ${escapeHtml(report.technicalScore)} | <strong>Behavioral:</strong> ${escapeHtml(report.behavioralScore)}</p>
      <h3>Summary</h3>
      <p>${escapeHtml(report.summary)}</p>
      ${listHtml('Strengths', report.strengths)}
      ${listHtml('Weaknesses', report.weaknesses)}
      ${listHtml('Action Items', report.actionItems)}
      <h3>Full Report JSON</h3>
      <pre style="white-space:pre-wrap;background:#f3f4f6;padding:12px;border-radius:6px">${escapeHtml(report.fullReportJson)}</pre>
    </div>
  `;
}

function buildEmailText(report) {
  return [
    'Mock Interview Report',
    '',
    `Candidate: ${report.candidateName}`,
    `Role: ${report.role}`,
    `Overall: ${report.overallScore} (${report.passFail})`,
    `Technical: ${report.technicalScore}`,
    `Behavioral: ${report.behavioralScore}`,
    '',
    'Summary:',
    report.summary,
    '',
    'Strengths:',
    report.strengths,
    '',
    'Weaknesses:',
    report.weaknesses,
    '',
    'Action Items:',
    report.actionItems,
    '',
    'Full Report JSON:',
    report.fullReportJson
  ].join('\n');
}

exports.handler = async (event) => {
  if (event.httpMethod !== 'POST') {
    return json(405, { error: 'Method not allowed' });
  }

  const apiKey = process.env.RESEND_API_KEY;
  const from = process.env.REPORT_FROM_EMAIL;
  if (!apiKey || !from) {
    return json(500, { error: 'Email service is not configured. Set RESEND_API_KEY and REPORT_FROM_EMAIL in Netlify.' });
  }

  let report;
  try {
    report = JSON.parse(event.body || '{}');
  } catch (e) {
    return json(400, { error: 'Invalid JSON body' });
  }

  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(report.hrEmail || '')) {
    return json(400, { error: 'Invalid recipient email' });
  }

  const subject = `Mock Interview Report - ${report.candidateName || 'Candidate'} - ${report.role || 'Role'}`;
  const resp = await fetch(RESEND_API_URL, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${apiKey}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      from,
      to: [report.hrEmail],
      subject,
      html: buildEmailHtml(report),
      text: buildEmailText(report)
    })
  });

  const data = await resp.json().catch(() => ({}));
  if (!resp.ok) {
    return json(resp.status, { error: data.message || data.error || 'Email provider rejected the request' });
  }

  return json(200, { sent: true, id: data.id });
};
