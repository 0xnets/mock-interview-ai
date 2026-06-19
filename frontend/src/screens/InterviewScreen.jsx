import { useEffect, useRef, useState } from 'react';
import { useNavigate, useLocation, Navigate } from 'react-router-dom';
import { useAppState } from '../providers/AppStateProvider.jsx';
import { useToast } from '../providers/ToastProvider.jsx';
import { isAuthenticated } from '../app/auth-store.js';
import { openInterviewSocket, sendMsg, closeSocket } from '../realtime/ws-client.js';
import { speak } from '../voice/speech-synthesis.js';
import { useSpeechRecognition } from '../hooks/useSpeechRecognition.js';
import { finalizeInterview } from '../api/client.js';
import { awaitReportReady } from '../reports/report-waiting.js';
import { LoadingScreen } from './LoadingScreen.jsx';

function computePreamble(section, ordinal, candidateName, lastSection) {
  if (ordinal === 1 || section === 'Intro') {
    return `Hello ${candidateName}. Welcome to your mock interview. To get started, `;
  }
  if (section === 'Follow-up') {
    return 'One follow-up on that. ';
  }
  if (lastSection && lastSection !== section) {
    if (section === 'Behavioral') return "Great. Now let's move to the behavioral questions. ";
  }
  return '';
}

function formatRemainingTime(totalSeconds) {
  const safeSeconds = Math.max(0, totalSeconds);
  const minutes = Math.floor(safeSeconds / 60);
  const seconds = safeSeconds % 60;
  return `${minutes}:${String(seconds).padStart(2, '0')}`;
}

