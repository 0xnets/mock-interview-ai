import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [react(), tailwindcss()],
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
