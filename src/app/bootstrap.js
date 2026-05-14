import { loadConfig, applyEmbeddedKey, checkConfig, updateDeployKeyStatus } from './config.js';
import { loadVoices } from '../voice/voice-selection.js';
import { setupRecognition } from '../voice/speech-recognition.js';
import { readCandidateSessionFromURL } from '../candidate/session-codec.js';
import { enterCandidateMode } from '../interview/candidate-mode.js';

export function bootstrap() {
  loadConfig();
  loadVoices();
  if ('speechSynthesis' in window) {
    speechSynthesis.onvoiceschanged = loadVoices;
  }
  setupRecognition();
  // Use embedded key if it was baked in
  applyEmbeddedKey();
  checkConfig();
  updateDeployKeyStatus();
  // Detect candidate mode from URL hash
  const candidateSession = readCandidateSessionFromURL();
  if (candidateSession) {
    enterCandidateMode(candidateSession);
  }
}
