import { envNumber } from '../app/env.js';

// HR's saved voice name. Set from AppStateProvider whenever config loads/saves,
// so the framework-agnostic `speak()` can resolve a voice without React.
let savedVoiceName = '';

export function setSavedVoiceName(name) {
  savedVoiceName = name || '';
}

function englishVoices() {
  if (!('speechSynthesis' in window)) return [];
  return speechSynthesis.getVoices().filter(v => v.lang.startsWith('en'));
}

/// Grouped English voices for the config <select>:
/// local US > local other > remote US > remote other.
export function listEnglishVoices() {
  const voices = englishVoices();
  return [
    { label: '✓ US voices installed on this device', voices: voices.filter(v => v.lang === 'en-US' && v.localService) },
    { label: '✓ Other English voices installed', voices: voices.filter(v => v.lang !== 'en-US' && v.localService) },
    { label: '⚠️ Cloud US voices (may not work reliably)', voices: voices.filter(v => v.lang === 'en-US' && !v.localService) },
    { label: '⚠️ Other cloud voices', voices: voices.filter(v => v.lang !== 'en-US' && !v.localService) },
  ].filter(g => g.voices.length > 0);
}

/// Pick the best available voice given the device's voices and HR's saved choice.
export function pickVoice(voices, savedName) {
  if (!voices || voices.length === 0) return null;

  // Priority 1: Google US voices (consistent across all Chrome platforms)
  const googleUS = voices.find(v => /^Google US English/i.test(v.name) && v.lang === 'en-US');
  if (googleUS) return googleUS;

  // Priority 2: HR's saved choice — only if it actually exists on this device
  if (savedName) {
    const v = voices.find(v => v.name === savedName && v.localService);
    if (v) return v;
  }

  // Priority 3: High-quality neural/natural local US voices
  const hqKeywords = /natural|neural|premium|enhanced|wavenet|studio/i;
  const hqLocal = voices.find(v => v.lang === 'en-US' && v.localService && hqKeywords.test(v.name));
  if (hqLocal) return hqLocal;

  // Priority 4: Reliable installed-by-default voices on Mac/Windows
  const reliableNames = /samantha|albert|david|zira|mark|guy|aria/i;
  const reliable = voices.find(v => v.lang === 'en-US' && v.localService && reliableNames.test(v.name));
  if (reliable) return reliable;

  // Priority 5: Any local US voice
  const anyLocalUS = voices.find(v => v.lang === 'en-US' && v.localService);
  if (anyLocalUS) return anyLocalUS;

  // Priority 6: Any US voice (even remote — better than no US accent)
  const anyUS = voices.find(v => v.lang === 'en-US');
  if (anyUS) return anyUS;

  // Priority 7: Any English voice
  const anyEN = voices.find(v => v.lang.startsWith('en'));
  if (anyEN) return anyEN;

  return null;
}

export function getSelectedVoice() {
  return pickVoice(englishVoices(), savedVoiceName);
}

export async function ensureVoicesLoaded() {
  if (englishVoices().length > 0) return englishVoices();
  for (let i = 0; i < envNumber('VOICE_LOAD_MAX_ATTEMPTS'); i++) {
    await new Promise(r => setTimeout(r, envNumber('VOICE_LOAD_RETRY_DELAY_MS')));
    if (englishVoices().length > 0) return englishVoices();
  }
  return englishVoices();
}
