export function encodeSession(obj) {
  const json = JSON.stringify(obj);
  const compressed = pako.deflate(json);
  let binary = '';
  for (let i = 0; i < compressed.length; i++) binary += String.fromCharCode(compressed[i]);
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
}

export function decodeSession(str) {
  const b64 = str.replace(/-/g, '+').replace(/_/g, '/');
  const padded = b64 + '='.repeat((4 - b64.length % 4) % 4);
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  const json = pako.inflate(bytes, { to: 'string' });
  return JSON.parse(json);
}

export function readCandidateSessionFromURL() {
  const hash = window.location.hash;
  const match = hash.match(new RegExp(ENV.CANDIDATE_SESSION_HASH_KEY + '=([^&]+)'));
  if (!match) return null;
  try {
    return decodeSession(match[1]);
  } catch (e) {
    console.error('Failed to decode session:', e);
    return null;
  }
}
import { ENV } from '../app/env.js';
