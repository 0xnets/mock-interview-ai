import { state } from './state.js';
import { ENV, envNumber } from './env.js';

export function loadConfig() {
  const saved = localStorage.getItem(ENV.LOCAL_STORAGE_CONFIG_KEY);
  if (saved) {
    state.config = { ...state.config, ...JSON.parse(saved) };
  }
  const hrEmailEl = document.getElementById('hrEmail');
  if (hrEmailEl) hrEmailEl.value = state.config.hrEmail || '';
  const techCountEl = document.getElementById('techCount');
  if (techCountEl) techCountEl.value = state.config.techCount;
  const nonTechCountEl = document.getElementById('nonTechCount');
  if (nonTechCountEl) nonTechCountEl.value = state.config.nonTechCount;
  const nonTechBankEl = document.getElementById('nonTechBank');
  if (nonTechBankEl) nonTechBankEl.value = state.config.nonTechBank || '';
  const passThresholdEl = document.getElementById('passThreshold');
  if (passThresholdEl) passThresholdEl.value = state.config.passThreshold;
  const techCountDisp = document.getElementById('techCountDisplay');
  if (techCountDisp) techCountDisp.textContent = state.config.techCount;
  const nonTechCountDisp = document.getElementById('nonTechCountDisplay');
  if (nonTechCountDisp) nonTechCountDisp.textContent = state.config.nonTechCount;
}

export function saveConfig() {
  const hrEmail = document.getElementById('hrEmail').value.trim();
  if (!hrEmail) {
    alert('Please enter the report recipient email in HR Configuration.');
    return;
  }
  if (!document.getElementById('hrEmail').checkValidity()) {
    alert('Please enter a valid report recipient email.');
    return;
  }

  state.config = {
    hrEmail,
    techCount: parseInt(document.getElementById('techCount').value) || envNumber('DEFAULT_TECH_QUESTION_COUNT'),
    nonTechCount: parseInt(document.getElementById('nonTechCount').value) || envNumber('DEFAULT_NON_TECH_QUESTION_COUNT'),
    nonTechBank: document.getElementById('nonTechBank').value.trim(),
    passThreshold: parseInt(document.getElementById('passThreshold').value) || envNumber('DEFAULT_PASS_THRESHOLD'),
    voiceName: document.getElementById('voiceSelect').value
  };
  localStorage.setItem(ENV.LOCAL_STORAGE_CONFIG_KEY, JSON.stringify(state.config));
  const status = document.getElementById('saveStatus');
  status.classList.remove('hidden');
  setTimeout(() => status.classList.add('hidden'), 2500);
  document.getElementById('techCountDisplay').textContent = state.config.techCount;
  document.getElementById('nonTechCountDisplay').textContent = state.config.nonTechCount;
  checkConfig();
}

export function checkConfig() {
  const warn = document.getElementById('configWarning');
  if (!warn) return;
  const ok = state.config.hrEmail && state.config.nonTechBank;
  warn.classList.toggle('hidden', ok);
}
