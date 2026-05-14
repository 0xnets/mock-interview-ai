import { state } from './state.js';
import { ENV, envNumber } from './env.js';

export function applyEmbeddedKey() {
  // If the file was deployed with a baked-in key, use it (overrides localStorage)
  if (window.EMBEDDED_KEY && window.EMBEDDED_KEY !== ENV.EMBEDDED_KEY_PLACEHOLDER && window.EMBEDDED_KEY.length > 10) {
    state.config.apiKey = window.EMBEDDED_KEY;
  }
}

export function updateDeployKeyStatus() {
  const el = document.getElementById('deployKeyStatus');
  if (!el) return;
  const localKey = (JSON.parse(localStorage.getItem(ENV.LOCAL_STORAGE_CONFIG_KEY) || '{}').apiKey) || '';
  const embedded = window.EMBEDDED_KEY && window.EMBEDDED_KEY !== ENV.EMBEDDED_KEY_PLACEHOLDER;
  if (embedded) {
    el.innerHTML = '<span class="text-green-700">✓ This file already has a key embedded. Re-download only if you want to update the key.</span>';
  } else if (localKey) {
    el.innerHTML = '<span class="text-green-700">✓ Key saved in your browser. Ready to bake into the deploy file.</span>';
  } else {
    el.innerHTML = '<span class="text-red-700">✗ No key configured yet. Go to HR Configuration first.</span>';
  }
}

export function loadConfig() {
  const saved = localStorage.getItem(ENV.LOCAL_STORAGE_CONFIG_KEY);
  if (saved) {
    state.config = { ...state.config, ...JSON.parse(saved) };
  }
  // Embedded config (from deployed file) overrides localStorage for non-key fields
  if (window.EMBEDDED_CONFIG) {
    state.config = { ...state.config, ...window.EMBEDDED_CONFIG };
  }
  // Populate UI fields (only if they exist — they may be hidden in candidate mode)
  const apiKeyEl = document.getElementById('apiKey');
  if (apiKeyEl) apiKeyEl.value = state.config.apiKey || '';
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
  state.config = {
    apiKey: document.getElementById('apiKey').value.trim(),
    hrEmail: document.getElementById('hrEmail').value.trim(),
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
  updateDeployKeyStatus();
}

export function checkConfig() {
  const warn = document.getElementById('configWarning');
  const ok = state.config.apiKey && state.config.nonTechBank;
  warn.classList.toggle('hidden', ok);
}
