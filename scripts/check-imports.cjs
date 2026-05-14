const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');
const srcDir = path.join(root, 'src');
const stylesDir = path.join(root, 'src', 'styles');
const missing = [];

function walk(dir, predicate, files = []) {
  if (!fs.existsSync(dir)) return files;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walk(fullPath, predicate, files);
    } else if (predicate(fullPath)) {
      files.push(fullPath);
    }
  }
  return files;
}

for (const filePath of walk(srcDir, file => file.endsWith('.js'))) {
  const source = fs.readFileSync(filePath, 'utf8');
  for (const match of source.matchAll(/from ['"]([^'"]+)['"]/g)) {
    const specifier = match[1];
    if (!specifier.startsWith('.')) continue;
    const target = path.resolve(path.dirname(filePath), specifier);
    if (!fs.existsSync(target)) {
      missing.push(`${path.relative(root, filePath)} -> ${specifier}`);
    }
  }
}

for (const filePath of walk(stylesDir, file => file.endsWith('.css'))) {
  const source = fs.readFileSync(filePath, 'utf8');
  for (const match of source.matchAll(/@import url\("([^"]+)"\)/g)) {
    const specifier = match[1];
    if (!specifier.startsWith('.')) continue;
    const target = path.resolve(path.dirname(filePath), specifier);
    if (!fs.existsSync(target)) {
      missing.push(`${path.relative(root, filePath)} -> ${specifier}`);
    }
  }
}

if (missing.length) {
  console.error('Missing relative imports:');
  for (const item of missing) console.error(`- ${item}`);
  process.exit(1);
}

console.log('All relative JS and CSS imports resolve.');
