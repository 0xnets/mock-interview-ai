import { state } from '../app/state.js';
import { showScreen } from '../ui/screens.js';
import { ensureVoicesLoaded } from '../voice/voice-selection.js';
import { startCandidateInterviewWithSocket } from './interview-flow.js';

/// Called from bootstrap when the URL has `?session=…`. The codec has already
/// fetched the session metadata and minted a single-use WS join nonce.
export async function enterCandidateMode(loaded) {
  const { session, nonce, wsPath, shortcode } = loaded;

  state.session.sessionId = session.id || '';
  state.session.shortcode = shortcode || '';
  state.session.candidateName = session.candidate_name;
  state.session.role = session.role_title;
  state.session.joinNonce = nonce;
  state.session.wsPath = wsPath || '/v1/ws/interview';
  state.session.totalQuestions = 0;
  state.session.lastSection = null;
  state.session.currentOrdinal = null;
  state.session.utteranceSeq = 0;
  state.session.results = null;
  state.session.currentQuestionText = '';
  state.session.reportPdfUrl = '';
  state.session.socket = null;

  // Hide HR settings button — candidate must not edit config.
  const settingsBtn = document.getElementById('settingsBtn');
  if (settingsBtn) settingsBtn.style.display = 'none';

  document.getElementById('welcomeName').textContent = session.candidate_name;
  document.getElementById('welcomeRole').textContent = session.role_title;

  // The candidate doesn't know how many questions yet — that arrives in the
  // `hello` frame. Show a generic count for now; the live progress label
  // updates as questions come in.
  const countEl = document.getElementById('welcomeQuestionCount');
  if (countEl) countEl.textContent = 'a personalized set of';

  showScreen('welcome');
  await ensureVoicesLoaded();
}

/// Invoked from the welcome screen's Start button.
export function startCandidateInterview() {
  startCandidateInterviewWithSocket();
}
