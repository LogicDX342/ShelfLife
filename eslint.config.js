import js from '@eslint/js';
import ts from 'typescript-eslint';
import svelte from 'eslint-plugin-svelte';
import prettier from 'eslint-config-prettier';
import globals from 'globals';

export default ts.config(
  // 1. Global ignores
  {
    ignores: ['build/', '.svelte-kit/', 'dist/', 'node_modules/', 'src-tauri/'],
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
    rules: {
      'svelte/no-navigation-without-resolve': 'off',
    },
  },
);
