import { state } from '../app/state.js';
import { ENV, envBoolean } from '../app/env.js';

export function setupRecognition() {
  const SR = window.SpeechRecognition || window.webkitSpeechRecognition;
  if (!SR) return;
  state.recognition = new SR();
  state.recognition.lang = ENV.SPEECH_RECOGNITION_LANG;
  state.recognition.continuous = envBoolean('SPEECH_RECOGNITION_CONTINUOUS');
  state.recognition.interimResults = envBoolean('SPEECH_RECOGNITION_INTERIM_RESULTS');

  state.recognition.onresult = (e) => {
    let interim = '';
    for (let i = e.resultIndex; i < e.results.length; i++) {
      const text = e.results[i][0].transcript;
      if (e.results[i].isFinal) {
        state.session.finalTranscript += text + ' ';
      } else {
        interim += text;
      }
    }
    state.session.interimTranscript = interim;
    document.getElementById('transcript').textContent =
      (state.session.finalTranscript + interim).trim() || '—';
    document.getElementById('transcript').classList.remove('italic');
    if (state.session.finalTranscript.trim().length > 0) {
      document.getElementById('submitBtn').disabled = false;
    }
  };

  state.recognition.onerror = (e) => {
    console.error('Speech recognition error', e);
    if (e.error === 'no-speech') {
      document.getElementById('micStatus').textContent = "Didn't catch that — click the mic and try again.";
    } else if (e.error === 'not-allowed') {
      document.getElementById('micStatus').textContent = "Microphone permission denied. Allow it in browser settings and reload.";
    }
  };

  state.recognition.onend = () => {
    if (state.session.isListening) {
      // Restart if we're still supposed to be listening
      try { state.recognition.start(); } catch(e) {}
    }
  };
}

export function toggleMic() {
  if (!state.recognition) {
    alert("Your browser doesn't support speech recognition. Please use Chrome or Edge.");
    return;
  }
  state.session.isListening = !state.session.isListening;
  if (state.session.isListening) {
    state.session.finalTranscript = '';
    state.session.interimTranscript = '';
    document.getElementById('transcript').textContent = 'Listening...';
    document.getElementById('transcript').classList.add('italic');
    document.getElementById('micBtn').classList.add('pulse-mic');
    document.getElementById('micBtn').textContent = '⏹️';
    document.getElementById('waveContainer').classList.remove('hidden');
    document.getElementById('micStatus').textContent = 'Listening — click again when done answering';
    try { state.recognition.start(); } catch(e) {}
  } else {
    document.getElementById('micBtn').classList.remove('pulse-mic');
    document.getElementById('micBtn').textContent = '🎤';
    document.getElementById('waveContainer').classList.add('hidden');
    document.getElementById('micStatus').textContent = state.session.finalTranscript.trim()
      ? 'Click Submit to continue, or 🎤 to re-record.'
      : 'No speech detected. Click 🎤 to try again.';
    try { state.recognition.stop(); } catch(e) {}
  }
}

// =============== TEXT TO SPEECH ===============
