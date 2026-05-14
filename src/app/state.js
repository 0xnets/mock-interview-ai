import { envNumber } from './env.js';

export const state = {
  config: {
    apiKey: '',
    hrEmail: '',
    techCount: envNumber('DEFAULT_TECH_QUESTION_COUNT'),
    nonTechCount: envNumber('DEFAULT_NON_TECH_QUESTION_COUNT'),
    nonTechBank: '',
    passThreshold: envNumber('DEFAULT_PASS_THRESHOLD'),
    voiceName: ''
  },
  session: {
    candidateName: '',
    role: '',
    jd: '',
    resume: '',
    techQuestions: [],
    nonTechQuestions: [],
    allQuestions: [], // {question, section, answer}
    currentIdx: 0,
    isListening: false,
    finalTranscript: '',
    interimTranscript: '',
    results: null
  },
  voices: [],
  recognition: null
};
