const DEFAULT_ENV = {
  LOCAL_STORAGE_CONFIG_KEY: 'mockInterviewConfig',
  DEFAULT_TECH_QUESTION_COUNT: '5',
  DEFAULT_NON_TECH_QUESTION_COUNT: '5',
  DEFAULT_PASS_THRESHOLD: '90',
  SPEECH_RECOGNITION_LANG: 'en-US',
  SPEECH_RECOGNITION_CONTINUOUS: 'true',
  SPEECH_RECOGNITION_INTERIM_RESULTS: 'true',
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
  WHATSAPP_SHARE_URL: 'https://wa.me/?text=',
  EMAIL_SHARE_SCHEME: 'mailto:',
  APP_API_BASE_URL: 'http://localhost:8080'
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
