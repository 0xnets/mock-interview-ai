import { fetchSessionByCode } from '../api/client.js';
import { ensureVoicesLoaded, getSelectedVoice } from '../voice/voice-selection.js';
import { speak } from '../voice/speech-synthesis.js';

const MIC_LEVEL_SAMPLE_MS = 2500;
const MIC_MIN_RMS_LEVEL = 0.015;

const SpeechRecognitionApi = () => (
  typeof window !== 'undefined'
    ? (window.SpeechRecognition || window.webkitSpeechRecognition)
    : null
);

function browserName() {
  const ua = navigator.userAgent || '';
  const vendor = navigator.vendor || '';
  const isEdge = /\bEdg\//.test(ua);
  const isChrome = /\bChrome\//.test(ua) && /Google Inc/.test(vendor) && !isEdge;
  const isChromium = /\bChrome\//.test(ua) || /\bChromium\//.test(ua) || isEdge;
  if (isEdge) return 'Microsoft Edge';
  if (isChrome) return 'Google Chrome';
  if (isChromium) return 'Chromium-based browser';
  if (/\bFirefox\//.test(ua)) return 'Firefox';
  if (/\bSafari\//.test(ua)) return 'Safari';
  return 'this browser';
}

function checkBrowser() {
  const missing = [];
  if (!window.isSecureContext) missing.push('secure HTTPS or localhost context');
  if (!navigator.mediaDevices?.getUserMedia) missing.push('microphone permission API');
  if (!SpeechRecognitionApi()) missing.push('speech recognition');
  if (!('speechSynthesis' in window)) missing.push('speech synthesis');

  if (missing.length > 0) {
    return {
      ok: false,
      detail: `${browserName()} is missing ${missing.join(', ')}. Use the latest Chrome or Edge on HTTPS or localhost.`,
    };
  }

  return {
    ok: true,
    detail: `${browserName()} supports the interview browser APIs.`,
  };
}

async function measureMicrophoneLevel(stream, onProgress) {
  const AudioContextApi = window.AudioContext || window.webkitAudioContext;
  if (!AudioContextApi) {
    return { ok: false, detail: 'Web Audio is not available for the microphone level test.' };
  }

  let ctx;
  try {
    onProgress?.('Speak a few words now. Checking microphone input level…');
    ctx = new AudioContextApi();
    await ctx.resume();

    const source = ctx.createMediaStreamSource(stream);
    const analyser = ctx.createAnalyser();
    analyser.fftSize = 2048;
    source.connect(analyser);

    const samples = new Uint8Array(analyser.fftSize);
    const deadline = performance.now() + MIC_LEVEL_SAMPLE_MS;
    let maxRms = 0;

    while (performance.now() < deadline) {
      analyser.getByteTimeDomainData(samples);
      let sumSquares = 0;
      for (const value of samples) {
        const centered = (value - 128) / 128;
        sumSquares += centered * centered;
      }
      maxRms = Math.max(maxRms, Math.sqrt(sumSquares / samples.length));
      await new Promise(resolve => requestAnimationFrame(resolve));
    }

    if (maxRms < MIC_MIN_RMS_LEVEL) {
      return {
        ok: false,
        detail: `Microphone is connected, but input was too quiet. Speak closer to the mic and retry. Peak level: ${maxRms.toFixed(3)}.`,
      };
    }

    return {
      ok: true,
      detail: `Microphone is receiving voice input. Peak level: ${maxRms.toFixed(3)}.`,
    };
  } catch (e) {
    return { ok: false, detail: `Could not measure microphone input: ${e?.message || 'unknown error'}.` };
  } finally {
    try { await ctx?.close(); } catch { /* ignore */ }
  }
}

async function checkMicrophone({ onProgress } = {}) {
  let stream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    const audioTracks = stream.getAudioTracks();
    if (audioTracks.length === 0) {
      return { ok: false, detail: 'No microphone track was provided by the browser.' };
    }
    return measureMicrophoneLevel(stream, onProgress);
  } catch (e) {
    const denied = e?.name === 'NotAllowedError' || e?.name === 'SecurityError';
    return {
      ok: false,
      detail: denied
        ? 'Microphone permission was denied. Allow microphone access in browser settings and retry.'
        : `Could not access the microphone: ${e?.message || e?.name || 'unknown error'}.`,
    };
  } finally {
    stream?.getTracks().forEach(track => track.stop());
  }
}

async function checkSpeaker() {
  const AudioContextApi = window.AudioContext || window.webkitAudioContext;
  if (!AudioContextApi) {
    return { ok: false, detail: 'Web Audio is not available for the speaker test.' };
  }

  let ctx;
  try {
    ctx = new AudioContextApi();
    await ctx.resume();
    const oscillator = ctx.createOscillator();
    const gain = ctx.createGain();
    oscillator.type = 'sine';
    oscillator.frequency.value = 660;
    gain.gain.setValueAtTime(0.0001, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.08, ctx.currentTime + 0.03);
    gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.22);
    oscillator.connect(gain).connect(ctx.destination);
    oscillator.start();
    oscillator.stop(ctx.currentTime + 0.25);
    await new Promise(resolve => {
      oscillator.onended = resolve;
    });
    return {
      ok: true,
      detail: 'Speaker test tone played.',
      needsConfirmation: {
        question: 'Did you hear the speaker test tone?',
        passDetail: 'Speaker output confirmed by the candidate.',
        failDetail: 'Speaker test tone was not heard. Check volume, output device, headphones, and browser tab audio, then retry.',
      },
    };
  } catch (e) {
    return { ok: false, detail: `Could not play the speaker test tone: ${e?.message || 'unknown error'}.` };
  } finally {
    try { await ctx?.close(); } catch { /* ignore */ }
  }
}

async function checkNetwork(shortcode) {
  if (!shortcode) {
    return { ok: false, detail: 'Interview session code is missing.' };
  }

  const session = await fetchSessionByCode(shortcode);
  if (!session?.state || session.state === 'expired') {
    return { ok: false, detail: 'The interview session is not available.' };
  }
  if (session.state === 'pending') {
    return { ok: false, detail: 'The interview is still preparing. Retry in a moment.' };
  }
  return { ok: true, detail: 'Backend connection and interview session are reachable.' };
}

async function checkVoiceSynthesis() {
  if (!('speechSynthesis' in window)) {
    return { ok: false, detail: 'Speech synthesis is not supported in this browser.' };
  }

  const voices = await ensureVoicesLoaded();
  const voice = getSelectedVoice();
  if (voices.length === 0 || !voice) {
    return { ok: false, detail: 'No English speech synthesis voice is available on this device.' };
  }

  await speak('Voice synthesis check complete.');
  return { ok: true, detail: `${voice.name} (${voice.lang}) is ready.` };
}

// `systemCheck: true` marks failures that count as system incompatibility — the
// device or browser cannot run the interview. The network check is excluded:
// it is transient/backend, not a device limitation.
export const PRE_INTERVIEW_CHECKS = [
  { id: 'browser', label: 'Browser', run: checkBrowser, systemCheck: true },
  { id: 'microphone', label: 'Microphone', run: checkMicrophone, systemCheck: true },
  { id: 'speaker', label: 'Speaker', run: checkSpeaker, systemCheck: true },
  { id: 'network', label: 'Network', run: ({ shortcode }) => checkNetwork(shortcode), systemCheck: false },
  { id: 'voice', label: 'Voice synthesis', run: checkVoiceSynthesis, systemCheck: true },
];
