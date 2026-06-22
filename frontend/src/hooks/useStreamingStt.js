import { useCallback, useEffect, useRef, useState } from 'react';
import { ENV, envNumber } from '../app/env.js';

const TARGET_SAMPLE_RATE = envNumber('STT_SAMPLE_RATE');
const CHUNK_DURATION_MS = envNumber('STT_CHUNK_DURATION_MS');

const supported = typeof window !== 'undefined'
  && !!navigator.mediaDevices?.getUserMedia
  && !!(window.AudioContext || window.webkitAudioContext);

/// Captures microphone audio and streams 16-bit PCM chunks (via an AudioWorklet)
/// to the caller, which forwards them to the backend STT proxy. Unlike the old
/// Web Speech hook this does NOT produce transcript text — the server does, and
/// pushes it back over the WebSocket.
///
/// Callbacks: `onChunk(arrayBuffer)` per audio frame, `onStart()` when capture
/// begins, `onStop()` when it ends, `onError(code)` on failure.
export function useStreamingStt({ onChunk, onStart, onStop, onError } = {}) {
  const ctxRef = useRef(null);
  const streamRef = useRef(null);
  const nodeRef = useRef(null);
  const sourceRef = useRef(null);
  const listeningRef = useRef(false);

  const onChunkRef = useRef(onChunk);
  const onStartRef = useRef(onStart);
  const onStopRef = useRef(onStop);
  const onErrorRef = useRef(onError);
  useEffect(() => { onChunkRef.current = onChunk; });
  useEffect(() => { onStartRef.current = onStart; });
  useEffect(() => { onStopRef.current = onStop; });
  useEffect(() => { onErrorRef.current = onError; });

  const [isListening, setIsListening] = useState(false);

  const teardown = useCallback(() => {
    try { nodeRef.current?.port?.close?.(); } catch { /* ignore */ }
    try { nodeRef.current?.disconnect(); } catch { /* ignore */ }
    try { sourceRef.current?.disconnect(); } catch { /* ignore */ }
    try { streamRef.current?.getTracks().forEach((t) => t.stop()); } catch { /* ignore */ }
    try { ctxRef.current?.close(); } catch { /* ignore */ }
    nodeRef.current = null;
    sourceRef.current = null;
    streamRef.current = null;
    ctxRef.current = null;
  }, []);

  const stop = useCallback(() => {
    if (!listeningRef.current) return;
    listeningRef.current = false;
    setIsListening(false);
    teardown();
    onStopRef.current?.();
  }, [teardown]);

  const start = useCallback(async () => {
    if (listeningRef.current) return;
    if (!supported) { onErrorRef.current?.('unsupported'); return; }
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      streamRef.current = stream;

      const AudioCtx = window.AudioContext || window.webkitAudioContext;
      // Best-effort native resample to the target rate; the worklet also
      // decimates so we stay correct if the browser ignores sampleRate.
      const ctx = new AudioCtx({ sampleRate: TARGET_SAMPLE_RATE });
      ctxRef.current = ctx;
      if (ctx.state === 'suspended') { try { await ctx.resume(); } catch { /* ignore */ } }

      await ctx.audioWorklet.addModule(ENV.STT_WORKLET_URL);
      const source = ctx.createMediaStreamSource(stream);
      const node = new AudioWorkletNode(ctx, 'pcm-processor', {
        processorOptions: {
          targetRate: TARGET_SAMPLE_RATE,
          chunkDurationMs: CHUNK_DURATION_MS,
        },
      });
      node.port.onmessage = (e) => { onChunkRef.current?.(e.data); };
      source.connect(node);
      // Do not connect to destination — we don't want to play the mic back.
      sourceRef.current = source;
      nodeRef.current = node;

      listeningRef.current = true;
      setIsListening(true);
      onStartRef.current?.();
    } catch (err) {
      teardown();
      const name = err?.name;
      if (name === 'NotAllowedError' || name === 'SecurityError') onErrorRef.current?.('not-allowed');
      else onErrorRef.current?.('capture-failed');
    }
  }, [teardown]);

  const toggle = useCallback(() => {
    if (listeningRef.current) stop();
    else start();
  }, [start, stop]);

  useEffect(() => () => { listeningRef.current = false; teardown(); }, [teardown]);

  return { supported, isListening, start, stop, toggle };
}
