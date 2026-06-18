import js from '@eslint/js';
import prettier from 'eslint-config-prettier';
import simpleImportSort from 'eslint-plugin-simple-import-sort';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import ts from 'typescript-eslint';

export default ts.config(
  // 1. Global ignores
  {
    ignores: ['build/', '.svelte-kit/', 'dist/', 'node_modules/', 'src-tauri/', '.agents/'],
  },

  // 2. Base recommended configs
  js.configs.recommended,
  ...ts.configs.recommended,
  ...svelte.configs['flat/recommended'],
  prettier,

  // 3. Language options (Globals)
  {
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
  },

  // 4. TypeScript & Svelte parser configuration
  {
    files: ['**/*.ts', '**/*.tsx', '**/*.svelte.ts'],
    languageOptions: {
      parser: ts.parser,
    },
  },
  {
    files: ['**/*.svelte'],
    languageOptions: {
      parserOptions: {
        parser: ts.parser,
      },
    },
  },

  // 5. Custom rules
  {
    plugins: {
      'simple-import-sort': simpleImportSort,
    },
    rules: {
      'svelte/no-navigation-without-resolve': 'off',
      'simple-import-sort/imports': 'error',
      'simple-import-sort/exports': 'error',
    },
  },
  {
    files: ['src/lib/components/ui/**/*'],
    rules: {
      'simple-import-sort/imports': 'off',
      'simple-import-sort/exports': 'off',
    },
  },
);
