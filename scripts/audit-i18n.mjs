import { readdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const root = process.cwd();
const srcDir = path.join(root, 'src');
const localeDir = path.join(srcDir, 'lib', 'i18n', 'locales');
const i18nPath = path.join(srcDir, 'lib', 'i18n', 'i18n.svelte.ts');
const sourceExtensions = new Set(['.svelte', '.ts']);

async function listSourceFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const fullPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await listSourceFiles(fullPath)));
      continue;
    }

    if (sourceExtensions.has(path.extname(entry.name)) && fullPath !== i18nPath) {
      files.push(fullPath);
    }
  }

  return files;
}

function collectUsage(source) {
  const literalKeys = new Set();
  const dynamicPrefixes = new Set();

  for (const match of source.matchAll(/\bi18n\.t\(\s*(['"])([^'"$]+?)\1/g)) {
    literalKeys.add(match[2]);
  }

  for (const match of source.matchAll(/\bi18n\.t\(\s*`([^`$]*?)\$\{/g)) {
    if (match[1]) dynamicPrefixes.add(match[1]);
  }

  for (const match of source.matchAll(/\blabelKey\s*:\s*(['"])([^'"$]+?)\1/g)) {
    literalKeys.add(match[2]);
  }

  return { literalKeys, dynamicPrefixes };
}

function sorted(values) {
  return [...values].sort((a, b) => a.localeCompare(b));
}

function printList(title, values) {
  console.log(`${title}:`);
  if (values.length === 0) {
    console.log('  none');
    return;
  }

  for (const value of values) {
    console.log(`  ${value}`);
  }
}

function flattenKeys(obj, prefix = '') {
  let keys = {};
  for (const [key, value] of Object.entries(obj)) {
    const fullKey = prefix ? `${prefix}.${key}` : key;
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      Object.assign(keys, flattenKeys(value, fullKey));
    } else {
      keys[fullKey] = value;
    }
  }
  return keys;
}

function deleteNestedKey(obj, pathStr) {
  const parts = pathStr.split('.');
  let current = obj;
  for (let i = 0; i < parts.length - 1; i++) {
    current = current[parts[i]];
    if (!current) return;
  }
  delete current[parts[parts.length - 1]];

  // Clean up empty parent objects recursively
  for (let i = parts.length - 2; i >= 0; i--) {
    let parent = obj;
    for (let j = 0; j < i; j++) {
      parent = parent[parts[j]];
    }
    const currentKey = parts[i];
    if (parent && parent[currentKey] && Object.keys(parent[currentKey]).length === 0) {
      delete parent[currentKey];
    }
  }
}

async function loadLocales(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const locales = {};

  for (const entry of entries) {
    if (!entry.isFile() || path.extname(entry.name) !== '.json') continue;

    const language = path.basename(entry.name, '.json');
    const filePath = path.join(directory, entry.name);
    locales[language] = {
      path: filePath,
      json: JSON.parse(await readFile(filePath, 'utf8')),
    };
  }

  if (Object.keys(locales).length === 0) {
    throw new Error(`No locale JSON files found in ${directory}`);
  }

  return locales;
}

const locales = await loadLocales(localeDir);
const languageKeys = Object.fromEntries(
  Object.entries(locales).map(([language, locale]) => [
    language,
    new Set(Object.keys(flattenKeys(locale.json))),
  ]),
);
const allTranslationKeys = new Set(Object.values(languageKeys).flatMap((keys) => [...keys]));

const sourceFiles = await listSourceFiles(srcDir);
const usage = { literalKeys: new Set(), dynamicPrefixes: new Set() };
usage.dynamicPrefixes.add('error.');
usage.dynamicPrefixes.add('tab.');
usage.dynamicPrefixes.add('tray.');

for (const file of sourceFiles) {
  const source = await readFile(file, 'utf8');
  const fileUsage = collectUsage(source);
  for (const key of fileUsage.literalKeys) usage.literalKeys.add(key);
  for (const prefix of fileUsage.dynamicPrefixes) usage.dynamicPrefixes.add(prefix);
}

const protectedByDynamicUsage = (key) =>
  sorted(usage.dynamicPrefixes).some((prefix) => key.startsWith(prefix));
const unused = sorted(allTranslationKeys).filter(
  (key) => !usage.literalKeys.has(key) && !protectedByDynamicUsage(key),
);
const missing = sorted(usage.literalKeys).filter((key) => !allTranslationKeys.has(key));
const missingByLanguage = Object.fromEntries(
  Object.entries(languageKeys).map(([language, keys]) => [
    language,
    sorted(allTranslationKeys).filter((key) => !keys.has(key)),
  ]),
);

printList('Unused translation keys', unused);
console.log('');
printList('Missing used translation keys', missing);
console.log('');
for (const [language, missingKeys] of Object.entries(missingByLanguage)) {
  printList(`Keys missing from ${language}`, missingKeys);
  console.log('');
}
printList('Dynamic prefixes protected from unused checks', sorted(usage.dynamicPrefixes));

if (unused.length > 0) {
  console.log('Delete unused keys?(y/n)');
  const answer = await new Promise((resolve) => {
    const listener = (data) => {
      const value = data.toString().trim().toLowerCase();
      if (value === 'y' || value === 'n') {
        process.stdin.removeListener('data', listener);
        process.stdin.pause();
        resolve(value);
      }
    };
    process.stdin.on('data', listener);
  });

  if (answer === 'y') {
    for (const locale of Object.values(locales)) {
      const updated = JSON.parse(JSON.stringify(locale.json));
      for (const key of unused) {
        deleteNestedKey(updated, key);
      }
      await writeFile(locale.path, JSON.stringify(updated, null, 2) + '\n', 'utf8');
    }
    console.log('Unused keys deleted');
  } else {
    process.exitCode = 1;
  }
}

if (missing.length > 0 || Object.values(missingByLanguage).some((keys) => keys.length > 0)) {
  process.exitCode = 1;
}
