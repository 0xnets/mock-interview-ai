import { state } from '../app/state.js';
import { envNumber } from '../app/env.js';
import { showScreen } from '../ui/screens.js';
import { callAI } from '../services/gemini.service.js';
import { extractJSON, normalizeQuestionArray } from '../utils/json.js';
import { pickNonTechQuestions } from './non-tech-bank.js';
import { speak } from '../voice/speech-synthesis.js';
import { toggleMic } from '../voice/speech-recognition.js';
import { emailReportToHR } from '../services/netlify-forms.service.js';
import { showResults } from '../ui/render-results.js';

export async function startInterview() {
  const name = document.getElementById('candidateName').value.trim();
  const role = document.getElementById('rolePosition').value.trim();
  const jd = document.getElementById('jdInput').value.trim();
  const resume = document.getElementById('resumeInput').value.trim();

  if (!state.config.apiKey) { alert('Please set your API key in HR Configuration first.'); return; }
  if (!state.config.nonTechBank) { alert('Please add non-tech questions in HR Configuration first.'); return; }
  if (!name || !role || !jd || !resume) { alert('Please fill in all fields: name, role, JD, and resume.'); return; }

  state.session = {
    candidateName: name, role, jd, resume,
    techQuestions: [], nonTechQuestions: [], allQuestions: [],
    currentIdx: 0, isListening: false,
    finalTranscript: '', interimTranscript: '',
    results: null
  };

  showScreen('loading');
  document.getElementById('loadingTitle').textContent = 'Preparing your interview...';
  document.getElementById('loadingSub').textContent = 'Analyzing the JD and resume to craft personalized questions';

  try {
    // Generate tech questions — JD-focused
    const techSys = `You are an expert technical interviewer. Generate exactly ${state.config.techCount} technical interview questions. The Job Description is your PRIMARY source — focus on the technologies, skills, and responsibilities listed there. Use the resume only to calibrate difficulty. Keep each question CONCISE — 1 to 2 sentences max. Return ONLY a JSON array of question strings.`;
    const techUser = `JOB DESCRIPTION (primary focus):\n${state.session.jd}\n\nCANDIDATE RESUME (for context only):\n${state.session.resume}\n\nGenerate ${state.config.techCount} concise technical interview questions, prioritizing JD content.`;
    const techSchema = { type: 'ARRAY', items: { type: 'STRING' } };
    const techRes = await callAI(techSys, techUser, envNumber('GEMINI_DEFAULT_MAX_OUTPUT_TOKENS'), techSchema, false);
    const techQs = normalizeQuestionArray(extractJSON(techRes));
    if (techQs.length === 0) throw new Error('No tech questions returned by the AI. Try again, or check your JD/resume aren\'t empty.');
    state.session.techQuestions = techQs.slice(0, state.config.techCount);

    // Pick non-tech questions (1 per category if categorized, else random N)
    state.session.nonTechQuestions = pickNonTechQuestions(state.config.nonTechBank, state.config.nonTechCount);

    // Combine
    state.session.allQuestions = [
      ...state.session.techQuestions.map(q => ({ question: q, section: 'Technical', answer: '' })),
      ...state.session.nonTechQuestions.map(q => ({ question: q, section: 'Behavioral', answer: '' }))
    ];

    // Start
    showScreen('interview');
    document.getElementById('interviewSubtitle').textContent = `Candidate: ${name} — Role: ${role}`;
    await runQuestion(0);
  } catch (e) {
    alert('Failed to start interview: ' + e.message);
    showScreen('setup');
  }
}

