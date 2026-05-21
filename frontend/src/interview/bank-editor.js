/// Pure helpers for the HR behavioral-question-bank editor. They operate on the
/// same `## Section` + question-line text format that `parseNonTechBank` reads.
import { parseNonTechBank } from './non-tech-bank.js';

/// Parse for the editor: a legacy non-section bank (`_default`) is surfaced under
/// a visible "Behavioral Questions" heading.
export function parseBankForEditor(text) {
  const bank = parseNonTechBank(text);
  if (bank._default) {
    bank['Behavioral Questions'] = bank._default;
    delete bank._default;
  }
  return bank;
}

export function serializeBank(bank) {
  return Object.entries(bank)
    .map(([section, questions]) => [`## ${section}`, ...questions].join('\n'))
    .join('\n\n');
}

export function hasBehavioralQuestions(text) {
  return Object.values(parseBankForEditor(text)).some(questions => questions.length > 0);
}

export function appendSectionToBank(text, section) {
  const bank = parseBankForEditor(text);
  const existing = Object.keys(bank).find(name => name.toLowerCase() === section.toLowerCase());
  if (existing) return serializeBank(bank);
  bank[section] = [];
  return serializeBank(bank);
}

export function findCanonicalSectionName(text, section) {
  const bank = parseBankForEditor(text);
  return Object.keys(bank).find(name => name.toLowerCase() === section.toLowerCase()) || section;
}

/// Sections that actually have questions, for the saved-config summary.
export function parseBankForSummary(text) {
  const bank = parseNonTechBank(text);
  return Object.fromEntries(
    Object.entries(bank)
      .filter(([, questions]) => questions.length > 0)
      .map(([section, questions]) => [
        section === '_default' ? 'Behavioral Questions' : section,
        questions,
      ])
  );
}
