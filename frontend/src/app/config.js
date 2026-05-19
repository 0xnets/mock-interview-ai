import { state } from './state.js';
import { ENV, envNumber } from './env.js';
import { parseNonTechBank } from '../interview/non-tech-bank.js';

const DEFAULT_NON_TECH_SECTION = 'Role Fit & Work Preferences';
let activeNonTechSection = '';

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
  const sectionInput = document.getElementById('nonTechSectionInput');
  if (sectionInput && !sectionInput.value) sectionInput.value = DEFAULT_NON_TECH_SECTION;
  activeNonTechSection = '';
  renderNonTechSectionEditor();
  renderSavedConfigSummary();
  setConfigEditorVisible(!saved);
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
  const nonTechBank = document.getElementById('nonTechBank').value.trim();
  if (!hasBehavioralQuestions(nonTechBank)) {
    alert('Please add at least one behavioral question.');
    return;
  }

  state.config = {
    hrEmail,
    techCount: parseInt(document.getElementById('techCount').value) || envNumber('DEFAULT_TECH_QUESTION_COUNT'),
    nonTechCount: parseInt(document.getElementById('nonTechCount').value) || envNumber('DEFAULT_NON_TECH_QUESTION_COUNT'),
    nonTechBank,
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
  renderSavedConfigSummary();
  setConfigEditorVisible(false);
}

export function checkConfig() {
  const warn = document.getElementById('configWarning');
  if (!warn) return;
  const ok = state.config.hrEmail && hasBehavioralQuestions(state.config.nonTechBank);
  warn.classList.toggle('hidden', ok);
}

export function addNonTechSection() {
  const sectionEl = document.getElementById('nonTechSectionInput');
  const bankEl = document.getElementById('nonTechBank');
  if (!sectionEl || !bankEl) return;

  const section = sectionEl.value.trim();
  if (!section) {
    alert('Please enter a section name.');
    return;
  }

  bankEl.value = appendSectionToBank(bankEl.value, section);
  activeNonTechSection = findCanonicalSectionName(bankEl.value, section);
  renderNonTechSectionEditor();

  const questionsEl = document.getElementById('nonTechQuestionsInput');
  if (questionsEl) questionsEl.focus();

  const status = document.getElementById('addNonTechSectionStatus');
  if (status) {
    status.classList.remove('hidden');
    setTimeout(() => status.classList.add('hidden'), 2500);
  }
}

export function updateActiveNonTechQuestions() {
  if (!activeNonTechSection) return;

  const bankEl = document.getElementById('nonTechBank');
  const questionsEl = document.getElementById('nonTechQuestionsInput');
  if (!bankEl || !questionsEl) return;

  const bank = parseBankForEditor(bankEl.value);
  if (!Object.hasOwn(bank, activeNonTechSection)) return;

  bank[activeNonTechSection] = questionsEl.value
    .split('\n')
    .map(question => question.trim())
    .filter(Boolean);
  bankEl.value = serializeBank(bank);
  renderNonTechSectionList(bank);
}

export function showConfigEditor() {
  setConfigEditorVisible(true);
  const emailEl = document.getElementById('hrEmail');
  if (emailEl) emailEl.focus();
}

export function cancelConfigEdit() {
  loadConfig();
  setConfigEditorVisible(false);
}

function appendSectionToBank(text, section) {
  const bank = parseBankForEditor(text);
  const existing = Object.keys(bank).find(name => name.toLowerCase() === section.toLowerCase());
  if (existing) return serializeBank(bank);

  bank[section] = [];
  return serializeBank(bank);
}

function findCanonicalSectionName(text, section) {
  const bank = parseBankForEditor(text);
  return Object.keys(bank).find(name => name.toLowerCase() === section.toLowerCase()) || section;
}

function renderNonTechSectionEditor() {
  const bankEl = document.getElementById('nonTechBank');
  const editorEl = document.getElementById('nonTechQuestionEditor');
  const questionsEl = document.getElementById('nonTechQuestionsInput');
  const labelEl = document.getElementById('nonTechQuestionsLabel');
  if (!bankEl || !editorEl || !questionsEl || !labelEl) return;

  const bank = parseBankForEditor(bankEl.value);
  renderNonTechSectionList(bank);

  if (!activeNonTechSection || !Object.hasOwn(bank, activeNonTechSection)) {
    editorEl.classList.add('hidden');
    questionsEl.value = '';
    return;
  }

  labelEl.textContent = `Questions for ${activeNonTechSection}`;
  questionsEl.value = bank[activeNonTechSection].join('\n');
  editorEl.classList.remove('hidden');
}

