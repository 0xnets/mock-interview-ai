import { state } from '../app/state.js';
import { ENV, envNumber } from '../app/env.js';
import { ensureVoicesLoaded, getSelectedVoice } from './voice-selection.js';

export function testVoice() {
  speak("Hello! This is how I'll sound during the interview. Let's get started when you're ready.");
}

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
    utter.onend = resolve;
    utter.onerror = resolve;
    speechSynthesis.speak(utter);
  });
}

export function speakCurrentQuestion() {
  const text = state.session.currentQuestionText;
  if (text) speak(text);
}
