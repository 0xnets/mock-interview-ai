import { waitForReportReady, reportPdfUrl } from '../api/client.js';
import { showScreen } from '../ui/screens.js';

function setText(id, value) {
  const el = document.getElementById(id);
  if (el) el.textContent = value;
}

function setHidden(id, hidden) {
  const el = document.getElementById(id);
  if (el) el.classList.toggle('hidden', hidden);
}

/// Show the report-waiting screen, poll status, then return the PDF URL once ready.
/// Caller decides what to do with the URL (open in tab, store on state, etc).
export async function awaitReportReady(sessionId) {
  showScreen('report-waiting');
  setHidden('reportWaitingError', true);
  setText('reportWaitingStatus', 'PDF: pending · Mail: pending');
  try {
    await waitForReportReady(sessionId, {
      onTick: ({ status }) => {
        setText('reportWaitingStatus', `PDF: ${status.pdf_status} · Mail: ${status.mail_status}`);
      },
    });
    return reportPdfUrl(sessionId);
  } catch (e) {
    const err = document.getElementById('reportWaitingError');
    if (err) {
      err.textContent = e.message || 'Report failed.';
      err.classList.remove('hidden');
    }
    throw e;
  }
}
