import { useEffect, useRef, useState } from 'react';
import { useAppState } from '../providers/AppStateProvider.jsx';
import { useVoices } from '../hooks/useVoices.js';
import { testVoice } from '../voice/speech-synthesis.js';
import { envNumber } from '../app/env.js';
import {
  parseBankForEditor,
  serializeBank,
  hasBehavioralQuestions,
  appendSectionToBank,
  findCanonicalSectionName,
  parseBankForSummary,
  validateBank,
  removeSectionFromBank,
  renameSectionInBank,
} from '../interview/bank-editor.js';

const DEFAULT_NON_TECH_SECTION = 'Role Fit & Work Preferences';

export function HrConfigTab() {
  const { config, hasSavedConfig, saveConfig } = useAppState();
  const voiceGroups = useVoices();
  const emailRef = useRef(null);
  const questionsRef = useRef(null);
  const renameRef = useRef(null);

  const [editorVisible, setEditorVisible] = useState(!hasSavedConfig);
  const [form, setForm] = useState(() => ({
    hrEmail: config.hrEmail || '',
    techCount: config.techCount,
    passThreshold: config.passThreshold,
    voiceName: config.voiceName || '',
  }));
  const [bankText, setBankText] = useState(config.nonTechBank || '');
  const [sectionInput, setSectionInput] = useState(DEFAULT_NON_TECH_SECTION);
  const [activeSection, setActiveSection] = useState('');
  const [questionsText, setQuestionsText] = useState('');
  const [sectionRename, setSectionRename] = useState('');
  const [renamingSection, setRenamingSection] = useState('');
  const [saveStatus, setSaveStatus] = useState(false);
  const [addStatus, setAddStatus] = useState(false);

  // Focus the active section's question editor when it opens.
  useEffect(() => {
    if (activeSection && !renamingSection) questionsRef.current?.focus();
  }, [activeSection, renamingSection]);

  const firstUsLocal = voiceGroups
    .flatMap(g => g.voices)
    .find(v => v.lang === 'en-US' && v.localService);
  const selectValue = form.voiceName || firstUsLocal?.name || '';

  const editorBank = parseBankForEditor(bankText);
  const sectionEntries = Object.entries(editorBank);
  const questionsEditorVisible = activeSection && Object.hasOwn(editorBank, activeSection);
  const validation = validateBank(bankText);
  const behavioralCount = validation.stats.sectionCount;

  function setField(key, value) {
    setForm(prev => ({ ...prev, [key]: value }));
  }

  function loadFromConfig() {
    setForm({
      hrEmail: config.hrEmail || '',
      techCount: config.techCount,
      passThreshold: config.passThreshold,
      voiceName: config.voiceName || '',
    });
    setBankText(config.nonTechBank || '');
    setActiveSection('');
    setQuestionsText('');
    setSectionRename('');
    setRenamingSection('');
    setSectionInput(DEFAULT_NON_TECH_SECTION);
  }

  function handleSave() {
    const hrEmail = form.hrEmail.trim();
    if (!hrEmail) {
      alert('Please enter the report recipient email in HR Configuration.');
      return;
    }
    if (!emailRef.current?.checkValidity()) {
      alert('Please enter a valid report recipient email.');
      return;
    }
    const trimmedBank = bankText.trim();
    if (!hasBehavioralQuestions(trimmedBank)) {
      alert('Please add at least one behavioral question.');
      return;
    }
    const bankValidation = validateBank(trimmedBank);
    if (bankValidation.errors.length > 0) {
      alert(bankValidation.errors[0]);
      return;
    }
    saveConfig({
      hrEmail,
      techCount: parseInt(form.techCount, 10) || envNumber('DEFAULT_TECH_QUESTION_COUNT'),
      nonTechCount: bankValidation.stats.sectionCount,
      nonTechBank: trimmedBank,
      passThreshold: parseInt(form.passThreshold, 10) || envNumber('DEFAULT_PASS_THRESHOLD'),
      voiceName: selectValue,
    });
    setSaveStatus(true);
    setTimeout(() => setSaveStatus(false), 2500);
    setEditorVisible(false);
  }

  function handleAddSection() {
    const section = sectionInput.trim();
    if (!section) {
      alert('Please enter a section name.');
      return;
    }
    const nextBank = appendSectionToBank(bankText, section);
    setBankText(nextBank);
    const canonical = findCanonicalSectionName(nextBank, section);
    setActiveSection(canonical);
    setSectionRename(canonical);
    setRenamingSection('');
    setSectionInput('');
    setQuestionsText((parseBankForEditor(nextBank)[canonical] || []).join('\n'));
    setAddStatus(true);
    setTimeout(() => setAddStatus(false), 2500);
  }

  function selectSection(section) {
    setActiveSection(section);
    setSectionRename(section);
    setRenamingSection('');
    setQuestionsText((parseBankForEditor(bankText)[section] || []).join('\n'));
  }

  function handleQuestionsChange(value) {
    setQuestionsText(value);
    if (!activeSection) return;
    const bank = parseBankForEditor(bankText);
    if (!Object.hasOwn(bank, activeSection)) return;
    bank[activeSection] = value.split('\n').map(q => q.trim()).filter(Boolean);
    setBankText(serializeBank(bank));
  }

  function handleRenameSection(section = activeSection) {
    if (!section) return;
    const nextBank = renameSectionInBank(bankText, section, sectionRename);
    setBankText(nextBank);
    const canonical = findCanonicalSectionName(nextBank, sectionRename);
    setActiveSection(canonical);
    setSectionRename(canonical);
    setRenamingSection('');
    setQuestionsText((parseBankForEditor(nextBank)[canonical] || []).join('\n'));
  }

  function handleRemoveSection(section) {
    const nextBank = removeSectionFromBank(bankText, section);
    setBankText(nextBank);
    if (activeSection === section) {
      setActiveSection('');
      setSectionRename('');
      setRenamingSection('');
      setQuestionsText('');
    }
  }

  function startRenameSection(section) {
    setActiveSection(section);
    setSectionRename(section);
    setRenamingSection(section);
    setQuestionsText((parseBankForEditor(bankText)[section] || []).join('\n'));
    requestAnimationFrame(() => {
      renameRef.current?.focus();
      renameRef.current?.select();
    });
  }

  function handleEdit() {
    setEditorVisible(true);
    requestAnimationFrame(() => emailRef.current?.focus());
  }

  const summaryBank = parseBankForSummary(config.nonTechBank);
  const summarySections = Object.entries(summaryBank);

  return (
    <div>
      <h2 className="text-xl font-bold mb-4">HR Configuration</h2>
      <p className="text-sm text-gray-600 mb-4">
        Configure report delivery, question counts, behavioral sections, pass threshold, and
        interviewer voice. These HR settings are saved in this browser and reused for future
        interviews.
      </p>

      {hasSavedConfig && (
        <div className="mb-6 p-4 bg-indigo-50 border border-indigo-100 rounded-lg">
          <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-3">
            <div>
              <div className="text-sm font-semibold text-indigo-950">Saved configuration</div>
              <div className="text-xs text-indigo-900 mt-1">
                {[
                  config.hrEmail || 'No email set',
                  `${config.techCount} technical`,
                  `${summarySections.length} behavioral`,
                  `${summarySections.length} behavioral ${summarySections.length === 1 ? 'section' : 'sections'}`,
                  `${config.passThreshold}% pass threshold`,
                ].join(' • ')}
              </div>
            </div>
            <button className="btn-secondary text-sm self-start md:self-auto" onClick={handleEdit}>Edit</button>
          </div>
          <div className="mt-4 space-y-3">
            {summarySections.length === 0 && (
              <p className="text-xs text-indigo-900">No behavioral questions saved yet.</p>
            )}
            {summarySections.map(([section, questions]) => (
              <section key={section} className="bg-white border border-indigo-100 rounded-lg p-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="text-sm font-semibold text-gray-900">{section}</div>
                  <div className="text-xs font-semibold text-indigo-700">{questions.length}</div>
                </div>
                <ul className="mt-2 list-disc pl-5 space-y-1 text-xs text-gray-700">
                  {questions.map((q, i) => <li key={i}>{q}</li>)}
                </ul>
              </section>
            ))}
          </div>
        </div>
      )}

      {editorVisible && (
        <div>
          <div className="mb-6">
            <label className="block text-sm font-semibold mb-2">
              Report Recipient Email <span className="text-red-500">*</span>
            </label>
            <input
              ref={emailRef}
              type="email"
              className="input"
              placeholder="hr@yourcompany.com"
              value={form.hrEmail}
              onChange={e => setField('hrEmail', e.target.value)}
            />
            <p className="text-xs text-gray-500 mt-1">
              Interview reports are emailed to this recipient after every completed interview.
            </p>
          </div>

          <div className="mb-6">
            <div>
              <label className="block text-sm font-semibold mb-2">Number of Technical Questions</label>
              <input
                type="number" min="3" max="10" className="input"
                value={form.techCount}
                onChange={e => setField('techCount', e.target.value)}
              />
            </div>
          </div>

          <div className="mb-6">
            <label className="block text-sm font-semibold mb-2">Non-Technical (Behavioral) Question Bank</label>
            <p className="text-xs text-gray-500 mb-2">
              Group questions under category headers (lines starting with <code>##</code>). The
              interview asks <strong>one random question from each populated section</strong>, so
              the behavioral question count is {behavioralCount}.
            </p>
            <div className="mb-3 p-4 bg-gray-50 border border-gray-200 rounded-lg">
              {(validation.errors.length > 0 || validation.warnings.length > 0) && (
                <div className="mb-4 space-y-2">
                  {validation.errors.map(message => (
                    <div key={message} className="text-sm text-red-700 bg-red-50 border border-red-200 rounded-lg p-2">
                      {message}
                    </div>
                  ))}
                  {validation.warnings.map(message => (
                    <div key={message} className="text-sm text-amber-800 bg-amber-50 border border-amber-200 rounded-lg p-2">
                      {message}
                    </div>
                  ))}
                </div>
              )}

              <div className="mb-4">
                <label className="block text-xs font-semibold text-gray-600 mb-1">Section</label>
                <div className="flex flex-col md:flex-row gap-2">
                  <input
                    className="input"
                    placeholder="Role Fit & Work Preferences"
                    value={sectionInput}
                    onChange={e => setSectionInput(e.target.value)}
                  />
                  <button className="btn-secondary text-sm whitespace-nowrap" onClick={handleAddSection}>
                    Add Section
                  </button>
                </div>
              </div>

              <div className="grid lg:grid-cols-[minmax(220px,300px)_1fr] gap-4">
                <div>
                  <div className="text-xs font-semibold text-gray-600 mb-2">Sections</div>
                  <div className="space-y-2">
                    {sectionEntries.length === 0 && (
                      <p className="text-xs text-gray-500">No sections added yet.</p>
                    )}
                    {sectionEntries.map(([section, questions]) => (
                      <div key={section} className={`bg-white border rounded-lg p-2 ${section === activeSection ? 'border-indigo-300' : 'border-gray-200'}`}>
                        {renamingSection === section ? (
                          <div>
                            <input
                              ref={renameRef}
                              className="input"
                              value={sectionRename}
                              onChange={e => setSectionRename(e.target.value)}
                              onKeyDown={e => {
                                if (e.key === 'Enter') handleRenameSection(section);
                                if (e.key === 'Escape') {
                                  setRenamingSection('');
                                  setSectionRename(section);
                                }
                              }}
                            />
                            <div className="flex flex-wrap gap-1 mt-2">
                              <button className="btn-secondary text-xs px-2 py-1" onClick={() => handleRenameSection(section)}>Save</button>
                              <button className="btn-secondary text-xs px-2 py-1" onClick={() => { setRenamingSection(''); setSectionRename(section); }}>Cancel</button>
                            </div>
                          </div>
                        ) : (
                          <>
                            <button
                              type="button"
                              className="w-full text-left"
                              onClick={() => selectSection(section)}
                            >
                              <span className="block text-sm font-semibold text-gray-900">{section}</span>
                              <span className="text-xs text-gray-500">{questions.length} question{questions.length === 1 ? '' : 's'}</span>
                            </button>
                            <div className="flex flex-wrap gap-1 mt-2">
                              <button className="btn-secondary text-xs px-2 py-1" onClick={() => startRenameSection(section)}>Rename</button>
                              <button className="btn-secondary text-xs px-2 py-1" onClick={() => handleRemoveSection(section)}>Remove</button>
                            </div>
                          </>
                        )}
                      </div>
                    ))}
                  </div>
                </div>

                <div>
                  {questionsEditorVisible ? (
                    <div>
                      <label className="block text-xs font-semibold text-gray-600 mb-1">
                        Questions for {activeSection}
                      </label>
                      <textarea
                        ref={questionsRef}
                        className="textarea"
                        rows={8}
                        placeholder={'What kind of team structure do you work best in?\nHow do you prefer to receive feedback?'}
                        value={questionsText}
                        onChange={e => handleQuestionsChange(e.target.value)}
                      />
                      <p className="text-xs text-gray-500 mt-1">
                        Add each question on a new line. Each non-empty line is saved as a separate question.
                      </p>
                    </div>
                  ) : (
                    <div className="bg-white border border-gray-200 rounded-lg p-4 text-sm text-gray-600">
                      Select a section to edit its questions.
                    </div>
                  )}
                </div>
              </div>

              {addStatus && (
                <span className="block mt-3 text-sm text-green-600">Section added. Add questions below.</span>
              )}
            </div>
          </div>

          <div className="mb-6">
            <label className="block text-sm font-semibold mb-2">Passing Threshold (%)</label>
            <input
              type="number" min="50" max="100" className="input w-32"
              value={form.passThreshold}
              onChange={e => setField('passThreshold', e.target.value)}
            />
            <p className="text-xs text-gray-500 mt-1">
              Candidates need this score to advance to the human round.
            </p>
          </div>

          <div className="mb-6">
            <label className="block text-sm font-semibold mb-2">Interviewer Voice</label>
            <select
              className="input"
              value={selectValue}
              onChange={e => setField('voiceName', e.target.value)}
            >
              {voiceGroups.map(group => (
                <optgroup key={group.label} label={group.label}>
                  {group.voices.map(v => (
                    <option key={v.name} value={v.name}>{v.name} ({v.lang})</option>
                  ))}
                </optgroup>
              ))}
            </select>
            <button className="text-xs text-indigo-600 underline mt-1" onClick={() => testVoice()}>
              🔊 Test voice
            </button>
          </div>

          <div className="flex flex-wrap items-center gap-3">
            <button className="btn-primary" onClick={handleSave} disabled={validation.errors.length > 0}>Save Configuration</button>
            {hasSavedConfig && (
              <button className="btn-secondary" onClick={() => { loadFromConfig(); setEditorVisible(false); }}>
                Cancel
              </button>
            )}
            {saveStatus && <span className="text-sm text-green-600">✓ Saved</span>}
          </div>
        </div>
      )}
    </div>
  );
}
