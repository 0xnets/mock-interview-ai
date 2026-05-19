/// Parse the HR-side textarea where behavioral questions are entered. Lines
/// starting with `## ` are topic headers; the lines beneath each header are
/// that topic's questions. The shape matches what `POST /v1/interviews`
/// stores in `interview_configs.behavioral_bank`: `{ topic: [questions...] }`.
export function parseNonTechBank(text) {
  const lines = (text || '').split('\n').map(l => l.trim());
  const categories = {};
  let currentCat = '_default';
  let sawHeader = false;
  for (const line of lines) {
    if (line.startsWith('## ')) {
      currentCat = line.slice(3).trim();
      sawHeader = true;
      if (!categories[currentCat]) categories[currentCat] = [];
    } else if (line) {
      if (!categories[currentCat]) categories[currentCat] = [];
      categories[currentCat].push(line);
    }
  }
  if (sawHeader && categories._default && categories._default.length === 0) {
    delete categories._default;
  }
  return categories;
}
