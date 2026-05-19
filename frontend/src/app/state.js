import { envNumber } from './env.js';

export const state = {
  config: {
    hrEmail: '',
    techCount: envNumber('DEFAULT_TECH_QUESTION_COUNT'),
    nonTechCount: envNumber('DEFAULT_NON_TECH_QUESTION_COUNT'),
    nonTechBank: '',
    passThreshold: envNumber('DEFAULT_PASS_THRESHOLD'),
    voiceName: ''
  },
  session: {
    sessionId: '',
    shortcode: '',
    candidateName: '',
    role: '',
    isListening: false,
    finalTranscript: '',
    interimTranscript: '',
    // Phase 4 — realtime
    socket: null,
    joinNonce: '',
    wsPath: '/v1/ws/interview',
    currentOrdinal: null,
    utteranceSeq: 0,
    questionStartedAt: 0,
    totalQuestions: 0,
    lastSection: null,
    currentQuestionText: '',
    reportPdfUrl: '',
    reportPdfError: '',
    results: null
  },
  voices: [],
  recognition: null
};
