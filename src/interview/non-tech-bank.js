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
  // If headers were used but the leading "_default" bucket has stray content, drop it
  if (sawHeader && categories._default && categories._default.length === 0) {
    delete categories._default;
  }
  return categories;
}

export function pickNonTechQuestions(text, fallbackCount) {
  const cats = parseNonTechBank(text);
  const catNames = Object.keys(cats);

  // Legacy mode — no headers, just a flat list
  if (catNames.length === 1 && catNames[0] === '_default') {
    const all = cats._default;
    const shuffled = [...all].sort(() => Math.random() - 0.5);
    return shuffled.slice(0, Math.min(fallbackCount, all.length));
  }

  // Categorized mode — pick 1 random question per non-empty category
  const picked = [];
  for (const cat of catNames) {
    const qs = cats[cat];
    if (qs.length > 0) {
      const random = qs[Math.floor(Math.random() * qs.length)];
      picked.push(random);
    }
  }
  return picked;
}

// =============== PDF TEXT EXTRACTION ===============
