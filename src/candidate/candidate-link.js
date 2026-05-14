import { prepareCandidateSession } from '../interview/question-generation.js';
import { ENV } from '../app/env.js';
import { encodeSession } from './session-codec.js';
import { shortenUrl } from '../services/url-shortener.service.js';

export async function generateCandidateLink(event) {
  const name = document.getElementById('linkName').value.trim();
  const role = document.getElementById('linkRole').value.trim();
  const jd = document.getElementById('linkJd').value.trim();
  const resume = document.getElementById('linkResume').value.trim();
  let base = document.getElementById('linkBaseUrl').value.trim();

  if (!name || !role || !jd || !resume) {
    alert('Please fill in all four fields.');
    return;
  }

  if (!base) base = window.location.href.split('#')[0];
  if (!base.endsWith('/') && !base.endsWith('.html')) base += '/';

  const linkInput = document.getElementById('generatedLink');
  const box = document.getElementById('generatedLinkBox');
  const info = document.getElementById('linkLengthInfo');
  const btn = event?.currentTarget;
  if (btn) btn.disabled = true;

  box.classList.remove('hidden');
  linkInput.value = '⏳ Preparing session...';
  info.innerHTML = '<span class="text-gray-600">This takes about 10-15 seconds — the AI generates personalized questions now so the candidate experience is instant.</span>';

  try {
    const session = await prepareCandidateSession(name, role, jd, resume, (msg) => {
      info.innerHTML = `<span class="text-gray-600">${msg}</span>`;
    });

    const encoded = encodeSession(session);
    const longUrl = `${base}#${ENV.CANDIDATE_SESSION_HASH_KEY}=${encoded}`;
    linkInput.value = longUrl;
    info.innerHTML = `<span class="text-gray-600">✓ Questions ready. URL is ${longUrl.length} chars. Trying to shorten...</span>`;

    shortenUrl(longUrl).then(shortUrl => {
      if (shortUrl) {
        linkInput.value = shortUrl;
        info.innerHTML = `<span class="text-green-800">✓ Short URL ready (${shortUrl.length} chars). Use the share buttons below to send it.</span>`;
      } else {
        info.innerHTML = `<span class="text-green-800">Using full URL (${longUrl.length} chars) — works perfectly in emails and chat apps. Use the share buttons below to skip writing the message yourself.</span>`;
      }
    });
  } catch (e) {
    info.innerHTML = `<span class="text-red-700">✗ Failed to prepare session: ${e.message}</span>`;
    linkInput.value = '';
  } finally {
    if (btn) btn.disabled = false;
  }

  const hasEmbeddedKey = window.EMBEDDED_KEY && window.EMBEDDED_KEY !== ENV.EMBEDDED_KEY_PLACEHOLDER;
  const warning = document.getElementById('linkConfigWarning');
  warning.classList.toggle('hidden', hasEmbeddedKey);
}

export function copyLink(event) {
  const inp = document.getElementById('generatedLink');
  inp.select();
  navigator.clipboard.writeText(inp.value).then(() => {
    const btn = event?.currentTarget;
    if (!btn) return;
    const orig = btn.textContent;
    btn.textContent = '✓ Copied!';
    setTimeout(() => btn.textContent = orig, 2000);
  });
}
