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
    .map(([section, questions]) => [
      `## ${section}`,
      ...questions.map(question => question.trim()).filter(Boolean),
    ].join('\n'))
    .join('\n\n');
}

export function hasBehavioralQuestions(text) {
  return Object.values(parseBankForEditor(text)).some(questions => questions.length > 0);
}

export function bankStats(text) {
  const bank = parseBankForEditor(text);
  const sections = Object.entries(bank).map(([section, questions]) => ({
    section,
    count: questions.length,
  }));
  const populatedSections = sections.filter(({ count }) => count > 0).length;
  const totalQuestions = sections.reduce((sum, { count }) => sum + count, 0);
  return {
    sections,
    sectionCount: sections.length,
    populatedSections,
    emptySections: sections.filter(({ count }) => count === 0).length,
    totalQuestions,
  };
}

export function validateBank(text) {
  const stats = bankStats(text);
  const errors = [];
  const warnings = [];

  if (stats.emptySections > 0) {
    errors.push(`Add questions to ${stats.emptySections} empty section${stats.emptySections === 1 ? '' : 's'} or remove them.`);
  } else if (stats.totalQuestions === 0) {
    errors.push('Add at least one behavioral question.');
  }
  return { errors, warnings, stats };
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

export function removeSectionFromBank(text, section) {
  const bank = parseBankForEditor(text);
  delete bank[section];
  return serializeBank(bank);
}

export function renameSectionInBank(text, fromSection, toSection) {
  const nextName = toSection.trim();
  if (!nextName) return text;

  const bank = parseBankForEditor(text);
  if (!Object.hasOwn(bank, fromSection)) return text;

  const existing = Object.keys(bank).find(
    section => section !== fromSection && section.toLowerCase() === nextName.toLowerCase()
  );
  if (existing) {
    bank[existing] = [...bank[existing], ...bank[fromSection]];
    delete bank[fromSection];
    return serializeBank(bank);
  }

  const nextBank = {};
  for (const [section, questions] of Object.entries(bank)) {
    nextBank[section === fromSection ? nextName : section] = questions;
  }
  return serializeBank(nextBank);
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
