import { state } from '../app/state.js';
import { envNumber } from '../app/env.js';

export function loadVoices() {
  if (!('speechSynthesis' in window)) return;
  state.voices = speechSynthesis.getVoices().filter(v => v.lang.startsWith('en'));
  const sel = document.getElementById('voiceSelect');
  if (!sel) return;
  sel.innerHTML = '';
  // Group voices: local US > local other > remote US > remote other
  const usLocal = state.voices.filter(v => v.lang === 'en-US' && v.localService);
  const otherLocal = state.voices.filter(v => v.lang !== 'en-US' && v.localService);
  const usRemote = state.voices.filter(v => v.lang === 'en-US' && !v.localService);
  const otherRemote = state.voices.filter(v => v.lang !== 'en-US' && !v.localService);

  const addGroup = (label, voices) => {
    if (!voices.length) return;
    const grp = document.createElement('optgroup');
    grp.label = label;
    voices.forEach(v => {
      const opt = document.createElement('option');
      opt.value = v.name;
      opt.textContent = `${v.name} (${v.lang})`;
      if (v.name === state.config.voiceName) opt.selected = true;
      grp.appendChild(opt);
    });
    sel.appendChild(grp);
  };
  addGroup('✓ US voices installed on this device', usLocal);
  addGroup('✓ Other English voices installed', otherLocal);
  addGroup('⚠️ Cloud US voices (may not work reliably)', usRemote);
  addGroup('⚠️ Other cloud voices', otherRemote);

  if (!state.config.voiceName && usLocal.length > 0) {
    sel.value = usLocal[0].name;
  }
}

export function getSelectedVoice() {
  if (!state.voices || state.voices.length === 0) return null;

  // STRATEGY: Prefer voices that exist consistently across devices.
  //
  // "Google US English" / "Google US English Female" / "Google US English Male"
  // ship WITH Chrome itself on Mac, Windows, Linux, and Android. Same voice everywhere
  // for any candidate using Chrome. This gives us the closest thing to a "universal"
  // voice without paying for a cloud TTS service.

  // Priority 1: Google US voices (consistent across all Chrome platforms)
  const googleUS = state.voices.find(v =>
    /^Google US English/i.test(v.name) && v.lang === 'en-US'
  );
  if (googleUS) return googleUS;

  // Priority 2: HR's saved choice — only used if it actually exists on this device
  if (state.config.voiceName) {
    const v = state.voices.find(v => v.name === state.config.voiceName && v.localService);
    if (v) return v;
  }

  // Priority 3: High-quality neural/natural local US voices
  const hqKeywords = /natural|neural|premium|enhanced|wavenet|studio/i;
  const hqLocal = state.voices.find(v =>
    v.lang === 'en-US' && v.localService && hqKeywords.test(v.name)
  );
  if (hqLocal) return hqLocal;

  // Priority 4: Reliable installed-by-default voices on Mac/Windows
  // Samantha/Albert ship with every Mac; Microsoft David/Zira ship with every Windows.
  const reliableNames = /samantha|albert|david|zira|mark|guy|aria/i;
  const reliable = state.voices.find(v =>
    v.lang === 'en-US' && v.localService && reliableNames.test(v.name)
  );
  if (reliable) return reliable;

  // Priority 5: Any local US voice
  const anyLocalUS = state.voices.find(v => v.lang === 'en-US' && v.localService);
  if (anyLocalUS) return anyLocalUS;

  // Priority 6: Any US voice (even remote — better than no US accent)
  const anyUS = state.voices.find(v => v.lang === 'en-US');
  if (anyUS) return anyUS;

  // Priority 7: Any English voice
  const anyEN = state.voices.find(v => v.lang.startsWith('en'));
  if (anyEN) return anyEN;

  // Last resort: let the browser pick
  return null;
}

export async function ensureVoicesLoaded() {
  if (state.voices && state.voices.length > 0) return;
  for (let i = 0; i < envNumber('VOICE_LOAD_MAX_ATTEMPTS'); i++) {
    loadVoices();
    if (state.voices && state.voices.length > 0) return;
    await new Promise(r => setTimeout(r, envNumber('VOICE_LOAD_RETRY_DELAY_MS')));
  }
}

export function populateWelcomeVoicePicker() {
  const sel = document.getElementById('welcomeVoiceSelect');
  if (!sel || !state.voices) return;
  // Local voices first (installed on device) — these actually work reliably.
  // Remote/cloud voices may be listed but silently fail or substitute.
  const usLocal = state.voices.filter(v => v.lang === 'en-US' && v.localService);
  const usRemote = state.voices.filter(v => v.lang === 'en-US' && !v.localService);
  const otherLocal = state.voices.filter(v => v.lang.startsWith('en') && v.lang !== 'en-US' && v.localService);
  const otherRemote = state.voices.filter(v => v.lang.startsWith('en') && v.lang !== 'en-US' && !v.localService);

  sel.innerHTML = '<option value="">Auto-pick best voice</option>';
  const addGroup = (label, voices) => {
    if (!voices.length) return;
    const grp = document.createElement('optgroup');
    grp.label = label;
    voices.forEach(v => {
      const opt = document.createElement('option');
      opt.value = v.name;
      opt.textContent = `${v.name} (${v.lang})`;
      grp.appendChild(opt);
    });
    sel.appendChild(grp);
  };
  addGroup('✓ Installed on this device (recommended)', usLocal);
  addGroup('✓ Other English (installed)', otherLocal);
  addGroup('⚠️ Cloud / may not work reliably', usRemote);
  addGroup('⚠️ Other cloud voices', otherRemote);

  const auto = getSelectedVoice();
  if (auto) sel.value = auto.name;
}
