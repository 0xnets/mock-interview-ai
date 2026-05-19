import { state } from '../app/state.js';
import { showScreen } from '../ui/screens.js';
import { parseNonTechBank } from './non-tech-bank.js';
import { speak } from '../voice/speech-synthesis.js';
import { toggleMic } from '../voice/speech-recognition.js';
import { showResults } from '../ui/render-results.js';
import {
  createInterview,
  waitForPrimed,
  issueJoinNonce,
  finalizeInterview as finalizeOnBackend,
} from '../api/client.js';
import { awaitReportReady } from '../reports/report-waiting.js';
import { closeSocket, openInterviewSocket, sendMsg, sendSkip } from '../realtime/ws-client.js';
import { isAuthenticated } from '../app/auth-store.js';

export async function startInterview() {
  const name = document.getElementById('candidateName').value.trim();
  const role = document.getElementById('rolePosition').value.trim();
  const jd = document.getElementById('jdInput').value.trim();
  const resume = document.getElementById('resumeInput').value.trim();

  if (!state.config.hrEmail) { alert('Please set the report recipient email in HR Configuration first.'); return; }
  if (!state.config.nonTechBank) { alert('Please add non-tech questions in HR Configuration first.'); return; }
  if (!name || !role || !jd || !resume) { alert('Please fill in all fields: name, role, JD, and resume.'); return; }

  resetSessionState({ candidateName: name, role });

  showScreen('loading');
  document.getElementById('loadingTitle').textContent = 'Preparing your interview...';
  document.getElementById('loadingSub').textContent = 'The backend is generating personalized questions';

  try {
    const created = await createInterview({
      candidate_name: name,
      role_title: role,
      jd_text: jd,
      resume_text: resume,
      hr_email: state.config.hrEmail,
      include_intro: true,
      tech_count: state.config.techCount,
      behavioral_count: state.config.nonTechCount,
      pass_threshold: state.config.passThreshold,
      behavioral_bank: parseNonTechBank(state.config.nonTechBank),
    });

    state.session.sessionId = created.id;
    state.session.shortcode = created.shortcode;

    await waitForPrimed(created.shortcode, {
      onTick: ({ state: s, attempt }) => {
        document.getElementById('loadingSub').textContent =
          s === 'pending' ? `Preparing personalized questions (attempt ${attempt})…` : `Status: ${s}`;
      },
    });

    const nonceInfo = await issueJoinNonce(created.shortcode);
    state.session.joinNonce = nonceInfo.join_nonce;
    state.session.wsPath = nonceInfo.ws_path;

    enterInterviewScreen();
    connectInterviewWebSocket();
  } catch (e) {
    alert('Failed to start interview: ' + e.message);
    showScreen('setup');
  }
}

/// Used by the candidate-link path. Caller has already populated
/// `state.session.{sessionId,shortcode,candidateName,role,joinNonce,wsPath}`.
export function startCandidateInterviewWithSocket() {
  enterInterviewScreen();
  connectInterviewWebSocket();
}

function resetSessionState({ candidateName, role }) {
  state.session.sessionId = '';
  state.session.shortcode = '';
  state.session.candidateName = candidateName;
  state.session.role = role;
  state.session.isListening = false;
  state.session.finalTranscript = '';
  state.session.interimTranscript = '';
  state.session.socket = null;
  state.session.joinNonce = '';
  state.session.currentOrdinal = null;
  state.session.utteranceSeq = 0;
  state.session.questionStartedAt = 0;
  state.session.totalQuestions = 0;
  state.session.lastSection = null;
  state.session.currentQuestionText = '';
  state.session.reportPdfUrl = '';
  state.session.reportPdfError = '';
  state.session.results = null;
}

function enterInterviewScreen() {
  showScreen('interview');
  document.getElementById('interviewSubtitle').textContent =
    `Candidate: ${state.session.candidateName} — Role: ${state.session.role}`;
  document.getElementById('sectionLabel').textContent = 'Section: —';
  document.getElementById('qProgress').textContent = 'Connecting…';
  document.getElementById('currentQuestion').textContent = 'Connecting to interviewer…';
  document.getElementById('transcript').textContent = '—';
  document.getElementById('transcript').classList.add('italic');
  document.getElementById('submitBtn').disabled = true;
  document.getElementById('micBtn').disabled = true;
  document.getElementById('micStatus').textContent = '';
}

function connectInterviewWebSocket() {
  const socket = openInterviewSocket({
    joinNonce: state.session.joinNonce,
    wsPath: state.session.wsPath,
    onOpen: (sock) => {
      sendMsg(sock, { t: 'hello', client_version: 'web/phase4' });
    },
    onMessage: handleServerMsg,
    onClose: handleSocketClose,
    onError: (e) => console.error('WS error', e),
  });
  state.session.socket = socket;
}

function handleSocketClose() {
  if (state.session.results) return; // expected close after results
  console.warn('Interview WS closed before completion');
}

async function handleServerMsg(msg) {
  switch (msg.t) {
    case 'hello':
      state.session.totalQuestions = msg.total_questions || 0;
      break;
    case 'question':
    case 'followup':
      await renderQuestionFromServer(msg);
      break;
    case 'state':
      handleStateChange(msg.value);
      break;
    case 'results_ready':
      await finalizeAndShow();
      break;
    case 'error':
      console.error('Server error frame', msg);
      alert(`Backend error: ${msg.message || msg.code || 'unknown'}`);
      if (msg.terminal) {
        closeSocket(state.session.socket);
        state.session.socket = null;
        // If the candidate was running through a candidate-link flow they can re-join via the link.
        // If an HR session expired, route to login. Otherwise drop to setup.
        if (!isAuthenticated() && state.session.shortcode) {
          showScreen('welcome');
        } else if (!isAuthenticated()) {
          showScreen('login');
        } else {
          showScreen('setup');
        }
      }
      break;
    default:
      break;
  }
}

