import { defineConfig } from 'vite';

export default defineConfig({
  envPrefix: [
    'APP_',
    'LOCAL_',
    'DEFAULT_',
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
