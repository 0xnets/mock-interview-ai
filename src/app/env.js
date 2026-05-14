const DEFAULT_ENV = {
  DEPLOYABLE_DOWNLOAD_FILENAME: 'mock-interview-deployed.html',
  EMBEDDED_KEY_PLACEHOLDER: '__REPLACE_ME_API_KEY__',
  LOCAL_STORAGE_CONFIG_KEY: 'mockInterviewConfig',
  GEMINI_MODEL: 'gemini-2.5-flash',
  GEMINI_GENERATE_CONTENT_BASE_URL: 'https://generativelanguage.googleapis.com/v1beta/models',
  GEMINI_GENERATE_CONTENT_METHOD: 'generateContent',
  GEMINI_DEFAULT_MAX_OUTPUT_TOKENS: '4000',
  GEMINI_SCORING_MAX_OUTPUT_TOKENS: '8000',
  GEMINI_RUBRIC_MAX_OUTPUT_TOKENS: '800',
  GEMINI_TEMPERATURE: '0.7',
  GEMINI_RESPONSE_MIME_TYPE: 'application/json',
  GEMINI_THINKING_BUDGET: '0',
  DEFAULT_TECH_QUESTION_COUNT: '5',
  DEFAULT_NON_TECH_QUESTION_COUNT: '5',
  DEFAULT_PASS_THRESHOLD: '90',
  CANDIDATE_SESSION_HASH_KEY: 'session',
  CLEANURI_SHORTEN_URL: 'https://cleanuri.com/api/v1/shorten',
  IS_GD_SHORTEN_URL: 'https://is.gd/create.php?format=simple&url=',
  DA_GD_SHORTEN_URL: 'https://da.gd/s?url=',
  URL_SHORTENER_TIMEOUT_MS: '6000',
  REPORT_EMAIL_FUNCTION_URL: '/.netlify/functions/send-report-email',
  NETLIFY_FORM_POST_URL: '/',
  NETLIFY_FORM_NAME: 'interview-result',
  NETLIFY_HONEYPOT_FIELD: 'bot-field',
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
  PDF_REPORT_FILENAME_PREFIX: 'Interview_Report_',
  RESULT_PASS_COLOR: '#10b981',
  RESULT_WARNING_COLOR: '#f59e0b',
  RESULT_FAIL_COLOR: '#ef4444',
  RESULT_RING_RADIUS: '70',
  RESULT_WARNING_SCORE_THRESHOLD: '70',
  WHATSAPP_SHARE_URL: 'https://wa.me/?text=',
  EMAIL_SHARE_SCHEME: 'mailto:'
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
