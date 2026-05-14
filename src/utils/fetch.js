export async function fetchWithAbort(url, init = {}, ms = 6000) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), ms);
  try {
    return await fetch(url, { ...init, signal: controller.signal });
  } catch (e) {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

// Cleanuri.com — most reliable CORS-supporting shortener
