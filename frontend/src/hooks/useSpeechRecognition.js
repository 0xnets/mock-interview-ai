import { useCallback, useEffect, useRef, useState } from 'react';
import { ENV, envBoolean } from '../app/env.js';

const SR = typeof window !== 'undefined'
  ? (window.SpeechRecognition || window.webkitSpeechRecognition)
  : null;

/// Web Speech recognition for the interview screen. Preserves the original
/// behavior: ENV-driven config, each finalized chunk streamed via `onFinalChunk`,
/// interim text exposed live, and auto-restart while still listening.
export function useSpeechRecognition({ onFinalChunk, onError } = {}) {
  const recognitionRef = useRef(null);
  const listeningRef = useRef(false);
  const finalRef = useRef('');
  const onFinalChunkRef = useRef(onFinalChunk);
  const onErrorRef = useRef(onError);

  const [isListening, setIsListening] = useState(false);
  const [finalTranscript, setFinalTranscript] = useState('');
  const [interimTranscript, setInterimTranscript] = useState('');

  useEffect(() => { onFinalChunkRef.current = onFinalChunk; });
  useEffect(() => { onErrorRef.current = onError; });

  useEffect(() => {
    if (!SR) return undefined;
    const rec = new SR();
    rec.lang = ENV.SPEECH_RECOGNITION_LANG;
    rec.continuous = envBoolean('SPEECH_RECOGNITION_CONTINUOUS');
    rec.interimResults = envBoolean('SPEECH_RECOGNITION_INTERIM_RESULTS');

    rec.onresult = (e) => {
      let interim = '';
      for (let i = e.resultIndex; i < e.results.length; i++) {
        const text = e.results[i][0].transcript;
        if (e.results[i].isFinal) {
          finalRef.current += text + ' ';
          setFinalTranscript(finalRef.current);
          // Stream each finalized chunk so the backend's answer text matches
          // what the candidate sees.
          onFinalChunkRef.current?.(text.trim());
        } else {
          interim += text;
        }
      }
      setInterimTranscript(interim);
    };
    rec.onerror = (e) => {
      console.error('Speech recognition error', e);
      onErrorRef.current?.(e.error);
    };
    rec.onend = () => {
      if (listeningRef.current) {
        try { rec.start(); } catch { /* already running */ }
      }
    };

    recognitionRef.current = rec;
    return () => {
      listeningRef.current = false;
      try { rec.stop(); } catch { /* ignore */ }
      recognitionRef.current = null;
    };
  }, []);

  const reset = useCallback(() => {
    finalRef.current = '';
    setFinalTranscript('');
    setInterimTranscript('');
  }, []);

  const stop = useCallback(() => {
    if (!listeningRef.current) return;
    listeningRef.current = false;
    setIsListening(false);
    try { recognitionRef.current?.stop(); } catch { /* ignore */ }
  }, []);

  const toggle = useCallback(() => {
    if (!recognitionRef.current) {
      onErrorRef.current?.('unsupported');
      return;
    }
    if (listeningRef.current) {
      stop();
      return;
    }
    listeningRef.current = true;
    setIsListening(true);
    finalRef.current = '';
    setFinalTranscript('');
    setInterimTranscript('');
    try { recognitionRef.current.start(); } catch { /* ignore */ }
  }, [stop]);

  return {
    supported: !!SR,
    isListening,
    finalTranscript,
    interimTranscript,
    toggle,
    stop,
    reset,
  };
}