function renderNonTechSectionList(bank) {
  const listEl = document.getElementById('nonTechSectionList');
  if (!listEl) return;

  listEl.replaceChildren();
  const entries = Object.entries(bank);
  if (entries.length === 0) {
    const empty = document.createElement('p');
    empty.className = 'text-xs text-gray-500';
    empty.textContent = 'No sections added yet.';
    listEl.appendChild(empty);
    return;
  }

  for (const [section, questions] of entries) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = `btn-secondary text-sm ${section === activeNonTechSection ? 'is-active' : ''}`;
    button.textContent = `${section} (${questions.length})`;
    button.addEventListener('click', () => {
      activeNonTechSection = section;
      renderNonTechSectionEditor();
      const questionsEl = document.getElementById('nonTechQuestionsInput');
      if (questionsEl) questionsEl.focus();
    });
    listEl.appendChild(button);
  }
}

function parseBankForEditor(text) {
  const bank = parseNonTechBank(text);
  if (bank._default) {
    bank['Behavioral Questions'] = bank._default;
    delete bank._default;
  }
  return bank;
}

function serializeBank(bank) {
  return Object.entries(bank)
    .map(([section, questions]) => {
      const lines = [`## ${section}`, ...questions];
      return lines.join('\n');
    })
    .join('\n\n');
}

function hasBehavioralQuestions(text) {
  return Object.values(parseBankForEditor(text)).some(questions => questions.length > 0);
}

function renderSavedConfigSummary() {
  const summaryEl = document.getElementById('savedConfigSummary');
  const detailsEl = document.getElementById('savedConfigDetails');
  const bankEl = document.getElementById('savedBehavioralBank');
  if (!summaryEl || !detailsEl || !bankEl) return;

  const hasSavedConfig = Boolean(localStorage.getItem(ENV.LOCAL_STORAGE_CONFIG_KEY));
  summaryEl.classList.toggle('hidden', !hasSavedConfig);
  if (!hasSavedConfig) return;

  const bank = parseBankForSummary(state.config.nonTechBank);
  const sections = Object.keys(bank).length;
  detailsEl.textContent = [
    state.config.hrEmail || 'No email set',
    `${state.config.techCount} technical`,
    `${state.config.nonTechCount} behavioral`,
    `${sections} behavioral ${sections === 1 ? 'section' : 'sections'}`,
    `${state.config.passThreshold}% pass threshold`
  ].join(' • ');
  renderBehavioralBankSummary(bankEl, bank);
}

function setConfigEditorVisible(visible) {
  const editorEl = document.getElementById('configEditor');
  const cancelEl = document.getElementById('cancelConfigEditBtn');
  const hasSavedConfig = Boolean(localStorage.getItem(ENV.LOCAL_STORAGE_CONFIG_KEY));
  if (editorEl) editorEl.classList.toggle('hidden', !visible);
  if (cancelEl) cancelEl.classList.toggle('hidden', !hasSavedConfig);
}

function parseBankForSummary(text) {
  const bank = parseNonTechBank(text);
  return Object.fromEntries(
    Object.entries(bank)
      .filter(([, questions]) => questions.length > 0)
      .map(([section, questions]) => [
        section === '_default' ? 'Behavioral Questions' : section,
        questions
      ])
  );
}

function renderBehavioralBankSummary(container, bank) {
  container.replaceChildren();

  const entries = Object.entries(bank);
  if (entries.length === 0) {
    const empty = document.createElement('p');
    empty.className = 'text-xs text-indigo-900';
    empty.textContent = 'No behavioral questions saved yet.';
    container.appendChild(empty);
    return;
  }

  for (const [section, questions] of entries) {
    const sectionEl = document.createElement('section');
    sectionEl.className = 'bg-white border border-indigo-100 rounded-lg p-3';

    const title = document.createElement('div');
    title.className = 'text-sm font-semibold text-gray-900';
    title.textContent = section;
    sectionEl.appendChild(title);

    const list = document.createElement('ul');
    list.className = 'mt-2 list-disc pl-5 space-y-1 text-xs text-gray-700';
    for (const question of questions) {
      const item = document.createElement('li');
      item.textContent = question;
      list.appendChild(item);
    }
    sectionEl.appendChild(list);
    container.appendChild(sectionEl);
  }
}