export function InterviewScreen() {
  const navigate = useNavigate();
  const location = useLocation();
  const { interview, config, updateInterview } = useAppState();
  const toast = useToast();
  const answerTimeLimitMs = Math.max(
    1000,
    Number(interview.answerTimeLimitMs || config.answerTimeLimitMs) || 180000,
  );

  // Mount snapshots of interview context — set once before this screen mounts.
  const candidateNameRef = useRef(interview.candidateName);
  const sessionIdRef = useRef(interview.sessionId);
  const shortcodeRef = useRef(interview.shortcode);
  const searchRef = useRef(location.search);

  const [phase, setPhase] = useState('interview');
  const [sectionLabel, setSectionLabel] = useState('Section: —');
  const [progressText, setProgressText] = useState('Connecting…');
  const [progressPct, setProgressPct] = useState(0);
  const [question, setQuestion] = useState('Connecting to interviewer…');
  const [micStatus, setMicStatus] = useState('');
  const [micEnabled, setMicEnabled] = useState(false);
  const [micStartedForQuestion, setMicStartedForQuestion] = useState(false);
  const [submitEnabled, setSubmitEnabled] = useState(false);
  const [followupThinking, setFollowupThinking] = useState(false);
  const [questionDictating, setQuestionDictating] = useState(false);
  const [answerTimeRemainingSeconds, setAnswerTimeRemainingSeconds] = useState(null);

  const socketRef = useRef(null);
  const utteranceSeqRef = useRef(0);
  const currentOrdinalRef = useRef(null);
  const questionStartedAtRef = useRef(0);
  const totalQuestionsRef = useRef(0);
  const lastSectionRef = useRef(null);
  const currentQuestionRef = useRef('');
  const finalizedRef = useRef(false);
  const answerTimerDeadlineRef = useRef(0);
  const answerTimeoutRef = useRef(null);
  const answerIntervalRef = useRef(null);
  const submittedOrdinalRef = useRef(null);
  const latestFinalTranscriptRef = useRef('');
  const latestInterimTranscriptRef = useRef('');

  const recog = useSpeechRecognition({
    onFinalChunk: (text) => {
      if (socketRef.current && currentOrdinalRef.current != null) {
        sendMsg(socketRef.current, {
          t: 'utterance',
          seq: ++utteranceSeqRef.current,
          ordinal: currentOrdinalRef.current,
          text,
          is_final: true,
        });
      }
    },
    onError: (err) => {
      if (err === 'unsupported') {
        toast.error("Your browser doesn't support speech recognition. Please use Chrome or Edge.");
      } else if (err === 'no-speech') {
        setMicStatus('No speech detected yet. Keep answering; Submit will unlock once speech is captured.');
      } else if (err === 'not-allowed') {
        setMicStatus('Microphone permission denied. Allow it in browser settings and reload.');
      }
    },
  });

  // Enable Submit as soon as speech appears, including interim text.
  useEffect(() => {
    if ((recog.finalTranscript + recog.interimTranscript).trim().length > 0) setSubmitEnabled(true);
  }, [recog.finalTranscript, recog.interimTranscript]);

  useEffect(() => {
    latestFinalTranscriptRef.current = recog.finalTranscript;
    latestInterimTranscriptRef.current = recog.interimTranscript;
  }, [recog.finalTranscript, recog.interimTranscript]);

  function clearAnswerTimer() {
    if (answerTimeoutRef.current) {
      window.clearTimeout(answerTimeoutRef.current);
      answerTimeoutRef.current = null;
    }
    if (answerIntervalRef.current) {
      window.clearInterval(answerIntervalRef.current);
      answerIntervalRef.current = null;
    }
    answerTimerDeadlineRef.current = 0;
    setAnswerTimeRemainingSeconds(null);
  }

  function updateAnswerTimeRemaining() {
    if (!answerTimerDeadlineRef.current) return;
    const remainingMs = answerTimerDeadlineRef.current - Date.now();
    if (remainingMs <= 0) {
      setAnswerTimeRemainingSeconds(0);
      submitAnswer({ allowEmpty: true, timeout: true });
      return;
    }
    setAnswerTimeRemainingSeconds(Math.ceil(remainingMs / 1000));
  }

  function startAnswerTimer() {
    clearAnswerTimer();
    answerTimerDeadlineRef.current = Date.now() + answerTimeLimitMs;
    setAnswerTimeRemainingSeconds(Math.ceil(answerTimeLimitMs / 1000));
    answerIntervalRef.current = window.setInterval(updateAnswerTimeRemaining, 250);
    answerTimeoutRef.current = window.setTimeout(() => {
      setAnswerTimeRemainingSeconds(0);
      submitAnswer({ allowEmpty: true, timeout: true });
    }, answerTimeLimitMs);
  }

  useEffect(() => () => clearAnswerTimer(), []);

  async function renderQuestionFromServer(msg) {
    clearAnswerTimer();
    const ordinal = msg.ordinal;
    currentOrdinalRef.current = ordinal;
    submittedOrdinalRef.current = null;

    const section = msg.t === 'followup'
      ? 'Follow-up'
      : (msg.kind === 'intro' ? 'Intro' : msg.kind === 'technical' ? 'Technical' : 'Behavioral');

    const questionNumber = msg.question_number || ordinal;
    const total = Math.max(msg.total_questions || 0, totalQuestionsRef.current, questionNumber);
    totalQuestionsRef.current = total;
    setSectionLabel('Section: ' + section);
    setProgressText(`Question ${questionNumber} of ${total}`);
    setProgressPct(((questionNumber - 1) / total) * 100);
    setQuestion(msg.text);
    currentQuestionRef.current = msg.text;
    setSubmitEnabled(false);
    setMicEnabled(false);
    setMicStartedForQuestion(false);
    setFollowupThinking(false);
    setQuestionDictating(true);
    setMicStatus('Listen to the question...');
    recog.reset();

    const preamble = computePreamble(section, ordinal, candidateNameRef.current, lastSectionRef.current);
    lastSectionRef.current = section;

    try {
      await speak(preamble + msg.text);
    } finally {
      setQuestionDictating(false);
    }

    questionStartedAtRef.current = Date.now();
    startAnswerTimer();
    setMicEnabled(true);
    setMicStatus('Click the microphone to start answering.');
  }

  function handleStateChange(value) {
    if (value === 'thinking') {
      clearAnswerTimer();
      setMicStatus('Interviewer is preparing a follow-up...');
      setMicEnabled(false);
      setMicStartedForQuestion(false);
      setSubmitEnabled(false);
      setFollowupThinking(true);
      setQuestionDictating(false);
    }
  }

  async function finalizeAndShow() {
    clearAnswerTimer();
    recog.stop();
    finalizedRef.current = true;
    setPhase('finalizing');
    try {
      const sessionId = sessionIdRef.current;
      if (!sessionId) throw new Error('Session id is missing — cannot finalize.');
      const { report } = await finalizeInterview(sessionId);
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

      let reportPdfUrl = '';
      let reportPdfError = '';
      try {
        reportPdfUrl = await awaitReportReady(sessionId);
      } catch (e) {
        reportPdfError = e?.message || 'PDF report generation failed.';
      }

      closeSocket(socketRef.current);
      socketRef.current = null;
      updateInterview({ results, reportPdfUrl, reportPdfError });
      navigate(`/results${searchRef.current}`);
    } catch (e) {
      toast.error('Failed to score interview: ' + e.message);
      navigate('/setup');
    }
  }

  async function handleServerMsg(msg) {
    switch (msg.t) {
      case 'hello':
        totalQuestionsRef.current = msg.total_questions || 0;
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
        toast.error(`Backend error: ${msg.message || msg.code || 'unknown'}`);
        if (msg.terminal) {
          closeSocket(socketRef.current);
          socketRef.current = null;
          if (!isAuthenticated() && shortcodeRef.current) {
            navigate(`/welcome${searchRef.current}`);
          } else if (!isAuthenticated()) {
            navigate('/login');
          } else {
            navigate('/setup');
          }
        }
        break;
      default:
        break;
    }
  }

  function handleSocketClose() {
    if (finalizedRef.current) return; // expected close after results
    console.warn('Interview WS closed before completion');
  }

  // Open the interview WebSocket once. The single-use join nonce means this must
  // not run twice — the app intentionally does not use React StrictMode.
  useEffect(() => {
    const joinNonce = interview.joinNonce;
    if (!joinNonce) return undefined;
    const socket = openInterviewSocket({
      joinNonce,
      wsPath: interview.wsPath,
      onOpen: (sock) => { sendMsg(sock, { t: 'hello', client_version: 'web/phase4' }); },
      onMessage: handleServerMsg,
      onClose: handleSocketClose,
      onError: (e) => console.error('WS error', e),
    });
    socketRef.current = socket;
    return () => {
      closeSocket(socketRef.current);
      socketRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function handleMicToggle() {
    if (!micEnabled || micStartedForQuestion || recog.isListening) return;
    recog.toggle();
    if (!recog.supported) return; // toggle() already alerted via onError
    setMicStartedForQuestion(true);
    setMicStatus('Recording your answer. Submit when you are done.');
  }

  function submitAnswer({ allowEmpty = false, timeout = false } = {}) {
    const answer = (latestFinalTranscriptRef.current + latestInterimTranscriptRef.current).trim();
    if (!allowEmpty && !answer) { toast.warning('Please record an answer first.'); return; }
    recog.stop();

    const ordinal = currentOrdinalRef.current;
    if (ordinal == null) return;
    if (submittedOrdinalRef.current === ordinal) return;
    submittedOrdinalRef.current = ordinal;
    clearAnswerTimer();

    if (answer && socketRef.current) {
      sendMsg(socketRef.current, {
        t: 'utterance',
        seq: ++utteranceSeqRef.current,
        ordinal,
        text: answer,
        is_final: true,
      });
    }
    const duration = Math.max(0, Date.now() - questionStartedAtRef.current);
    sendMsg(socketRef.current, { t: 'submit', ordinal, duration_ms: duration });

    setSubmitEnabled(false);
    setMicEnabled(false);
    setMicStatus(timeout ? 'Time is up. Submitting…' : 'Submitting…');
  }

  function handleSubmit() {
    submitAnswer();
  }

  function handleAbort() {
    if (confirm('Cancel this interview? All progress will be lost.')) {
      clearAnswerTimer();
      recog.stop();
      if ('speechSynthesis' in window) speechSynthesis.cancel();
      closeSocket(socketRef.current);
      socketRef.current = null;
      navigate(`/interview-cancelled${searchRef.current}`, { replace: true });
    }
  }

  if (!interview.joinNonce) {
    return <Navigate to={`/${location.search}`} replace />;
  }

  if (phase === 'finalizing') {
    return (
      <LoadingScreen
        title="Scoring your interview..."
        subtitle="Rolling up your per-question grades into a final report"
      />
    );
  }

  const live = (recog.finalTranscript + recog.interimTranscript).trim();
  const timerValue = answerTimeRemainingSeconds == null
    ? '—'
    : formatRemainingTime(answerTimeRemainingSeconds);
  const answerLimitSeconds = Math.max(1, Math.ceil(answerTimeLimitMs / 1000));
  const timerPercent = answerTimeRemainingSeconds == null
    ? 0
    : Math.min(100, Math.max(0, (answerTimeRemainingSeconds / answerLimitSeconds) * 100));
  const readyToAnswer = !questionDictating && micEnabled && !micStartedForQuestion && !recog.isListening;
  const answerSubmitted = currentOrdinalRef.current != null
    && submittedOrdinalRef.current === currentOrdinalRef.current;
  const recording = micStartedForQuestion && !answerSubmitted && !followupThinking;
  const endingSoon = answerTimeRemainingSeconds != null && answerTimeRemainingSeconds <= 10;
  const timerPanelClass = [
    'rounded-lg border p-4 shadow-sm transition-colors',
    endingSoon
      ? 'border-red-300 bg-red-50 text-red-800'
      : 'border-indigo-100 bg-indigo-50 text-indigo-900',
  ].join(' ');
  const timerBarClass = [
    'h-2 rounded-full transition-all duration-300',
    endingSoon ? 'bg-red-600' : 'bg-indigo-600',
  ].join(' ');
  const submitButtonClass = [
    'btn-primary',
    endingSoon && submitEnabled ? 'submit-urgent' : '',
  ].filter(Boolean).join(' ');
  const submitLabel = endingSoon ? 'Submit now' : 'Submit & Next';
  const showRepeatQuestion = !micStartedForQuestion && !answerSubmitted && !followupThinking;
  const actionRowClass = [
    'flex gap-3',
    showRepeatQuestion ? 'justify-between' : 'justify-end',
  ].join(' ');
  let transcriptText;
  let transcriptItalic;
  if (live) {
    transcriptText = live;
    transcriptItalic = false;
  } else if (followupThinking) {
    transcriptText = 'Generating follow-up…';
    transcriptItalic = true;
  } else if (recog.isListening) {
    transcriptText = 'Listening...';
    transcriptItalic = true;
  } else {
    transcriptText = '—';
    transcriptItalic = true;
  }

  return (
    <div className="card p-8">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-2xl font-bold">Interview in Progress</h2>
          <p className="text-gray-600 text-sm">
            Candidate: {interview.candidateName} — Role: {interview.role}
          </p>
        </div>
        <button className="btn-danger text-sm" onClick={handleAbort}>✕ Cancel</button>
      </div>

      <div className="mb-6">
        <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between text-sm font-semibold mb-2">
          <span>{sectionLabel}</span>
          <span>{progressText}</span>
        </div>
        <div className="w-full bg-gray-200 rounded-full h-2">
          <div
            className="bg-indigo-600 h-2 rounded-full transition-all duration-300"
            style={{ width: `${progressPct}%` }}
          />
        </div>
      </div>

      <div className={timerPanelClass} aria-live="polite">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <div className="text-xs uppercase font-bold text-current opacity-75">Time left</div>
            <div className="text-3xl font-bold tabular-nums leading-none mt-1">{timerValue}</div>
          </div>
          <div className="text-sm font-medium sm:text-right">
            {endingSoon
              ? 'Submit now, or your answer will auto-save.'
              : readyToAnswer
                ? 'Click the microphone, then answer clearly.'
                : recording
                  ? 'Recording. Submit when you finish.'
                  : answerSubmitted
                    ? 'Answer submitted. Waiting for the next question.'
                    : 'The timer starts after the question is read.'}
          </div>
        </div>
        <div className="mt-4 h-2 w-full rounded-full bg-white/80">
          <div
            className={timerBarClass}
            style={{ width: `${timerPercent}%` }}
          />
        </div>
      </div>

      <div className="bg-gray-50 rounded-xl p-6 my-6 min-h-[140px] flex items-center">
        <div className="w-full">
          <div className="text-xs uppercase tracking-wide text-gray-500 mb-2">Interviewer asks:</div>
          <div className="text-lg leading-relaxed">{question}</div>
        </div>
      </div>

      <div className="text-center mb-6">
        <div className="text-sm text-gray-600 mb-3">{micStatus}</div>
        {readyToAnswer && (
          <button
            className="ready-mic bg-indigo-600 hover:bg-indigo-700 text-white rounded-full w-24 h-24 text-4xl shadow-lg transition-all"
            onClick={handleMicToggle}
            aria-label="Start recording answer"
          >
            🎤
          </button>
        )}
        {recording && (
          <div className="inline-flex flex-col items-center rounded-xl border border-indigo-100 bg-indigo-50 px-6 py-4 text-indigo-900">
            <div className="text-sm font-bold uppercase">Recording</div>
            <div className="mt-3 h-10">
              {[0, 1, 2, 3, 4].map(i => <div key={i} className="wave-bar" />)}
            </div>
          </div>
        )}
        {!readyToAnswer && !recording && (
          <div className="inline-flex min-h-24 items-center rounded-xl border border-gray-200 bg-gray-50 px-6 py-4 text-sm font-medium text-gray-500">
            {answerSubmitted
              ? 'Answer submitted'
              : questionDictating
                ? 'Listening mode'
                : 'Microphone will unlock after the question.'}
          </div>
        )}
      </div>

      <div className="bg-white border border-gray-200 rounded-lg p-4 mb-4">
        <div className="text-xs uppercase tracking-wide text-gray-500 mb-2">Your live transcript:</div>
        <div className={`text-sm text-gray-700 min-h-[60px] ${transcriptItalic ? 'italic' : ''}`}>
          {transcriptText}
        </div>
      </div>

      <div className={actionRowClass}>
        {showRepeatQuestion && (
          <button
            className="btn-secondary"
            onClick={() => { if (currentQuestionRef.current) speak(currentQuestionRef.current); }}
          >
            🔁 Repeat Question
          </button>
        )}
        <button className={submitButtonClass} disabled={!submitEnabled} onClick={handleSubmit}>
          {submitLabel}
        </button>
      </div>
    </div>
  );
}
