// =============================================================================
// main.ts - Point d'entrée de l'application frontend
// main.ts - Frontend application entry point
// =============================================================================

// Import de la fonction invoke pour appeler les commandes Tauri depuis le frontend
// Import invoke function to call Tauri commands from the frontend
import { invoke } from "@tauri-apps/api/core";

// Import de la fonction listen pour écouter les événements émis par Tauri
// Import listen function to listen to events emitted by Tauri
import { listen } from "@tauri-apps/api/event";

// Import de la fenêtre courante pour le redimensionnement
// Import current window for resizing
import { getCurrentWindow, PhysicalSize } from "@tauri-apps/api/window";

// Import de la détection du type d'OS pour le padding titlebar macOS
// Import OS type detection for macOS titlebar padding
import { type as osType } from '@tauri-apps/plugin-os';

// Import d'Alpine.js pour la réactivité de l'interface utilisateur
// Import Alpine.js for user interface reactivity
import Alpine from 'alpinejs';

// Import du store et des interfaces depuis store.ts
// Import store and interfaces from store.ts
import { UIStore, BackendStore } from './store';

// Import du module i18n
// Import i18n module
import { initLocale, onLocaleChange, setLocale } from './i18n';

// Import de la détection de locale système via Tauri plugin OS
// Import system locale detection via Tauri plugin OS
import { locale as getSystemLocale } from '@tauri-apps/plugin-os';

// Import Webcomponents
import './components/ProgressBar';

// =============================================================================
// REDIMENSIONNEMENT AUTOMATIQUE DE LA FENÊTRE
// AUTOMATIC WINDOW RESIZING
// See https://github.com/tauri-apps/tauri/issues/12420
// =============================================================================

// Ajuste la taille de la fenêtre Tauri au contenu via PhysicalSize
// Adjusts Tauri window size to match content using PhysicalSize
async function resizeWindow(container: HTMLElement) {
  const currentWindow = getCurrentWindow();
  const rect = container.getBoundingClientRect();
  const factor = window.devicePixelRatio;
  const currentSize = await currentWindow.outerSize();
  const width = currentSize.width;
  const height = Math.ceil(rect.height * factor);

  // Ajoute le padding de la titlebar macOS si la fenêtre est décorée
  // Add macOS titlebar padding if window is decorated
  const isDecorated = await currentWindow.isDecorated();
  const topPadding = isDecorated && osType() === 'macos' ? 55 : 0;

  await currentWindow.setSize(new PhysicalSize(width, height + topPadding));
}

// Initialise le ResizeObserver sur <main> après le chargement du DOM
// Initialize ResizeObserver on <main> after DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
  const container = document.querySelector('main');
  if (!container) return;

  const observer = new ResizeObserver(() => {
    resizeWindow(container);
  });
  observer.observe(container);
});

// =============================================================================
// RACCOURCI CLAVIER POUR COPIER LES RÉSULTATS
// KEYBOARD SHORTCUT TO COPY RESULTS
// =============================================================================

function eventToShortcut(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.metaKey) parts.push('Cmd');
  if (e.ctrlKey) parts.push('Ctrl');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');
  parts.push(e.key.length === 1 ? e.key.toUpperCase() : e.key);
  return parts.join('+');
}

function formatTemplate(template: string, store: UIStore): string {
  return template
    .replace(/%f\.hex%/g, store.foregroundHex)
    .replace(/%b\.hex%/g, store.backgroundHex)
    .replace(/%cr%/g, store.contrastRatio)
    .replace(/%crr%/g, store.contrastRatio)
    .replace(/%1\.4\.3%/g, store.level143Regular ? 'Pass' : 'Fail')
    .replace(/%1\.4\.6%/g, store.level146Regular ? 'Pass' : 'Fail')
    .replace(/%1\.4\.11%/g, store.level1411 ? 'Pass' : 'Fail');
}

