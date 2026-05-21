import { useState } from 'react';
import { ENV, envNumber } from '../app/env.js';
import { extractTextFromPDF } from '../services/pdf-parser.service.js';

const TONE_CLASS = {
  red: 'text-red-600',
  gray: 'text-gray-600',
  amber: 'text-amber-700',
  green: 'text-green-700',
};

/// A JD/resume field: a textarea plus an "Upload PDF" control that extracts
/// text into the textarea. The text stays editable after extraction.
export function PdfTextarea({ label, value, onChange, rows = 6, placeholder, className = '' }) {
  const [status, setStatus] = useState(null);

  async function handleFile(event) {
    const input = event.currentTarget;
    const file = input.files[0];
    if (!file) return;
    if (file.type !== ENV.PDF_ACCEPTED_MIME_TYPE) {
      setStatus({ tone: 'red', text: '✗ Please select a PDF file.' });
      return;
    }
    setStatus({ tone: 'gray', text: `⏳ Reading ${file.name}...` });
    try {
      const text = await extractTextFromPDF(file);
      if (!text || text.length < envNumber('PDF_MIN_EXTRACTED_TEXT_LENGTH')) {
        setStatus({ tone: 'amber', text: '⚠️ Could not extract much text. If this is a scanned PDF (image), please paste the text manually.' });
        return;
      }
      onChange(text);
      setStatus({ tone: 'green', text: `✓ Loaded ${file.name} — ${text.length} characters extracted. Review the text above and edit if needed.` });
    } catch (e) {
      console.error(e);
      setStatus({ tone: 'red', text: `✗ Failed to read PDF: ${e.message}` });
    } finally {
      input.value = '';
    }
  }

  return (
    <div className={className}>
      <div className="flex items-center justify-between mb-2">
        <label className="block text-sm font-semibold">{label}</label>
        <label className="text-xs text-indigo-600 cursor-pointer hover:underline">
          📄 Upload PDF
          <input type="file" accept={ENV.PDF_ACCEPTED_MIME_TYPE} className="hidden" onChange={handleFile} />
        </label>
      </div>
      <textarea
        className="textarea"
        rows={rows}
        placeholder={placeholder}
        value={value}
        onChange={e => onChange(e.target.value)}
      />
      <div className="text-xs text-gray-500 mt-1">
        {status && <span className={TONE_CLASS[status.tone]}>{status.text}</span>}
      </div>
    </div>
  );
}
