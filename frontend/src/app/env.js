function defaultApiBaseUrl() {
  if (typeof window === 'undefined' || !window.location?.hostname) {
    return 'http://localhost:8080';
  }
  if (window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1') {
    return 'http://localhost:8080';
  }
  const protocol = window.location.protocol === 'https:' ? 'https:' : 'http:';
  return `${protocol}//${window.location.host}`;
}

const DEFAULT_ENV = {
  DEFAULT_TECH_QUESTION_COUNT: '5',
  DEFAULT_PASS_THRESHOLD: '90',
  MAX_CANDIDATE_NAME_CHARS: '120',
  MAX_ROLE_TITLE_CHARS: '160',
  MIN_JD_WORDS: '20',
  MIN_RESUME_WORDS: '30',
  MIN_INVITE_PASSWORD_CHARS: '12',
  DEFAULT_NON_TECH_SECTION: 'Role Fit & Work Preferences',
  MAX_BANK_SECTIONS: '20',
  MAX_BANK_QUESTIONS: '400',
  MAX_BANK_SECTION_CHARS: '80',
  MAX_BANK_QUESTION_CHARS: '500',
  SESSION_PRIMING_POLL_INTERVAL_MS: '1500',
  SESSION_PRIMING_TIMEOUT_MS: '90000',
  COPY_FEEDBACK_RESET_MS: '2000',
  CONFIG_SAVE_STATUS_MS: '2500',
  CONFIG_ADD_STATUS_MS: '2500',
  ACCEPT_INVITE_REDIRECT_DELAY_MS: '1200',
  ANSWER_TIME_LIMIT_MS: '180000',
  STT_SAMPLE_RATE: '16000',
  STT_CHUNK_DURATION_MS: '100',
  STT_WORKLET_URL: '/pcm-processor.js',
  SPEECH_SYNTHESIS_FALLBACK_LANG: 'en-US',
  SPEECH_SYNTHESIS_RATE: '0.95',
  SPEECH_SYNTHESIS_PITCH: '1.0',
  VOICE_LOAD_MAX_ATTEMPTS: '30',
  VOICE_LOAD_RETRY_DELAY_MS: '100',
  PDF_ACCEPTED_MIME_TYPE: 'application/pdf',
  PDF_MIN_EXTRACTED_TEXT_LENGTH: '20',
  RESULT_PASS_COLOR: '#10b981',
  RESULT_WARNING_COLOR: '#f59e0b',
  RESULT_FAIL_COLOR: '#ef4444',
  RESULT_RING_RADIUS: '70',
  RESULT_WARNING_SCORE_THRESHOLD: '70',
  APP_API_BASE_URL: defaultApiBaseUrl()
};

export const ENV = {
  ...DEFAULT_ENV,
  ...((typeof import.meta !== 'undefined' && import.meta.env) ? import.meta.env : {})
};

export function envNumber(key) {
  return Number(ENV[key]);
}

export function envBoolean(key) {
  return String(ENV[key]).toLowerCase() === 'true';
}