let toastTimeout: ReturnType<typeof setTimeout>;
function showCopyToast(text: string) {
  const toast = document.getElementById('copy-toast');
  if (!toast) return;
  const duration = parseInt(localStorage.getItem('cca-toast-duration') ?? '3', 10);

  toast.textContent = text;

  // Mode manuel (duration === 0) : ajoute un bouton fermer
  // Manual mode (duration === 0): add a close button
  if (duration === 0) {
    const btn = document.createElement('button');
    btn.textContent = '\u00d7';
    btn.className = 'toast-close';
    btn.onclick = () => toast.classList.remove('visible');
    toast.appendChild(btn);
  }

  toast.classList.add('visible');
  clearTimeout(toastTimeout);
  if (duration > 0) {
    toastTimeout = setTimeout(() => toast.classList.remove('visible'), duration * 1000);
  }
}

interface CopyTemplate {
  template: string;
  shortcut: string;
}

function loadCopyTemplates(): CopyTemplate[] {
  try {
    const raw = localStorage.getItem('cca-copy-templates');
    if (raw) return JSON.parse(raw);
  } catch {}
  const defaultShortcut = navigator.platform.includes('Mac') ? 'Cmd+S' : 'Ctrl+S';
  return [{ template: '%f.hex%/%b.hex% = ratio de %cr%:1', shortcut: defaultShortcut }];
}

document.addEventListener('keydown', (e) => {
  const pressed = eventToShortcut(e);
  const templates = loadCopyTemplates();

  for (const tpl of templates) {
    if (tpl.shortcut && tpl.shortcut === pressed) {
      e.preventDefault();
      const store = Alpine.store('uiStore') as UIStore;
      const text = formatTemplate(tpl.template, store);
      navigator.clipboard.writeText(text);
      showCopyToast(text);
      return;
    }
  }
});

// =============================================================================
// CONFIGURATION DU STORE ALPINE.JS
// ALPINE.JS STORE CONFIGURATION
// =============================================================================

// Enregistre le store dans Alpine.js avec le nom 'uiStore'
// Register the store in Alpine.js with the name 'uiStore'
Alpine.store('uiStore', UIStore);

// =============================================================================
// SYNCHRONISATION i18n BIDIRECTIONNELLE
// BIDIRECTIONAL i18n SYNCHRONIZATION
// =============================================================================

// Quand la locale change côté frontend (setLocale), on synchronise Alpine et Rust
// When locale changes on frontend (setLocale), sync Alpine and Rust
onLocaleChange((locale) => {
  const alpineStore = Alpine.store('uiStore') as UIStore;
  alpineStore.locale = locale;

  // Notifie le backend Rust pour reconstruire les menus
  // Notify Rust backend to rebuild menus
  invoke('set_locale', { locale }).catch((err) => {
    console.error('Error setting locale in backend:', err);
  });
});

// =============================================================================
// INITIALISATION
// INITIALIZATION
// =============================================================================

// Initialise Alpine.js et active la réactivité dans le DOM
// Initialize Alpine.js and activate reactivity in the DOM
Alpine.start();

