import { defineConfig } from 'vite';

export default defineConfig({
  envPrefix: [
    'APP_',
    'DEPLOYABLE_',
    'EMBEDDED_',
    'LOCAL_',
    'GEMINI_',
    'DEFAULT_',
    'MIN_',
    'MAX_',
    'CANDIDATE_',
    'CLEANURI_',
    'IS_GD_',
    'DA_GD_',
    'URL_',
    'NETLIFY_',
    'SPEECH_',
    'VOICE_',
    'PDF_',
    'RESULT_',
    'WHATSAPP_',
    'EMAIL_'
  ],
  build: {
    rollupOptions: {
      input: {
        index: 'index.html'
      }
    },
    modulePreload: false
  }
});
