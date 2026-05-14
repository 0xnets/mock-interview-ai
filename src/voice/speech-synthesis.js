import { state } from '../app/state.js';
import { ENV, envNumber } from '../app/env.js';
import { ensureVoicesLoaded, getSelectedVoice } from './voice-selection.js';

export function testVoice() {
  speak("Hello! This is how I'll sound during the interview. Let's get started when you're ready.");
}

export function testWelcomeVoice() {
  const name = state.session.candidateName || 'there';
  speak(`Hi ${name}! This is the voice you'll hear during your interview. If it sounds clear and natural, check the box below and click Start when ready.`);
}

export function onVoiceSelectChange() {
  // When voice changes, uncheck the confirmed box and re-test
  const cb = document.getElementById('voiceConfirmed');
  if (cb) cb.checked = false;
  updateStartButton();
  testWelcomeVoice();
}

export function updateStartButton() {
  const cb = document.getElementById('voiceConfirmed');
  const btn = document.getElementById('welcomeStartBtn');
  const hint = document.getElementById('startBtnHint');
  if (!btn || !cb) return;
  const ok = cb.checked;
  btn.disabled = !ok;
  if (hint) hint.style.display = ok ? 'none' : 'block';
}

// =============== TABS / SCREENS ===============

export async function speak(text) {
  if (!('speechSynthesis' in window)) return;
  await ensureVoicesLoaded();
  return new Promise((resolve) => {
    speechSynthesis.cancel();
    const utter = new SpeechSynthesisUtterance(text);
    const voice = getSelectedVoice();
    if (voice) {
      utter.voice = voice;
      // Use the voice's OWN language — overriding it with 'en-US' can cause
      // some browsers (esp. Chrome on macOS) to substitute a different voice.
      utter.lang = voice.lang;
    } else {
      utter.lang = ENV.SPEECH_SYNTHESIS_FALLBACK_LANG;
    }
    utter.rate = envNumber('SPEECH_SYNTHESIS_RATE');
    utter.pitch = envNumber('SPEECH_SYNTHESIS_PITCH');
    // Show what's actually playing so candidate can verify
    const indicator = document.getElementById('voiceInUse');
    if (indicator && voice) {
      indicator.textContent = `Speaking with: ${voice.name}${voice.localService ? ' ✓ Local' : ' ⚠️ Remote (may not work)'}`;
    }
    utter.onend = resolve;
    utter.onerror = resolve;
    speechSynthesis.speak(utter);
  });
}

export function speakCurrentQuestion() {
  const q = state.session.allQuestions[state.session.currentIdx];
  if (q) speak(q.question);
}

// =============== GOOGLE GEMINI API ===============