// Fonction immédiatement invoquée asynchrone (IIFE) pour la synchronisation avec Tauri
// Immediately Invoked Async Function Expression (IIFE) for Tauri synchronization
(async () => {
  // Étape 0 : Détection de la locale système et initialisation i18n
  // Step 0: Detect system locale and initialize i18n
  let detectedLocale = 'en';
  try {
    const systemLocale = await getSystemLocale();
    detectedLocale = initLocale(systemLocale ?? undefined);
  } catch (error) {
    console.error('Error detecting system locale:', error);
    detectedLocale = initLocale();
  }

  // Synchronise la locale dans le store Alpine
  // Sync locale into Alpine store
  const alpineStore = Alpine.store('uiStore') as UIStore;
  alpineStore.locale = detectedLocale;

  // Envoie la locale initiale au backend
  // Send initial locale to backend
  try {
    await invoke('set_locale', { locale: detectedLocale });
  } catch (error) {
    console.error('Error setting initial locale:', error);
  }

  // Étape 1 : Récupération de l'état initial du store Tauri au chargement de la page
  // Step 1: Fetch initial Tauri store state on page load
  try {
    // Appelle la commande get_store pour obtenir l'état actuel du backend
    // Call get_store command to get current backend state
    const initialStore = await invoke<BackendStore>('get_store');

    // Récupère la référence au store Alpine.js
    // Get reference to Alpine.js store
    const store = Alpine.store('uiStore') as UIStore;

    // Synchronise le store Alpine avec l'état initial de Tauri
    // Synchronize Alpine store with Tauri's initial state
    store.updateFromTauriStore(initialStore);
  } catch (error) {
    // Affiche l'erreur si le chargement initial échoue
    // Display error if initial load fails
    console.error('Error loading initial store:', error);
  }

  // Étape 2 : Écoute en continu des mises à jour du store Tauri
  // Step 2: Continuously listen for Tauri store updates
  await listen<BackendStore>('store-updated', (event) => {
    // Récupère la référence au store Alpine.js
    // Get reference to Alpine.js store
    const store = Alpine.store('uiStore') as UIStore;

    // Synchronise le store Alpine avec le nouveau payload reçu de Tauri
    // Ceci rend l'interface réactive aux changements du backend
    // Synchronize Alpine store with new payload received from Tauri
    // This makes the interface reactive to backend changes
    store.updateFromTauriStore(event.payload);
  });

  // Étape 3 : Écoute les changements de profil ICC depuis le menu
  // Step 3: Listen for ICC profile changes from the menu
  await listen<string>('icc-profile-changed', (event) => {
    // Récupère le nom du profil ICC sélectionné
    // Get the selected ICC profile name
    const profileName = event.payload;

    // Affiche le profil sélectionné dans la console (pour debug)
    // Display selected profile in console (for debug)
    console.log('ICC Profile changed to:', profileName);

    // Récupère la référence au store Alpine.js
    // Get reference to Alpine.js store
    const store = Alpine.store('uiStore') as UIStore;

    // Met à jour le profil ICC dans le store Alpine
    // Update ICC profile in Alpine store
    store.currentICCProfile = profileName;
  });

  // Étape 4 : Écoute les changements de locale depuis le menu natif Rust
  // Step 4: Listen for locale changes from native Rust menu
  await listen<string>('locale-changed', (event) => {
    const locale = event.payload;

    // Appelle setLocale qui mettra à jour le module i18n et déclenchera onLocaleChange
    // Calls setLocale which updates the i18n module and triggers onLocaleChange
    // Note: onLocaleChange invoquera invoke('set_locale') mais le backend est déjà à jour,
    // donc c'est un no-op côté Rust (la locale est déjà la bonne)
    // Note: onLocaleChange will invoke invoke('set_locale') but backend is already up to date,
    // so it's a no-op on Rust side (locale is already correct)
    setLocale(locale);
  });

  // Étape 5 : Écoute l'événement focus-main depuis la fenêtre settings
  // Step 5: Listen for focus-main event from settings window
  await listen('focus-main', () => {
    getCurrentWindow().setFocus();
  });

  // Étape 5b : Envoie les modèles de copie au backend pour le menu Édition
  // Step 5b: Send copy templates to backend for Edit menu
  try {
    const templates = loadCopyTemplates();
    await invoke('set_copy_templates', { templates });
  } catch (error) {
    console.error('Error sending templates to backend:', error);
  }

  // Étape 5c : Écoute les clics sur les modèles de copie depuis le menu natif
  // Step 5c: Listen for copy template clicks from native menu
  await listen<number>('copy-template', (event) => {
    const index = event.payload;
    const templates = loadCopyTemplates();
    if (index < templates.length) {
      const store = Alpine.store('uiStore') as UIStore;
      const text = formatTemplate(templates[index].template, store);
      navigator.clipboard.writeText(text);
      showCopyToast(text);
    }
  });

  // Étape 6 : Récupère le profil ICC initial
  // Step 6: Get initial ICC profile
  try {
    // Appelle la commande pour obtenir le profil ICC actuellement sélectionné
    // Call command to get currently selected ICC profile
    const currentProfile = await invoke<string | null>('get_selected_icc_profile');

    // Récupère la référence au store Alpine.js
    // Get reference to Alpine.js store
    const store = Alpine.store('uiStore') as UIStore;

    // Met à jour le profil ICC dans le store (ou 'Auto' par défaut)
    // Update ICC profile in store (or 'Auto' as default)
    store.currentICCProfile = currentProfile || 'Auto';
  } catch (error) {
    // Affiche l'erreur si la récupération échoue
    // Display error if retrieval fails
    console.error('Error loading ICC profile:', error);
  }
})();
