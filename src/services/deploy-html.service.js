import { ENV, envNumber } from '../app/env.js';

async function fetchText(url) {
  const resp = await fetch(url);
  if (!resp.ok) throw new Error(`Failed to fetch ${url}`);
  return resp.text();
}

async function buildStandaloneHtmlFromCurrentPage() {
  const pageUrl = new URL(window.location.href);
  pageUrl.hash = '';
  pageUrl.search = '';

  let html = await fetchText(pageUrl.href);

  if (html.includes('/@vite/client') || html.includes('./src/main.js')) {
    throw new Error('The deployable HTML generator must run from a built Vite page. Use npm run build && npm run preview, or use the deployed dist app.');
  }

  html = await replaceAsync(
    html,
    /<link rel="stylesheet"([^>]*?)href="([^"]+)"([^>]*)>/g,
    async (_match, beforeHref, href, afterHref) => {
      const css = await fetchText(new URL(href, pageUrl).href);
      return `<style${beforeHref}${afterHref}>\n${css}\n</style>`;
    }
  );

  html = await replaceAsync(
    html,
    /<script type="module"([^>]*?)src="([^"]+)"([^>]*)><\/script>/g,
    async (_match, beforeSrc, src, afterSrc) => {
      const js = await fetchText(new URL(src, pageUrl).href);
      return `<script type="module"${beforeSrc}${afterSrc}>\n${js}\n</script>`;
    }
  );

  return html;
}

async function replaceAsync(input, regex, replacer) {
  const matches = [...input.matchAll(regex)];
  const replacements = await Promise.all(matches.map(match => replacer(...match)));
  let index = 0;
  return input.replace(regex, () => replacements[index++]);
}

export async function downloadDeployableHTML() {
  // Determine which key to bake
  const savedConfig = JSON.parse(localStorage.getItem(ENV.LOCAL_STORAGE_CONFIG_KEY) || '{}');
  const keyToBake = (window.EMBEDDED_KEY && window.EMBEDDED_KEY !== ENV.EMBEDDED_KEY_PLACEHOLDER)
    ? window.EMBEDDED_KEY
    : savedConfig.apiKey;

  if (!keyToBake) {
    alert('No API key found. Please save one in HR Configuration first.');
    return;
  }

  // Also bake non-tech bank + config so candidates don't need a fresh setup
  const bakedConfig = {
    hrEmail: savedConfig.hrEmail || '',
    nonTechBank: savedConfig.nonTechBank || '',
    techCount: savedConfig.techCount || envNumber('DEFAULT_TECH_QUESTION_COUNT'),
    nonTechCount: savedConfig.nonTechCount || envNumber('DEFAULT_NON_TECH_QUESTION_COUNT'),
    passThreshold: savedConfig.passThreshold || envNumber('DEFAULT_PASS_THRESHOLD'),
    voiceName: savedConfig.voiceName || ''
  };

  let html;
  try {
    html = await buildStandaloneHtmlFromCurrentPage();
  } catch (e) {
    alert(e.message);
    return;
  }

  // Replace the placeholder key
  html = html.replace(ENV.EMBEDDED_KEY_PLACEHOLDER, keyToBake);

  // Inject baked config: replace the placeholder marker in init JS
  const configScript = `<script>window.EMBEDDED_CONFIG = ${JSON.stringify(bakedConfig)};</` + 'script>';
  html = html.replace(
    '<script>window.EMBEDDED_KEY',
    configScript + '\n<script>window.EMBEDDED_KEY'
  );

  // Download
  const blob = new Blob([html], { type: 'text/html' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = ENV.DEPLOYABLE_DOWNLOAD_FILENAME;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
