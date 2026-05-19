import { getTranscript } from '../api/client.js';
import { showScreen } from '../ui/screens.js';
import { state } from '../app/state.js';

function setText(id, value) {
  const el = document.getElementById(id);
  if (el) el.textContent = value;
}

function setHidden(id, hidden) {
  const el = document.getElementById(id);
  if (el) el.classList.toggle('hidden', hidden);
}

function decodeBase64Utf8(b64) {
  try {
    const bin = atob(b64.replace(/-/g, '+').replace(/_/g, '/'));
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    return new TextDecoder('utf-8', { fatal: false }).decode(bytes);
  } catch {
    return '';
  }
}

export async function openTranscriptForCurrentSession() {
  const sid = state.session.sessionId;
  if (!sid) {
    alert('No session selected. Finish or open an interview first.');
    return;
  }
  showScreen('transcript');
  setText('transcriptSessionId', sid);
  setText('transcriptPublicKey', '…');
  setText('transcriptChainStatus', '');
  const chunksDiv = document.getElementById('transcriptChunks');
  const cpDiv = document.getElementById('transcriptCheckpoints');
  if (chunksDiv) chunksDiv.innerHTML = '<div class="text-sm text-gray-500">Loading…</div>';
  if (cpDiv) cpDiv.innerHTML = '';
  setHidden('transcriptError', true);

  try {
    const t = await getTranscript(sid);
    setText('transcriptPublicKey', t.public_key_b64 || '—');
    if (chunksDiv) {
      chunksDiv.innerHTML = '';
      for (const c of (t.chunks || [])) {
        const row = document.createElement('div');
        row.className = 'p-3 bg-gray-50 border border-gray-200 rounded';
        const decoded = decodeBase64Utf8(c.canonical_row_b64);
        row.innerHTML = `
          <div class="text-xs text-gray-500 mb-1">#${c.seq} · ${escapeHtml(c.ordinal_or_section || '')}</div>
          <pre class="text-xs whitespace-pre-wrap break-words">${escapeHtml(decoded || c.canonical_row_b64)}</pre>
          <div class="text-[10px] font-mono text-gray-400 mt-1">chain: ${escapeHtml(c.chain_hash_b64 || '')}</div>
        `;
        chunksDiv.appendChild(row);
      }
    }
    if (cpDiv) {
      cpDiv.innerHTML = '';
      for (const cp of (t.checkpoints || [])) {
        const row = document.createElement('div');
        row.className = 'p-2 bg-gray-50 border border-gray-200 rounded';
        row.innerHTML = `
          <div>#${cp.seq} · signed_at <span class="text-gray-500">${escapeHtml(cp.signed_at || '')}</span></div>
          <div class="text-gray-500">chain: ${escapeHtml(cp.chain_hash_b64 || '')}</div>
          <div class="text-gray-500 break-all">sig: ${escapeHtml(cp.signature_b64 || '')}</div>
        `;
        cpDiv.appendChild(row);
      }
    }
    setText('transcriptChainStatus', `${(t.chunks || []).length} chunk(s), ${(t.checkpoints || []).length} checkpoint(s).`);
  } catch (e) {
    const err = document.getElementById('transcriptError');
    if (err) {
      err.textContent = e.message || 'Failed to load transcript.';
      err.classList.remove('hidden');
    }
  }
}

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}
