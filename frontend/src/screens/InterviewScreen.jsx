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
    if (section === 'Technical') return "Now let's move to the technical questions. ";
  }
  return '';
}

export function InterviewScreen() {
  const navigate = useNavigate();
  const location = useLocation();
  const { interview, updateInterview } = useAppState();
  const toast = useToast();

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
  const [submitEnabled, setSubmitEnabled] = useState(false);
  const [followupThinking, setFollowupThinking] = useState(false);

  const socketRef = useRef(null);
  const utteranceSeqRef = useRef(0);
  const currentOrdinalRef = useRef(null);
  const questionStartedAtRef = useRef(0);
  const totalQuestionsRef = useRef(0);
  const lastSectionRef = useRef(null);
  const currentQuestionRef = useRef('');
  const finalizedRef = useRef(false);

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
        setMicStatus("Didn't catch that — click the mic and try again.");
      } else if (err === 'not-allowed') {
        setMicStatus('Microphone permission denied. Allow it in browser settings and reload.');
      }
    },
  });

  // Mirror the original onresult behavior: a non-empty final transcript enables Submit.
  useEffect(() => {
    if (recog.finalTranscript.trim().length > 0) setSubmitEnabled(true);
  }, [recog.finalTranscript]);

  async function renderQuestionFromServer(msg) {
    const ordinal = msg.ordinal;
    currentOrdinalRef.current = ordinal;

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
    setFollowupThinking(false);
    setMicStatus('🔊 Listen to the question...');
    recog.reset();

    const preamble = computePreamble(section, ordinal, candidateNameRef.current, lastSectionRef.current);
    lastSectionRef.current = section;

    await speak(preamble + msg.text);

    questionStartedAtRef.current = Date.now();
    setMicEnabled(true);
    setMicStatus('Click the microphone when ready to answer.');
  }

  function handleStateChange(value) {
    if (value === 'thinking') {
      setMicStatus('🤔 Interviewer is preparing a follow-up…');
      setMicEnabled(false);
      setSubmitEnabled(false);
      setFollowupThinking(true);
    }
  }

  async function finalizeAndShow() {
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
    const wasListening = recog.isListening;
    recog.toggle();
    if (!recog.supported) return; // toggle() already alerted via onError
    if (wasListening) {
      setMicStatus(recog.finalTranscript.trim()
        ? 'Click Submit to continue, or 🎤 to re-record.'
        : 'No speech detected. Click 🎤 to try again.');
    } else {
      setMicStatus('Listening — click again when done answering');
    }
  }

  function handleSubmit() {
    const answer = (recog.finalTranscript + recog.interimTranscript).trim();
    if (!answer) { toast.warning('Please record an answer first.'); return; }
    recog.stop();

    const ordinal = currentOrdinalRef.current;
    if (ordinal == null) return;

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
    setMicStatus('Submitting…');
  }

  function handleAbort() {
    if (confirm('Cancel this interview? All progress will be lost.')) {
      recog.stop();
      if ('speechSynthesis' in window) speechSynthesis.cancel();
      closeSocket(socketRef.current);
      socketRef.current = null;
      navigate('/setup');
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
        <div className="flex justify-between text-sm font-semibold mb-2">
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

      <div className="bg-gray-50 rounded-xl p-6 mb-6 min-h-[140px] flex items-center">
        <div className="w-full">
          <div className="text-xs uppercase tracking-wide text-gray-500 mb-2">Interviewer asks:</div>
          <div className="text-lg leading-relaxed">{question}</div>
        </div>
      </div>

      <div className="text-center mb-6">
        <div className="text-sm text-gray-600 mb-3">{micStatus}</div>
        <button
          className={`bg-indigo-600 hover:bg-indigo-700 text-white rounded-full w-24 h-24 text-4xl shadow-lg transition-all ${recog.isListening ? 'pulse-mic' : ''}`}
          disabled={!micEnabled}
          onClick={handleMicToggle}
        >
          {recog.isListening ? '⏹️' : '🎤'}
        </button>
        {recog.isListening && (
          <div className="mt-4">
            {[0, 1, 2, 3, 4].map(i => <div key={i} className="wave-bar" />)}
          </div>
        )}
      </div>

      <div className="bg-white border border-gray-200 rounded-lg p-4 mb-4">
        <div className="text-xs uppercase tracking-wide text-gray-500 mb-2">Your live transcript:</div>
        <div className={`text-sm text-gray-700 min-h-[60px] ${transcriptItalic ? 'italic' : ''}`}>
          {transcriptText}
        </div>
      </div>

      <div className="flex justify-between gap-3">
        <button
          className="btn-secondary"
          onClick={() => { if (currentQuestionRef.current) speak(currentQuestionRef.current); }}
        >
          🔁 Repeat Question
        </button>
        <button className="btn-primary" disabled={!submitEnabled} onClick={handleSubmit}>
          Submit &amp; Next →
        </button>
      </div>
    </div>
  );
}
