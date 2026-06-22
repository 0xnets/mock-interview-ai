/// Thin wrapper around `WebSocket` for the Phase 4 interview channel.
/// Translates between JSON frames and JS objects so callers don't have to.

import { ENV } from '../app/env.js';

export function openInterviewSocket({ joinNonce, wsPath = '/v1/ws/interview', onMessage, onOpen, onClose, onError }) {
  const base = (ENV.APP_API_BASE_URL || '').replace(/\/$/, '');
  const wsBase = base.replace(/^http(s?):/, 'ws$1:');
  const url = `${wsBase}${wsPath}?token=${encodeURIComponent(joinNonce)}`;
  const socket = new WebSocket(url);
  socket.addEventListener('open', () => { try { onOpen && onOpen(socket); } catch (e) { console.error(e); } });
  socket.addEventListener('close', (e) => { try { onClose && onClose(e); } catch (err) { console.error(err); } });
  socket.addEventListener('error', (e) => { try { onError && onError(e); } catch (err) { console.error(err); } });
  socket.addEventListener('message', (event) => {
    let parsed;
    try {
      parsed = JSON.parse(event.data);
    } catch (e) {
      console.error('Bad WS frame', e, event.data);
      return;
    }
    try {
      onMessage && onMessage(parsed);
    } catch (e) {
      console.error('WS message handler error', e, parsed);
    }
  });
  return socket;
}

export function sendMsg(socket, msg) {
  if (!socket || socket.readyState !== WebSocket.OPEN) return false;
  try {
    socket.send(JSON.stringify(msg));
    return true;
  } catch (e) {
    console.error('WS send failed', e);
    return false;
  }
}

/// Send a raw binary frame (e.g. a PCM audio chunk) to the backend.
export function sendBinary(socket, data) {
  if (!socket || socket.readyState !== WebSocket.OPEN) return false;
  try {
    socket.send(data);
    return true;
  } catch (e) {
    console.error('WS binary send failed', e);
    return false;
  }
}

/// Tell the backend the candidate is skipping the current question.
export function sendSkip(socket, ordinal) {
  return sendMsg(socket, { t: 'skip', ordinal });
}

export function closeSocket(socket) {
  if (!socket) return;
  try { socket.close(); } catch (_) { /* ignore */ }
}
