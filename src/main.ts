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

// Import d'Alpine.js pour la réactivité de l'interface utilisateur
// Import Alpine.js for user interface reactivity
import Alpine from 'alpinejs';

// Import du store et des interfaces depuis store.ts
// Import store and interfaces from store.ts
import { UIStore, ColorStore } from './store';

// =============================================================================
// CONFIGURATION DU STORE ALPINE.JS
// ALPINE.JS STORE CONFIGURATION
// =============================================================================

// Enregistre le store dans Alpine.js avec le nom 'uiStore'
// Register the store in Alpine.js with the name 'uiStore'
Alpine.store('uiStore', UIStore);

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
  // Étape 1 : Récupération de l'état initial du store Tauri au chargement de la page
  // Step 1: Fetch initial Tauri store state on page load
  try {
    // Appelle la commande get_store pour obtenir l'état actuel du backend
    // Call get_store command to get current backend state
    const initialStore = await invoke<ColorStore>('get_store');

    // Récupère la référence au store Alpine.js
    // Get reference to Alpine.js store
    const alpineStore = Alpine.store('uiStore') as UIStore;

    // Synchronise le store Alpine avec l'état initial de Tauri
    // Synchronize Alpine store with Tauri's initial state
    alpineStore.updateFromTauriStore(initialStore);
  } catch (error) {
    // Affiche l'erreur si le chargement initial échoue
    // Display error if initial load fails
    console.error('Error loading initial store:', error);
  }

  // Étape 2 : Écoute en continu des mises à jour du store Tauri
  // Step 2: Continuously listen for Tauri store updates
  await listen<ColorStore>('store-updated', (event) => {
    // Récupère la référence au store Alpine.js
    // Get reference to Alpine.js store
    const alpineStore = Alpine.store('uiStore') as UIStore;

    // Synchronise le store Alpine avec le nouveau payload reçu de Tauri
    // Ceci rend l'interface réactive aux changements du backend
    // Synchronize Alpine store with new payload received from Tauri
    // This makes the interface reactive to backend changes
    alpineStore.updateFromTauriStore(event.payload);
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
    const alpineStore = Alpine.store('uiStore') as UIStore;

    // Met à jour le profil ICC dans le store Alpine
    // Update ICC profile in Alpine store
    alpineStore.currentICCProfile = profileName;
  });

  // Étape 4 : Récupère le profil ICC initial
  // Step 4: Get initial ICC profile
  try {
    // Appelle la commande pour obtenir le profil ICC actuellement sélectionné
    // Call command to get currently selected ICC profile
    const currentProfile = await invoke<string | null>('get_selected_icc_profile');

    // Récupère la référence au store Alpine.js
    // Get reference to Alpine.js store
    const alpineStore = Alpine.store('uiStore') as UIStore;

    // Met à jour le profil ICC dans le store (ou 'Auto' par défaut)
    // Update ICC profile in store (or 'Auto' as default)
    alpineStore.currentICCProfile = currentProfile || 'Auto';
  } catch (error) {
    // Affiche l'erreur si la récupération échoue
    // Display error if retrieval fails
    console.error('Error loading ICC profile:', error);
  }
})();