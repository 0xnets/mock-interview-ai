import { fetchSessionByCode, issueJoinNonce, waitForPrimed } from '../api/client.js';

const QUERY_KEY = 'session';

function readShortcodeFromQuery() {
  const params = new URLSearchParams(window.location.search);
  const value = params.get(QUERY_KEY);
  return value ? value.trim() : null;
}

/// Resolve the candidate landing URL into `{ session, nonce }`. Waits if the
/// backend is still priming, then mints a single-use WS join nonce. Returns
/// `null` when there's no `?session=` in the URL.
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
    const nonceInfo = await issueJoinNonce(shortcode);
    return {
      shortcode,
      session,
      nonce: nonceInfo.join_nonce,
      wsPath: nonceInfo.ws_path,
    };
  } catch (e) {
    console.error('Failed to load session by code:', e);
    alert(`This interview link could not be loaded.\n${e.message}`);
    return null;
  }
}
