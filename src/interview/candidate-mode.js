import { state } from '../app/state.js';
import { envNumber } from '../app/env.js';
import { showScreen } from '../ui/screens.js';
import { ensureVoicesLoaded } from '../voice/voice-selection.js';
import { callAI } from '../services/gemini.service.js';
import { extractJSON, normalizeQuestionArray } from '../utils/json.js';
import { pickNonTechQuestions } from './non-tech-bank.js';
import { runQuestion } from './interview-flow.js';

export async function enterCandidateMode(sessionData) {
  // Populate session — handles both new format (pre-generated questions)
  // and legacy format (JD/resume in URL).
  state.session.candidateName = sessionData.name;
  state.session.role = sessionData.role;
  state.session.jd = sessionData.jd || '';
  state.session.resume = sessionData.resume || '';
  state.session.preparedTechQuestions = sessionData.techQuestions || [];
  state.session.preparedNonTechQuestions = sessionData.nonTechQuestions || [];
  state.session.scoringContext = sessionData.scoringContext || '';

  // Hide HR settings button
  const settingsBtn = document.getElementById('settingsBtn');
  if (settingsBtn) settingsBtn.style.display = 'none';

  // Populate welcome screen
  document.getElementById('welcomeName').textContent = sessionData.name;
  document.getElementById('welcomeRole').textContent = sessionData.role;
  const total = state.session.preparedTechQuestions.length > 0
    ? state.session.preparedTechQuestions.length + state.session.preparedNonTechQuestions.length
    : (state.config.techCount || envNumber('DEFAULT_TECH_QUESTION_COUNT')) + (state.config.nonTechCount || envNumber('DEFAULT_NON_TECH_QUESTION_COUNT'));
  document.getElementById('welcomeQuestionCount').textContent = total;

  showScreen('welcome');
  await ensureVoicesLoaded();
}

export async function startCandidateInterview() {
  if (!state.config.apiKey) {
    alert('This interview link is not fully configured. Please contact the HR team.');
    return;
  }

  state.session.allQuestions = [];
  state.session.currentIdx = 0;
  state.session.isListening = false;
  state.session.finalTranscript = '';
  state.session.results = null;

  try {
    // NEW PATH: questions were pre-generated when HR created the link
    if (state.session.preparedTechQuestions.length > 0) {
      state.session.allQuestions = [
        ...state.session.preparedTechQuestions.map(q => ({ question: q, section: 'Technical', answer: '' })),
        ...state.session.preparedNonTechQuestions.map(q => ({ question: q, section: 'Behavioral', answer: '' }))
      ];
      showScreen('interview');
      document.getElementById('interviewSubtitle').textContent = `Candidate: ${state.session.candidateName} — Role: ${state.session.role}`;
      await runQuestion(0);
      return;
    }

    // LEGACY PATH: old-format link with JD/resume in URL — generate questions on the fly
    if (!state.config.nonTechBank) {
      state.config.nonTechBank = [
        "Tell me about a time you handled a difficult teammate.",
        "Describe a project where you took ownership beyond your role.",
        "How do you prioritize when everything feels urgent?",
        "Walk me through a mistake you made and what you learned.",
        "Tell me about a time you had to learn something new quickly.",
        "Describe a time you disagreed with a manager. How did you handle it?"
      ].join('\n');
    }

    showScreen('loading');
    document.getElementById('loadingTitle').textContent = 'Preparing your interview...';
    document.getElementById('loadingSub').textContent = 'Analyzing the JD and resume to craft personalized questions';

    const techSys = `You are an expert technical interviewer. Generate exactly ${state.config.techCount} technical interview questions tailored to the candidate's resume and the job description. Keep each question CONCISE — 1 to 2 sentences max. Return ONLY a JSON array of question strings.`;
    const techUser = `JOB DESCRIPTION:\n${state.session.jd}\n\nCANDIDATE RESUME:\n${state.session.resume}\n\nGenerate ${state.config.techCount} concise technical interview questions.`;
    const techSchema = { type: 'ARRAY', items: { type: 'STRING' } };
    const techRes = await callAI(techSys, techUser, envNumber('GEMINI_DEFAULT_MAX_OUTPUT_TOKENS'), techSchema, false);
    const techQs = normalizeQuestionArray(extractJSON(techRes));
    if (techQs.length === 0) throw new Error('No tech questions returned. Try again.');

    const nonTechQs = pickNonTechQuestions(state.config.nonTechBank, state.config.nonTechCount);

    state.session.allQuestions = [
      ...techQs.slice(0, state.config.techCount).map(q => ({ question: q, section: 'Technical', answer: '' })),
      ...nonTechQs.map(q => ({ question: q, section: 'Behavioral', answer: '' }))
    ];

    showScreen('interview');
    document.getElementById('interviewSubtitle').textContent = `Candidate: ${state.session.candidateName} — Role: ${state.session.role}`;
    await runQuestion(0);
  } catch (e) {
    alert('Failed to start interview: ' + e.message);
    showScreen('welcome');
  }
}

// =============== DEPLOY FILE GENERATOR ===============
