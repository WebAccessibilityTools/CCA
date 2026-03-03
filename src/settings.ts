// =============================================================================
// settings.ts - Point d'entrée de la fenêtre Settings
// settings.ts - Settings window entry point
// =============================================================================

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Alpine from 'alpinejs';
import { locale as getSystemLocale } from '@tauri-apps/plugin-os';
import {
  initLocale,
  getLocale,
  onLocaleChange,
  setLocale,
  setLocalePreference,
  getLocalePreference,
  t as i18nT,
  type LocalePreference,
} from './i18n';

// =============================================================================
// DÉTECTION LOCALE SYSTÈME
// SYSTEM LOCALE DETECTION
// =============================================================================

let systemLocale: string | undefined;

// =============================================================================
// STORE ALPINE POUR SETTINGS
// ALPINE STORE FOR SETTINGS
// =============================================================================

Alpine.store('settings', {
  // Préférence actuelle / Current preference
  preference: 'auto' as LocalePreference,

  // Locale résolue pour réactivité Alpine / Resolved locale for Alpine reactivity
  locale: 'en',

  // Traduction réactive / Reactive translation
  t(key: string): string {
    void (this as any).locale;
    return i18nT(key);
  },

  // Applique un changement de préférence / Apply a preference change
  apply(pref: LocalePreference): void {
    (this as any).preference = pref;
    setLocalePreference(pref, systemLocale);
    // Synchronise le backend Rust / Sync Rust backend
    invoke('set_locale', { locale: getLocale() }).catch((err: unknown) => {
      console.error('Error setting locale in backend:', err);
    });
  },
});

// =============================================================================
// SYNCHRONISATION
// SYNCHRONIZATION
// =============================================================================

// Quand la locale change (via setLocalePreference ou setLocale), met à jour le store Alpine
// When locale changes (via setLocalePreference or setLocale), update Alpine store
onLocaleChange((locale) => {
  const store = Alpine.store('settings') as any;
  store.locale = locale;
  store.preference = getLocalePreference();
});

// =============================================================================
// INITIALISATION
// INITIALIZATION
// =============================================================================

Alpine.start();

(async () => {
  // Détecte la locale système / Detect system locale
  try {
    systemLocale = (await getSystemLocale()) ?? undefined;
  } catch (error) {
    console.error('Error detecting system locale:', error);
  }

  // Initialise i18n / Initialize i18n
  const detectedLocale = initLocale(systemLocale);

  // Synchronise le store Alpine / Sync Alpine store
  const store = Alpine.store('settings') as any;
  store.locale = detectedLocale;
  store.preference = getLocalePreference();

  // Écoute les changements de locale depuis le menu natif ou d'autres fenêtres
  // Listen for locale changes from native menu or other windows
  await listen<string>('locale-changed', (event) => {
    setLocale(event.payload);
    // Relit la préférence car elle a pu être mise à jour par l'autre fenêtre
    // Re-read preference as it may have been updated by the other window
    const s = Alpine.store('settings') as any;
    s.preference = getLocalePreference();
  });
})();
