import { loadConfig, checkConfig } from './config.js';
import { loadVoices } from '../voice/voice-selection.js';
import { setupRecognition } from '../voice/speech-recognition.js';
import { readCandidateSessionFromURL } from '../candidate/session-codec.js';
import { initializeLinkBaseUrlField } from '../candidate/candidate-link.js';
import { enterCandidateMode } from '../interview/candidate-mode.js';
import { showScreen } from '../ui/screens.js';
import { isAuthenticated } from './auth-store.js';
import { tryResumeSession, initAuthChrome } from '../auth/auth-flow.js';
import { readInviteTokenFromUrl, openAcceptInviteScreen } from '../auth/accept-invite.js';

export async function bootstrap() {
  loadConfig();
  loadVoices();
  if ('speechSynthesis' in window) {
    speechSynthesis.onvoiceschanged = loadVoices;
  }
  setupRecognition();
  checkConfig();
  initializeLinkBaseUrlField();
  initAuthChrome();

  const sp = new URLSearchParams(window.location.search);
  const candidateCode = sp.get('session');
  const inviteToken = readInviteTokenFromUrl();

  if (inviteToken) {
    openAcceptInviteScreen();
    return;
  }

  if (candidateCode) {
    showScreen('loading');
    const loadingTitle = document.getElementById('loadingTitle');
    const loadingSub = document.getElementById('loadingSub');
    if (loadingTitle) loadingTitle.textContent = 'Loading your interview...';
    if (loadingSub) loadingSub.textContent = 'Waiting for the backend to finish preparing personalized questions';

    const loaded = await readCandidateSessionFromURL({
      onWaiting: ({ state, attempt }) => {
        if (loadingSub) {
          loadingSub.textContent = state === 'pending'
            ? `Preparing personalized questions (attempt ${attempt})…`
            : `Status: ${state}`;
        }
      }
    });
    if (loaded) {
      await enterCandidateMode(loaded);
    } else {
      showScreen('setup');
    }
    return;
  }

  // HR flow: try a silent refresh first; on failure show login.
  const resumed = await tryResumeSession();
  if (resumed || isAuthenticated()) {
    showScreen('setup');
  } else {
    showScreen('login');
  }
}
