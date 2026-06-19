import { readdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const root = process.cwd();
const srcDir = path.join(root, 'src');
const i18nPath = path.join(srcDir, 'lib', 'i18n', 'i18n.svelte.ts');
const sourceExtensions = new Set(['.svelte', '.ts']);

function escapeRegex(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function findMatchingBrace(source, openBraceIndex) {
  let depth = 0;
  let quote = null;
  let escaped = false;
  let lineComment = false;
  let blockComment = false;

  for (let index = openBraceIndex; index < source.length; index += 1) {
    const char = source[index];
    const next = source[index + 1];

    if (lineComment) {
      if (char === '\n') lineComment = false;
      continue;
    }

    if (blockComment) {
      if (char === '*' && next === '/') {
        blockComment = false;
        index += 1;
      }
      continue;
    }

    if (quote) {
      if (escaped) {
        escaped = false;
      } else if (char === '\\') {
        escaped = true;
      } else if (char === quote) {
        quote = null;
      }
      continue;
    }

    if (char === '/' && next === '/') {
      lineComment = true;
      index += 1;
      continue;
    }

    if (char === '/' && next === '*') {
      blockComment = true;
      index += 1;
      continue;
    }

    if (char === "'" || char === '"' || char === '`') {
      quote = char;
      continue;
    }

    if (char === '{') depth += 1;
    if (char === '}') {
      depth -= 1;
      if (depth === 0) return index;
    }
  }

  throw new Error(`Could not find matching brace at index ${openBraceIndex}`);
}

function extractLanguageKeys(source, language) {
  const languageMatch = new RegExp(`\\b${escapeRegex(language)}\\s*:\\s*\\{`).exec(source);
  if (!languageMatch) {
    throw new Error(`Could not find translation block for "${language}"`);
  }

  const openBraceIndex = source.indexOf('{', languageMatch.index);
  const closeBraceIndex = findMatchingBrace(source, openBraceIndex);
  const block = source.slice(openBraceIndex + 1, closeBraceIndex);
  const keys = new Set();

  for (const match of block.matchAll(/^\s*'([^']+)'\s*:/gm)) {
    keys.add(match[1]);
  }

  return keys;
}

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

const i18nSource = await readFile(i18nPath, 'utf8');
const languageKeys = {
  en: extractLanguageKeys(i18nSource, 'en'),
  zh: extractLanguageKeys(i18nSource, 'zh'),
};
const allTranslationKeys = new Set([...languageKeys.en, ...languageKeys.zh]);

const sourceFiles = await listSourceFiles(srcDir);
const usage = { literalKeys: new Set(), dynamicPrefixes: new Set() };

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
const missingInZh = sorted(languageKeys.en).filter((key) => !languageKeys.zh.has(key));
const missingInEn = sorted(languageKeys.zh).filter((key) => !languageKeys.en.has(key));

printList('Unused translation keys', unused);
console.log('');
printList('Missing used translation keys', missing);
console.log('');
printList('Keys missing from zh', missingInZh);
console.log('');
printList('Keys missing from en', missingInEn);
console.log('');
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
    let updatedSource = i18nSource;
    const unusedSet = new Set(unused);

    for (const lang of ['en', 'zh']) {
      const languageMatch = new RegExp(`\\b${escapeRegex(lang)}\\s*:\\s*\\{`).exec(updatedSource);
      if (!languageMatch) {
        throw new Error(`Could not find translation block for "${lang}"`);
      }

      const openBraceIndex = updatedSource.indexOf('{', languageMatch.index);
      const closeBraceIndex = findMatchingBrace(updatedSource, openBraceIndex);
      const block = updatedSource.slice(openBraceIndex + 1, closeBraceIndex);

      const lines = block.split('\n');
      const newLines = [];
      let skipping = false;
      const keyRegex = /^\s*'([^']+)'\s*:/;

      for (const line of lines) {
        const match = line.match(keyRegex);
        if (match) {
          const key = match[1];
          skipping = unusedSet.has(key);
        }
        if (!skipping) {
          newLines.push(line);
        }
      }

      const newBlock = newLines.join('\n');
      updatedSource =
        updatedSource.slice(0, openBraceIndex + 1) +
        newBlock +
        updatedSource.slice(closeBraceIndex);
    }

    await writeFile(i18nPath, updatedSource, 'utf8');
    console.log('Unused keys deleted');
  } else {
    process.exitCode = 1;
  }
}

if (missing.length > 0 || missingInZh.length > 0 || missingInEn.length > 0) {
  process.exitCode = 1;
}
