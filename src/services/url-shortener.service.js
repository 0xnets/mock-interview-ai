import { fetchWithAbort } from '../utils/fetch.js';
import { ENV, envNumber } from '../app/env.js';

export async function tryCleanuri(longUrl) {
  const resp = await fetchWithAbort(ENV.CLEANURI_SHORTEN_URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: 'url=' + encodeURIComponent(longUrl)
  }, envNumber('URL_SHORTENER_TIMEOUT_MS'));
  if (!resp || !resp.ok) return null;
  try {
    const data = await resp.json();
    return data.result_url && data.result_url.startsWith('http') ? data.result_url : null;
  } catch (e) { return null; }
}

// is.gd — GET API, CORS but sometimes flaky

export async function tryIsGd(longUrl) {
  const resp = await fetchWithAbort(ENV.IS_GD_SHORTEN_URL + encodeURIComponent(longUrl), {}, envNumber('URL_SHORTENER_TIMEOUT_MS'));
  if (!resp || !resp.ok) return null;
  const text = (await resp.text()).trim();
  return text.startsWith('http') ? text : null;
}

// da.gd — another GET API

export async function tryDaGd(longUrl) {
  const resp = await fetchWithAbort(ENV.DA_GD_SHORTEN_URL + encodeURIComponent(longUrl), {}, envNumber('URL_SHORTENER_TIMEOUT_MS'));
  if (!resp || !resp.ok) return null;
  const text = (await resp.text()).trim();
  return text.startsWith('http') ? text : null;
}

export async function shortenUrl(longUrl) {
  // Try in parallel — first one to return a valid short URL wins.
  // Cleanuri is most reliable; the others are insurance.
  try {
    return await Promise.any([
      tryCleanuri(longUrl),
      tryIsGd(longUrl),
      tryDaGd(longUrl)
    ].map(p => p.then(r => r || Promise.reject('no result'))));
  } catch (e) {
    return null;
  }
}
