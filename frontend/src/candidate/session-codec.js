import { fetchSessionByCode, waitForPrimed } from '../api/client.js';

const QUERY_KEY = 'session';

function readShortcodeFromQuery() {
  const params = new URLSearchParams(window.location.search);
  const value = params.get(QUERY_KEY);
  return value ? value.trim() : null;
}

/// Resolve the candidate landing URL into `{ shortcode, session }`. Waits if the
/// backend is still priming. The single-use join nonce is NOT minted here — it
/// is minted when the candidate clicks "Start Interview", so a failed
/// pre-interview system check never consumes the link. Returns `null` when
/// there's no `?session=` in the URL.
export async function readCandidateSessionFromURL({ onWaiting } = {}) {
  const shortcode = readShortcodeFromQuery();
  if (!shortcode) return null;

  try {
    const initial = await fetchSessionByCode(shortcode);
    let session = initial;
    if (initial.state === 'pending') {
      if (onWaiting) onWaiting({ state: initial.state, attempt: 1 });
      session = await waitForPrimed(shortcode, { onTick: onWaiting });
    }
    return { shortcode, session };
  } catch (e) {
    console.error('Failed to load session by code:', e);
    return null;
  }
}
