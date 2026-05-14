import { ENV, envNumber } from '../app/env.js';

export async function extractTextFromPDF(file) {
  if (!window.pdfjsLib) throw new Error('PDF library not loaded. Refresh and try again.');
  const buffer = await file.arrayBuffer();
  const pdf = await window.pdfjsLib.getDocument({ data: buffer }).promise;
  let fullText = '';
  for (let i = 1; i <= pdf.numPages; i++) {
    const page = await pdf.getPage(i);
    const content = await page.getTextContent();
    const pageText = content.items.map(it => it.str).join(' ');
    fullText += pageText + '\n\n';
  }
  return fullText.trim();
}

export async function loadPdfIntoTextarea(input, textareaId, statusId) {
  const file = input.files[0];
  const status = document.getElementById(statusId);
  if (!file) return;
  if (file.type !== ENV.PDF_ACCEPTED_MIME_TYPE) {
    status.innerHTML = '<span class="text-red-600">✗ Please select a PDF file.</span>';
    return;
  }
  status.innerHTML = '<span class="text-gray-600">⏳ Reading ' + file.name + '...</span>';
  try {
    const text = await extractTextFromPDF(file);
    if (!text || text.length < envNumber('PDF_MIN_EXTRACTED_TEXT_LENGTH')) {
      status.innerHTML = '<span class="text-amber-700">⚠️ Could not extract much text. If this is a scanned PDF (image), please paste the text manually.</span>';
      return;
    }
    document.getElementById(textareaId).value = text;
    status.innerHTML = '<span class="text-green-700">✓ Loaded ' + file.name + ' — ' + text.length + ' characters extracted. Review the text above and edit if needed.</span>';
  } catch (e) {
    console.error(e);
    status.innerHTML = '<span class="text-red-600">✗ Failed to read PDF: ' + e.message + '</span>';
  }
}

// =============== CANDIDATE SESSION ENCODING ===============
