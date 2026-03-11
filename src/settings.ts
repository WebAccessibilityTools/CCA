// =============================================================================
// settings.ts - Point d'entrée de la fenêtre Settings
// settings.ts - Settings window entry point
// =============================================================================

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { emit } from "@tauri-apps/api/event";
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

interface CopyTemplate {
  name: string;
  template: string;
  shortcut: string;
}

interface AppShortcut {
  id: string;
  key: string;
}

const DEFAULT_SHORTCUTS: AppShortcut[] = [
  { id: 'pick_fg', key: 'F11' },
  { id: 'pick_bg', key: 'F12' },
];

function loadShortcuts(): AppShortcut[] {
  try {
    const raw = localStorage.getItem('cca-shortcuts');
    if (raw) return JSON.parse(raw);
  } catch {}
  return structuredClone(DEFAULT_SHORTCUTS);
}

const DEFAULT_SHORTCUT = navigator.platform.includes('Mac') ? 'Cmd+S' : 'Ctrl+S';

const DEFAULT_TEMPLATES: CopyTemplate[] = [
  { name: 'Short', template: '%f.hex%/%b.hex% = ratio de %cr%:1', shortcut: DEFAULT_SHORTCUT },
];

function loadTemplates(): CopyTemplate[] {
  try {
    const raw = localStorage.getItem('cca-copy-templates');
    if (raw) return JSON.parse(raw);
  } catch {}
  return structuredClone(DEFAULT_TEMPLATES);
}

function keyboardEventToShortcut(event: KeyboardEvent): string {
  const parts: string[] = [];
  if (event.metaKey) parts.push('Cmd');
  if (event.ctrlKey) parts.push('Ctrl');
  if (event.altKey) parts.push('Alt');
  if (event.shiftKey) parts.push('Shift');
  parts.push(event.key.length === 1 ? event.key.toUpperCase() : event.key);
  return parts.join('+');
}

Alpine.store('settings', {
  // Préférence actuelle / Current preference
  preference: 'auto' as LocalePreference,

  // Locale résolue pour réactivité Alpine / Resolved locale for Alpine reactivity
  locale: 'en',

  // Raccourcis clavier / Keyboard shortcuts
  shortcuts: loadShortcuts() as AppShortcut[],

  // Liste des modèles de copie / Copy templates list
  templates: loadTemplates() as CopyTemplate[],

  // Durée du toast en secondes (0 = manuel) / Toast duration in seconds (0 = manual)
  toastDuration: parseInt(localStorage.getItem('cca-toast-duration') ?? '3', 10),

  // Traduction réactive / Reactive translation
  t(key: string): string {
    void (this as any).locale;
    return i18nT(key);
  },

  // Applique un changement de préférence / Apply a preference change
  apply(pref: LocalePreference): void {
    (this as any).preference = pref;
    setLocalePreference(pref, systemLocale);
    invoke('set_locale', { locale: getLocale() }).catch((err: unknown) => {
      console.error('Error setting locale in backend:', err);
    });
  },

  // Met à jour un raccourci / Update a shortcut
  updateShortcut(index: number, event: KeyboardEvent): void {
    if (['Control', 'Alt', 'Shift', 'Meta'].includes(event.key)) return;
    (this as any).shortcuts[index].key = keyboardEventToShortcut(event);
  },

  // Ajoute un modèle / Add a template
  addTemplate(): void {
    (this as any).templates.push({ name: '', template: '', shortcut: '' });
  },

  // Supprime un modèle / Remove a template
  removeTemplate(index: number): void {
    (this as any).templates.splice(index, 1);
  },

  // Met à jour le raccourci d'un modèle / Update a template's shortcut
  updateTemplateShortcut(index: number, event: KeyboardEvent): void {
    if (['Control', 'Alt', 'Shift', 'Meta'].includes(event.key)) return;
    (this as any).templates[index].shortcut = keyboardEventToShortcut(event);
  },

  // Sauvegarde les préférences / Save preferences
  async save(): Promise<void> {
    // Filtre les modèles sans nom / Filter out templates without a name
    (this as any).templates = (this as any).templates.filter((t: CopyTemplate) => t.name.trim() !== '');
    localStorage.setItem('cca-copy-templates', JSON.stringify((this as any).templates));
    localStorage.setItem('cca-shortcuts', JSON.stringify((this as any).shortcuts));
    localStorage.setItem('cca-toast-duration', String((this as any).toastDuration));
    // Synchronise les modèles avec le backend pour le menu Édition
    // Sync templates with backend for Edit menu
    try {
      await invoke('set_copy_templates', { templates: (this as any).templates });
    } catch (error) {
      console.error('Error syncing templates to backend:', error);
    }
    await emit('focus-main');
    getCurrentWindow().close();
  },

  // Annule les modifications / Cancel changes
  async cancel(): Promise<void> {
    (this as any).templates = loadTemplates();
    (this as any).shortcuts = loadShortcuts();
    (this as any).toastDuration = parseInt(localStorage.getItem('cca-toast-duration') ?? '3', 10);
    await emit('focus-main');
    getCurrentWindow().close();
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
