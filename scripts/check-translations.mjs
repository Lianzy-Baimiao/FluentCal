// Catalog coverage and placeholder integrity for Rust, Slint and the web viewer.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
const read = file => fs.readFileSync(file, 'utf8');
const en = JSON.parse(read('locales/en-US.json'));
const tw = JSON.parse(read('locales/zh-TW.json'));
assert.deepEqual(Object.keys(en).sort(), Object.keys(tw).sort(), 'Catalogs must contain the same keys');
const fields = value => [...value.matchAll(/\{(\d+)\}/g)].map(m => m[1]).sort();
for (const [language, catalog] of [['en-US', en], ['zh-TW', tw]]) {
  for (const [source, text] of Object.entries(catalog)) {
    assert.ok(text.trim(), `${language}: empty translation for ${source}`);
    assert.deepEqual(fields(text), fields(source), `${language}: placeholders differ for ${source}`);
    if (language === 'en-US') assert.ok(!/[\u3400-\u9fff]/.test(text), `Untranslated English: ${source}`);
  }
}
function* files(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const file = path.join(dir, entry.name);
    if (entry.isDirectory()) yield* files(file);
    else if (/\.(rs|slint|ts|vue)$/.test(file) && !/\.test\.ts$/.test(file)) yield file;
  }
}
let uses = 0;
for (const file of ['native/app/ui', 'native/app/src', 'native/core/src', 'src'].flatMap(dir => [...files(dir)])) {
  if (file.endsWith('i18n.rs') || file.endsWith('i18n.ts')) continue;
  const content = read(file);
  for (const match of content.matchAll(/(?:@tr|\btrf?|\bt)\(\s*("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')/g)) {
    const raw = match[1];
    const source = raw[0] === '"' ? JSON.parse(raw) : raw.slice(1, -1).replaceAll("\\'", "'").replaceAll('\\n', '\n');
    assert.ok(Object.hasOwn(en, source), `${file}: missing translation for ${source}`);
    uses++;
  }
}
console.log(`${Object.keys(en).length} messages per language; ${uses} source uses checked.`);
