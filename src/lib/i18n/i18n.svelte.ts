import { browser } from '$app/environment';
import { updateTrayLabels } from '$lib/api/tray';
import type { TrayLabels } from '$lib/types';

type Language = string;
type Theme = 'light' | 'dark' | 'system';
type TranslationTree = {
  [key: string]: string | TranslationTree;
};

const fallbackLanguage = 'en';
const localeModules = import.meta.glob<TranslationTree>('./locales/*.json', {
  eager: true,
  import: 'default',
});
const translations = Object.fromEntries(
  Object.entries(localeModules).map(([path, translation]) => {
    const language = path.match(/\/([^/]+)\.json$/)?.[1];
    if (!language) {
      throw new Error(`Could not resolve language code from locale path: ${path}`);
    }

    return [language, translation];
  }),
) as Record<Language, TranslationTree>;
const fallbackTranslations = translations[fallbackLanguage];

if (!fallbackTranslations) {
  throw new Error(`Missing fallback locale: ${fallbackLanguage}`);
}

function isLanguage(value: unknown): value is Language {
  return typeof value === 'string' && value in translations;
}

function isTheme(value: unknown): value is Theme {
  return value === 'light' || value === 'dark' || value === 'system';
}

function resolveTranslation(dict: TranslationTree, keys: string[]): string | undefined {
  let current: string | TranslationTree | undefined = dict;

  for (const key of keys) {
    if (typeof current !== 'object') {
      return undefined;
    }

    current = current[key];
  }

  return typeof current === 'string' ? current : undefined;
}

const languageOptions = Object.entries(translations)
  .map(([code, translation]) => {
    const label = resolveTranslation(translation, ['meta', 'languageName']);
    if (!label) {
      throw new Error(`Missing language name metadata for locale: ${code}`);
    }

    return { code, label };
  })
  .sort((a, b) => a.code.localeCompare(b.code));
const languageNames = new Map(languageOptions.map(({ code, label }) => [code, label] as const));

class AppState {
  readonly languageOptions = languageOptions;

  currentLang = $state<Language>(fallbackLanguage);
  currentTheme = $state<Theme>('system');

  initialized = false;

  constructor() {
    if (browser) {
      const savedLang = localStorage.getItem('shelflife_lang');
      if (isLanguage(savedLang)) {
        this.currentLang = savedLang;
      }

      const savedTheme = localStorage.getItem('shelflife_theme');
      if (isTheme(savedTheme)) {
        this.currentTheme = savedTheme;
      }

      // Sync theme and language choices across windows
      window.addEventListener('storage', (e) => {
        if (e.key === 'shelflife_theme') {
          if (isTheme(e.newValue)) {
            this.currentTheme = e.newValue;
          }
        }
        if (e.key === 'shelflife_lang') {
          if (isLanguage(e.newValue)) {
            this.currentLang = e.newValue;
          }
        }
      });
    }
  }

  init() {
    if (this.initialized || !browser) return;
    this.initialized = true;

    // Sync HTML lang attribute
    $effect(() => {
      document.documentElement.setAttribute('lang', this.currentLang);
    });

    $effect(() => {
      void this.syncTrayLabels(this.currentLang);
    });

    // Handle Theme side effects
    $effect(() => {
      const root = document.documentElement;
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');

      const applyTheme = () => {
        const isDark =
          this.currentTheme === 'dark' || (this.currentTheme === 'system' && mediaQuery.matches);

        root.classList.toggle('dark', isDark);
      };

      applyTheme();

      if (this.currentTheme === 'system') {
        mediaQuery.addEventListener('change', applyTheme);
        return () => mediaQuery.removeEventListener('change', applyTheme);
      }
    });
  }

  setLang(lang: string | undefined) {
    if (!isLanguage(lang)) return;

    this.currentLang = lang;
    if (browser) {
      localStorage.setItem('shelflife_lang', lang);
    }
  }

  languageName(language: string): string {
    return languageNames.get(language) ?? language;
  }

  setTheme(theme: Theme) {
    this.currentTheme = theme;
    if (browser) {
      localStorage.setItem('shelflife_theme', theme);
    }
  }

  private trayLabels(): TrayLabels {
    return {
      open: this.t('tray.open'),
      review: this.t('tray.review'),
      dashboard: this.t('tray.dashboard'),
      pause: this.t('tray.pause'),
      resume: this.t('tray.resume'),
      reconcile: this.t('tray.reconcile'),
      preferences: this.t('tray.preferences'),
      quit: this.t('tray.quit'),
      tooltip: this.t('tray.tooltip'),
      tooltip_paused: this.t('tray.tooltipPaused'),
    };
  }

  private async syncTrayLabels(language: Language) {
    if (!language) return;

    try {
      await updateTrayLabels(this.trayLabels());
    } catch {
      // The tray is only available in the Tauri desktop runtime.
    }
  }

  t(key: string, replacements?: Record<string, string | number>): string {
    const dict = translations[this.currentLang] ?? fallbackTranslations;
    const keys = key.split('.');

    let text = resolveTranslation(dict, keys) ?? resolveTranslation(fallbackTranslations, keys);

    if (text === undefined) {
      return key;
    }

    for (const [k, v] of Object.entries(replacements ?? {})) {
      text = text.replaceAll(`{${k}}`, String(v));
    }
    return text;
  }
}

export const i18n = new AppState();
