import { saveConfig } from './config.js';
import { switchTab } from '../ui/tabs.js';
import { showScreen } from '../ui/screens.js';
import { loadPdfIntoTextarea } from '../services/pdf-parser.service.js';
import { testVoice, speakCurrentQuestion } from '../voice/speech-synthesis.js';
import { toggleMic } from '../voice/speech-recognition.js';
import { generateCandidateLink, copyLink } from '../candidate/candidate-link.js';
import { shareViaEmail, shareViaWhatsApp, copyShareMessage } from '../candidate/share-message.js';
import { startCandidateInterview } from '../interview/candidate-mode.js';
import { downloadDeployableHTML } from '../services/deploy-html.service.js';
import { startInterview, submitAnswer, abortInterview } from '../interview/interview-flow.js';
import { downloadPDF } from '../reports/pdf-report.js';

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
  on('downloadDeployableBtn', 'click', downloadDeployableHTML);
  on('testVoiceBtn', 'click', testVoice);
  on('saveConfigBtn', 'click', saveConfig);
  on('welcomeStartBtn', 'click', startCandidateInterview);
  on('abortInterviewBtn', 'click', abortInterview);
  on('micBtn', 'click', toggleMic);
  on('replayBtn', 'click', speakCurrentQuestion);
  on('submitBtn', 'click', submitAnswer);

  document.addEventListener('click', event => {
    const target = event.target;
    if (!(target instanceof Element)) return;

    if (target.id === 'downloadPdfBtn') {
      downloadPDF();
    } else if (target.id === 'newInterviewBtn') {
      showScreen('setup');
      switchTab('start');
    }
  });
}