export async function runQuestion(idx) {
  state.session.currentIdx = idx;
  const q = state.session.allQuestions[idx];
  document.getElementById('sectionLabel').textContent = 'Section: ' + q.section;
  document.getElementById('qProgress').textContent = `Question ${idx + 1} of ${state.session.allQuestions.length}`;
  document.getElementById('progressBar').style.width = ((idx) / state.session.allQuestions.length * 100) + '%';
  document.getElementById('currentQuestion').textContent = q.question;
  document.getElementById('transcript').textContent = '—';
  document.getElementById('transcript').classList.add('italic');
  document.getElementById('submitBtn').disabled = true;
  document.getElementById('micBtn').disabled = true;
  document.getElementById('micStatus').textContent = '🔊 Listen to the question...';

  state.session.finalTranscript = '';
  state.session.interimTranscript = '';

  // Build preamble based on section transitions (not on question count, which can be 0
  // in pre-generated sessions and cause incorrect intros).
  let preamble = '';
  const prevSection = idx > 0 ? state.session.allQuestions[idx - 1].section : null;
  const currSection = q.section;
  if (idx === 0) {
    const firstLabel = currSection === 'Technical' ? 'technical section' : 'behavioral section';
    preamble = `Hello ${state.session.candidateName}. Welcome to your mock interview. Let's start with the ${firstLabel}. `;
  } else if (prevSection && prevSection !== currSection) {
    if (currSection === 'Behavioral') {
      preamble = `Great. Now let's move to the behavioral questions. `;
    } else if (currSection === 'Technical') {
      preamble = `Now let's move to the technical questions. `;
    }
  }

  await speak(preamble + q.question);

  document.getElementById('micBtn').disabled = false;
  document.getElementById('micStatus').textContent = 'Click the microphone when ready to answer.';
}

export function submitAnswer() {
  const answer = (state.session.finalTranscript + state.session.interimTranscript).trim();
  if (!answer) { alert('Please record an answer first.'); return; }
  if (state.session.isListening) toggleMic();
  state.session.allQuestions[state.session.currentIdx].answer = answer;

  if (state.session.currentIdx + 1 < state.session.allQuestions.length) {
    runQuestion(state.session.currentIdx + 1);
  } else {
    finalizeInterview();
  }
}

export async function finalizeInterview() {
  if (state.session.isListening) toggleMic();
  showScreen('loading');
  document.getElementById('loadingTitle').textContent = 'Scoring your interview...';
  document.getElementById('loadingSub').textContent = 'Analyzing your answers — this takes 20-30 seconds';

  try {
    const transcript = state.session.allQuestions.map((q, i) =>
      `[${q.section} #${i+1}]\nQ: ${q.question}\nA: ${q.answer || '(no answer)'}\n`
    ).join('\n');

    const sys = `You are an expert technical interviewer scoring a mock interview. You will score the candidate fairly and rigorously.

You will return ONLY a valid JSON object with this exact structure (no other text):
{
  "overall_percentage": <0-100 number>,
  "technical_score": <0-100>,
  "behavioral_score": <0-100>,
  "strengths": ["...", "...", "..."],
  "weaknesses": ["...", "...", "..."],
  "action_items": ["...", "...", "..."],
  "per_question": [
    {"q": <question number>, "section": "Technical"|"Behavioral", "score": <0-100>, "feedback": "short feedback"},
    ...
  ],
  "summary": "2-3 sentence overall summary"
}

Scoring guidance:
- Be rigorous. Score correctness, depth, clarity, and relevance.
- A score of 90% means truly excellent — strong technical depth, clear communication, well-structured answers.
- Penalize vague, off-topic, or shallow answers.
- For technical answers, weigh accuracy heavily.
- For behavioral, look for STAR-style structure, specific examples, and self-awareness.
- Overall percentage should reflect both sections weighted ~60% technical, ~40% behavioral.`;

    const contextBlock = state.session.scoringContext
      ? `EVALUATION RUBRIC:\n${state.session.scoringContext}`
      : `JOB DESCRIPTION:\n${state.session.jd}\n\nCANDIDATE RESUME:\n${state.session.resume}`;
    const user = `${contextBlock}\n\nROLE: ${state.session.role}\nCANDIDATE: ${state.session.candidateName}\n\nINTERVIEW TRANSCRIPT:\n${transcript}\n\nScore this interview and return the JSON evaluation.`;

    const resStr = await callAI(sys, user, envNumber('GEMINI_SCORING_MAX_OUTPUT_TOKENS'), null, false);
    const results = extractJSON(resStr);
    state.session.results = results;

    // Fire-and-forget email to HR — show result on screen too
    const emailResult = await emailReportToHR(results);
    state.session.emailResult = emailResult;

    showResults();
  } catch (e) {
    alert('Failed to score interview: ' + e.message);
    showScreen('setup');
  }
}

export function abortInterview() {
  if (confirm('Cancel this interview? All progress will be lost.')) {
    if (state.session.isListening) toggleMic();
    speechSynthesis.cancel();
    showScreen('setup');
  }
}

// =============== RESULTS ===============
