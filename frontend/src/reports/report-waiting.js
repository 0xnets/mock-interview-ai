import { reportPdfUrl } from '../api/client.js';

/// The backend now renders the PDF on-demand from the stored report row, so
/// there is no asynchronous job to wait for after finalize() — the PDF URL
/// works as soon as finalize returns. We keep the same async signature so
/// callers don't need to change.
export async function awaitReportReady(sessionId) {
  return reportPdfUrl(sessionId);
}
