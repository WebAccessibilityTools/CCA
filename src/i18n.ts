// =============================================================================
// i18n.ts - Module d'internationalisation
// i18n.ts - Internationalization module
// =============================================================================

import en from './locales/en.json';
import fr from './locales/fr.json';

// Traductions disponibles / Available translations
const translations: Record<string, Record<string, unknown>> = { en, fr };

// Langues supportées / Supported languages
const SUPPORTED_LOCALES = ['en', 'fr'];
const DEFAULT_LOCALE = 'en';
const STORAGE_KEY = 'cca-locale';
const PREFERENCE_KEY = 'cca-locale-preference';

// Type de préférence / Preference type
export type LocalePreference = 'auto' | 'en' | 'fr';

// Locale courante / Current locale
let currentLocale = DEFAULT_LOCALE;

// Callbacks de changement / Change callbacks
type LocaleChangeCallback = (locale: string) => void;
const callbacks: LocaleChangeCallback[] = [];

// =============================================================================
// FONCTIONS UTILITAIRES
// UTILITY FUNCTIONS
// =============================================================================

/// Normalise un tag BCP-47 vers une locale supportée
/// Normalizes a BCP-47 tag to a supported locale
function normalizeLocale(tag: string): string {
  // Extrait le code de langue (avant le premier tiret)
  // Extract language code (before first dash)
  const lang = tag.split('-')[0].toLowerCase();
  return SUPPORTED_LOCALES.includes(lang) ? lang : DEFAULT_LOCALE;
}

/// Résout une clé imbriquée dans un objet (ex: "app.title")
/// Resolves a nested key in an object (e.g., "app.title")
function resolve(obj: Record<string, unknown>, key: string): string | undefined {
  const parts = key.split('.');
  let current: unknown = obj;
  for (const part of parts) {
    if (current === null || current === undefined || typeof current !== 'object') {
      return undefined;
    }
    current = (current as Record<string, unknown>)[part];
  }
  return typeof current === 'string' ? current : undefined;
}

// =============================================================================
// API PUBLIQUE
// PUBLIC API
// =============================================================================

/// Retourne la traduction pour une clé, avec fallback EN puis clé brute
/// Returns the translation for a key, with EN fallback then raw key
export function t(key: string): string {
  return resolve(translations[currentLocale], key)
    ?? resolve(translations[DEFAULT_LOCALE], key)
    ?? key;
}

/// Retourne la locale courante
/// Returns the current locale
export function getLocale(): string {
  return currentLocale;
}

/// Retourne la préférence de locale stockée ('auto', 'en', ou 'fr')
/// Returns the stored locale preference ('auto', 'en', or 'fr')
export function getLocalePreference(): LocalePreference {
  const stored = localStorage.getItem(PREFERENCE_KEY);
  if (stored === 'auto' || stored === 'en' || stored === 'fr') {
    return stored;
  }
  // Si pas de préférence mais une locale explicite stockée, c'est un choix explicite
  // If no preference but an explicit locale stored, it's an explicit choice
  const legacyLocale = localStorage.getItem(STORAGE_KEY);
  if (legacyLocale && SUPPORTED_LOCALES.includes(legacyLocale)) {
    return legacyLocale as LocalePreference;
  }
  return 'auto';
}

/// Change la préférence de locale et applique la locale effective
/// Changes the locale preference and applies the effective locale
///
/// @param pref - 'auto', 'en', ou 'fr'
/// @param systemLocale - locale système (nécessaire si pref === 'auto')
export function setLocalePreference(pref: LocalePreference, systemLocale?: string): void {
  localStorage.setItem(PREFERENCE_KEY, pref);

  // Résout la locale effective / Resolve effective locale
  const effective = pref === 'auto'
    ? (systemLocale ? normalizeLocale(systemLocale) : DEFAULT_LOCALE)
    : pref;

  // Applique si changement / Apply if changed
  if (effective !== currentLocale) {
    currentLocale = effective;
    localStorage.setItem(STORAGE_KEY, effective);
    for (const cb of callbacks) {
      cb(effective);
    }
  }
}

/// Change la locale et persiste dans localStorage
/// Changes the locale and persists in localStorage
export function setLocale(locale: string): void {
  const normalized = normalizeLocale(locale);
  if (normalized === currentLocale) return;

  currentLocale = normalized;
  localStorage.setItem(STORAGE_KEY, normalized);

  // Met aussi à jour la préférence vers la valeur explicite
  // Also update preference to explicit value
  localStorage.setItem(PREFERENCE_KEY, normalized);

  // Notifie les callbacks
  // Notify callbacks
  for (const cb of callbacks) {
    cb(normalized);
  }
}

/// Initialise la locale : préférence > localStorage > système > EN
/// Initializes the locale: preference > localStorage > system > EN
export function initLocale(systemLocale?: string): string {
  // Vérifie d'abord la préférence / Check preference first
  const pref = localStorage.getItem(PREFERENCE_KEY);
  if (pref === 'auto') {
    currentLocale = systemLocale ? normalizeLocale(systemLocale) : DEFAULT_LOCALE;
  } else if (pref && SUPPORTED_LOCALES.includes(pref)) {
    currentLocale = pref;
  } else {
    // Fallback legacy : localStorage > système > EN
    // Legacy fallback: localStorage > system > EN
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored && SUPPORTED_LOCALES.includes(stored)) {
      currentLocale = stored;
    } else if (systemLocale) {
      currentLocale = normalizeLocale(systemLocale);
    } else {
      currentLocale = DEFAULT_LOCALE;
    }
  }
  return currentLocale;
}

/// Enregistre un callback appelé à chaque changement de locale
/// Registers a callback called on each locale change
export function onLocaleChange(callback: LocaleChangeCallback): void {
  callbacks.push(callback);
}