async function renderQuestionFromServer(msg) {
  const ordinal = msg.ordinal;
  state.session.currentOrdinal = ordinal;

  const section = msg.t === 'followup'
    ? 'Follow-up'
    : (msg.kind === 'intro' ? 'Intro' : msg.kind === 'technical' ? 'Technical' : 'Behavioral');

  const total = Math.max(state.session.totalQuestions, ordinal);
  document.getElementById('sectionLabel').textContent = 'Section: ' + section;
  document.getElementById('qProgress').textContent = `Question ${ordinal} of ${total}`;
  document.getElementById('progressBar').style.width = (((ordinal - 1) / total) * 100) + '%';
  document.getElementById('currentQuestion').textContent = msg.text;
  state.session.currentQuestionText = msg.text;
  document.getElementById('transcript').textContent = '—';
  document.getElementById('transcript').classList.add('italic');
  document.getElementById('submitBtn').disabled = true;
  document.getElementById('micBtn').disabled = true;
  document.getElementById('micStatus').textContent = '🔊 Listen to the question...';

  state.session.finalTranscript = '';
  state.session.interimTranscript = '';

  const preamble = computePreamble(section, ordinal);
  state.session.lastSection = section;

  await speak(preamble + msg.text);

  state.session.questionStartedAt = Date.now();
  document.getElementById('micBtn').disabled = false;
  document.getElementById('micStatus').textContent = 'Click the microphone when ready to answer.';
}

function computePreamble(section, ordinal) {
  if (ordinal === 1 || section === 'Intro') {
    return `Hello ${state.session.candidateName}. Welcome to your mock interview. To get started, `;
  }
  if (section === 'Follow-up') {
    return `One follow-up on that. `;
  }
  if (state.session.lastSection && state.session.lastSection !== section) {
    if (section === 'Behavioral') return `Great. Now let's move to the behavioral questions. `;
    if (section === 'Technical') return `Now let's move to the technical questions. `;
  }
  return '';
}

function handleStateChange(value) {
  if (value === 'thinking') {
    document.getElementById('micStatus').textContent = '🤔 Interviewer is preparing a follow-up…';
    document.getElementById('micBtn').disabled = true;
    document.getElementById('submitBtn').disabled = true;
    const tx = document.getElementById('transcript');
    if (tx && (tx.textContent === '—' || tx.classList.contains('italic'))) {
      tx.textContent = 'Generating follow-up…';
    }
  }
}

export function submitAnswer() {
  const answer = (state.session.finalTranscript + state.session.interimTranscript).trim();
  if (!answer) { alert('Please record an answer first.'); return; }
  if (state.session.isListening) toggleMic();

  const ordinal = state.session.currentOrdinal;
  if (ordinal == null) return;

  if (answer && state.session.socket) {
    sendMsg(state.session.socket, {
      t: 'utterance',
      seq: ++state.session.utteranceSeq,
      ordinal,
      text: answer,
      is_final: true,
    });
  }

  const duration = Math.max(0, Date.now() - state.session.questionStartedAt);
  sendMsg(state.session.socket, {
    t: 'submit',
    ordinal,
    duration_ms: duration,
  });

  document.getElementById('submitBtn').disabled = true;
  document.getElementById('micBtn').disabled = true;
  document.getElementById('micStatus').textContent = 'Submitting…';
}

async function finalizeAndShow() {
  if (state.session.isListening) toggleMic();
  showScreen('loading');
  document.getElementById('loadingTitle').textContent = 'Scoring your interview...';
  document.getElementById('loadingSub').textContent = 'Rolling up your per-question grades into a final report';

  try {
    if (!state.session.sessionId) throw new Error('Session id is missing — cannot finalize.');
    const { report } = await finalizeOnBackend(state.session.sessionId);
    const results = {
      overall_percentage: report.overall_percentage,
      technical_score: report.technical_score,
      behavioral_score: report.behavioral_score,
      passed: report.passed,
      summary: report.summary,
      strengths: report.strengths,
      weaknesses: report.weaknesses,
      action_items: report.action_items,
      per_question: report.per_question,
    };
    state.session.results = results;
    state.session.reportPdfUrl = '';
    state.session.reportPdfError = '';

    closeSocket(state.session.socket);
    state.session.socket = null;

    awaitReportReady(state.session.sessionId)
      .then(url => { state.session.reportPdfUrl = url; })
      .catch(e => {
        state.session.reportPdfError = e?.message || 'PDF report generation failed.';
      })
      .finally(() => {
        showResults();
      });
  } catch (e) {
    alert('Failed to score interview: ' + e.message);
    showScreen('setup');
  }
}

export function skipQuestion() {
  const ordinal = state.session.currentOrdinal;
  if (ordinal == null || !state.session.socket) return;
  if (state.session.isListening) toggleMic();
  sendSkip(state.session.socket, ordinal);
  document.getElementById('submitBtn').disabled = true;
  document.getElementById('micBtn').disabled = true;
  document.getElementById('micStatus').textContent = 'Skipping…';
}

export function abortInterview() {
  if (confirm('Cancel this interview? All progress will be lost.')) {
    if (state.session.isListening) toggleMic();
    if ('speechSynthesis' in window) speechSynthesis.cancel();
    closeSocket(state.session.socket);
    state.session.socket = null;
    showScreen('setup');
  }
}
