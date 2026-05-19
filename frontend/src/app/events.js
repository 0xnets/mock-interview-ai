import { saveConfig } from './config.js';
import { switchTab } from '../ui/tabs.js';
import { showScreen } from '../ui/screens.js';
import { loadPdfIntoTextarea } from '../services/pdf-parser.service.js';
import { testVoice, speakCurrentQuestion } from '../voice/speech-synthesis.js';
import { toggleMic } from '../voice/speech-recognition.js';
import { generateCandidateLink, copyLink } from '../candidate/candidate-link.js';
import { shareViaEmail, shareViaWhatsApp, copyShareMessage } from '../candidate/share-message.js';
import { startCandidateInterview } from '../interview/candidate-mode.js';
import { startInterview, submitAnswer, abortInterview } from '../interview/interview-flow.js';
import { state } from './state.js';
import { attemptLogin, attemptLogout } from '../auth/auth-flow.js';
import { submitAcceptInvite, openAcceptInviteScreen } from '../auth/accept-invite.js';
import { openAdminInvites, submitCreateInvite, copyInviteLink } from '../admin/admin-invites.js';
import { openTranscriptForCurrentSession } from '../transcript/transcript-viewer.js';

function on(id, eventName, handler) {
  const el = document.getElementById(id);
  if (el) el.addEventListener(eventName, handler);
}

export function attachEventHandlers() {
  on('settingsBtn', 'click', () => showScreen('setup'));
  document.querySelectorAll('.tab[data-tab]').forEach(tab => {
    tab.addEventListener('click', () => switchTab(tab.dataset.tab));
  });

  document.querySelectorAll('input[type="file"][data-pdf-textarea][data-pdf-status]').forEach(input => {
    input.addEventListener('change', event => {
      loadPdfIntoTextarea(event.currentTarget, input.dataset.pdfTextarea, input.dataset.pdfStatus);
    });
  });

  on('startBtn', 'click', startInterview);
  on('generateLinkBtn', 'click', generateCandidateLink);
  on('copyLinkBtn', 'click', copyLink);
  on('shareEmailBtn', 'click', shareViaEmail);
  on('shareWhatsAppBtn', 'click', shareViaWhatsApp);
  on('copyShareMessageBtn', 'click', copyShareMessage);
  on('testVoiceBtn', 'click', testVoice);
  on('saveConfigBtn', 'click', saveConfig);
  on('welcomeStartBtn', 'click', startCandidateInterview);
  on('abortInterviewBtn', 'click', abortInterview);
  on('micBtn', 'click', toggleMic);
  on('replayBtn', 'click', speakCurrentQuestion);
  on('submitBtn', 'click', submitAnswer);

  // Auth
  on('loginBtn', 'click', attemptLogin);
  on('logoutBtn', 'click', attemptLogout);
  on('acceptInviteBtn', 'click', submitAcceptInvite);
  on('toggleAcceptInviteBtn', 'click', openAcceptInviteScreen);
  on('backToLoginBtn', 'click', () => showScreen('login'));

  // Admin invites
  on('adminInvitesBtn', 'click', openAdminInvites);
  on('createInviteBtn', 'click', submitCreateInvite);
  on('copyInviteLinkBtn', 'click', copyInviteLink);
  on('backFromAdminInviteBtn', 'click', () => showScreen('setup'));

  // Transcript viewer
  on('viewTranscriptBtn', 'click', openTranscriptForCurrentSession);
  on('backFromTranscriptBtn', 'click', () => showScreen('results'));

  document.addEventListener('click', event => {
    const target = event.target;
    if (!(target instanceof Element)) return;

    if (target.id === 'downloadPdfBtn') {
      const url = state.session.reportPdfUrl;
      if (url) {
        window.open(url, '_blank', 'noopener');
      } else {
        alert('PDF is still being generated. Please try again in a few seconds.');
      }
    } else if (target.id === 'newInterviewBtn') {
      showScreen('setup');
      switchTab('start');
    }
  });
}
