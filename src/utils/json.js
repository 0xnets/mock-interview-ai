export function extractJSON(text) {
  if (!text || typeof text !== 'string') throw new Error('Empty response');
  // Strip markdown code fences
  let cleaned = text.trim();
  cleaned = cleaned.replace(/^```(?:json)?\s*/i, '').replace(/\s*```$/, '');
  // Try direct parse first (works if response is clean JSON)
  try { return JSON.parse(cleaned); } catch(e) {}
  // Try to find a JSON object or array inside the text
  const objMatch = cleaned.match(/\{[\s\S]*\}/);
  if (objMatch) {
    try { return JSON.parse(objMatch[0]); } catch(e) {}
  }
  const arrMatch = cleaned.match(/\[[\s\S]*\]/);
  if (arrMatch) {
    try { return JSON.parse(arrMatch[0]); } catch(e) {}
  }
  throw new Error('Could not parse JSON. Raw response: ' + text.slice(0, 500));
}

// Normalize an "array of questions" response. Accepts:
//   ["q1", "q2", ...]
//   {"questions": ["q1", ...]}
//   {"results": ["q1", ...]}
//   any object whose first value is an array

export function normalizeQuestionArray(parsed) {
  if (Array.isArray(parsed)) return parsed.filter(q => typeof q === 'string');
  if (parsed && typeof parsed === 'object') {
    // Common wrapper keys
    for (const key of ['questions', 'results', 'items', 'data']) {
      if (Array.isArray(parsed[key])) return parsed[key].filter(q => typeof q === 'string');
    }
    // First array-typed value
    const firstArr = Object.values(parsed).find(v => Array.isArray(v));
    if (firstArr) return firstArr.filter(q => typeof q === 'string');
  }
  throw new Error('Expected an array of questions. Got: ' + JSON.stringify(parsed).slice(0, 300));
}

// =============== EMAIL REPORT TO HR (via Netlify Forms) ===============
