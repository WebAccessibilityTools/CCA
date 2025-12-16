// =============================================================================
// store.ts - Configuration du store Alpine.js
// store.ts - Alpine.js store configuration
// =============================================================================

// Import de la fonction invoke pour appeler les commandes Tauri
// Import invoke function to call Tauri commands
import { invoke } from "@tauri-apps/api/core";

// Import de la fonction hexToRgb depuis les utilitaires
// Import hexToRgb function from utilities
import { hexToRgb } from './utils';

// =============================================================================
// INTERFACES TYPESCRIPT
// TYPESCRIPT INTERFACES
// =============================================================================

// Interface pour le résultat retourné par la commande pick_color de Tauri
// Interface for the result returned by Tauri's pick_color command
export interface ColorResult {
  // Couleur de premier plan au format RGB [r, g, b] ou null
  // Foreground color in RGB format [r, g, b] or null
  foreground: [number, number, number] | null;

  // Couleur d'arrière-plan au format RGB [r, g, b] ou null
  // Background color in RGB format [r, g, b] or null
  background: [number, number, number] | null;

  // Indique si le mode continu est activé
  // Indicates if continue mode is enabled
  continue_mode: boolean;
}

// Interface pour le store Tauri (état global côté backend)
// Interface for Tauri store (global state on backend side)
export interface ColorStore {
  // Couleur de premier plan au format hexadécimal "#RRGGBB" ou null
  // Foreground color in hexadecimal format "#RRGGBB" or null
  foreground: string | null;

  // Couleur d'arrière-plan au format hexadécimal "#RRGGBB" ou null
  // Background color in hexadecimal format "#RRGGBB" or null
  background: string | null;

  // Indique si le mode continu est activé
  // Indicates if continue mode is enabled
  continue_mode: boolean;
}

// Interface pour le store Alpine.js du color picker (état local côté frontend)
// Interface for Alpine.js color picker store (local state on frontend side)
export interface ColorPickerStore {
  // Indique si une sélection de couleur est en cours
  // Indicates if a color selection is in progress
  isPicking: boolean;

  // Indique si les résultats doivent être affichés
  // Indicates if results should be displayed
  resultVisible: boolean;

  // Couleur de premier plan au format hexadécimal
  // Foreground color in hexadecimal format
  foreground: string;

  // Couleur d'arrière-plan au format hexadécimal
  // Background color in hexadecimal format
  background: string;

  // Couleur de premier plan au format RGB "r, g, b" pour affichage
  // Foreground color in RGB format "r, g, b" for display
  foregroundRgb: string;

  // Couleur d'arrière-plan au format RGB "r, g, b" pour affichage
  // Background color in RGB format "r, g, b" for display
  backgroundRgb: string;

  // Indique si la notification "Copied!" doit être affichée
  // Indicates if "Copied!" notification should be displayed
  copiedVisible: boolean;

  // Méthode pour lancer le sélecteur de couleur
  // Method to launch the color picker
  pickColor(fg: boolean): Promise<void>;

  // Méthode pour copier une couleur dans le presse-papiers
  // Method to copy a color to clipboard
  copyHex(isforeground: boolean): Promise<void>;

  // Méthode pour mettre à jour le store Alpine depuis le store Tauri
  // Method to update Alpine store from Tauri store
  updateFromTauriStore(store: ColorStore): void;
}

// =============================================================================
// CONFIGURATION DU STORE
// STORE CONFIGURATION
// =============================================================================

// Configuration du store Alpine.js exportée pour utilisation dans main.ts
// Alpine.js store configuration exported for use in main.ts
export const colorPickerStore = {
  // État initial : aucune sélection en cours
  // Initial state: no selection in progress
  isPicking: false,

  // État initial : résultats cachés
  // Initial state: results hidden
  resultVisible: false,

  // État initial : aucune couleur de premier plan
  // Initial state: no foreground color
  foreground: '',

  // État initial : aucune couleur d'arrière-plan
  // Initial state: no background color
  background: '',

  // État initial : RGB de premier plan vide
  // Initial state: empty foreground RGB
  foregroundRgb: '',

  // État initial : RGB d'arrière-plan vide
  // Initial state: empty background RGB
  backgroundRgb: '',

  // État initial : notification de copie cachée
  // Initial state: copy notification hidden
  copiedVisible: false,

  // Méthode asynchrone pour lancer le sélecteur de couleur
  // Asynchronous method to launch the color picker
  async pickColor(this: ColorPickerStore, fg: boolean = true) {
    // Active l'indicateur de sélection en cours (désactive le bouton)
    // Enable picking indicator (disables button)
    this.isPicking = true;

    try {
      // Appelle la commande Tauri pick_color avec le paramètre fg (true = foreground, false = background)
      // L'appel met automatiquement à jour le store Tauri côté backend
      // et émet l'événement "store-updated" qui sera capturé par le listener ci-dessous
      // Calls Tauri pick_color command with fg parameter (true = foreground, false = background)
      // The call automatically updates Tauri store on backend side
      // and emits "store-updated" event which will be captured by the listener below
      await invoke<ColorResult>('pick_color', { fg });
    } catch (error) {
      // Affiche l'erreur dans la console si la sélection échoue
      // Display error in console if selection fails
      console.error('Error:', error);
    } finally {
      // Désactive l'indicateur de sélection (réactive le bouton)
      // Disable picking indicator (re-enable button)
      this.isPicking = false;
    }
  },

  // Méthode asynchrone pour copier une couleur dans le presse-papiers
  // Asynchronous method to copy a color to clipboard
  async copyHex(this: ColorPickerStore, isForeground: boolean = true) {
    try {
      // Sélectionne la couleur à copier selon le paramètre (foreground ou background)
      // Select color to copy based on parameter (foreground or background)
      const hex = isForeground ? this.foreground : this.background;

      // Copie la couleur dans le presse-papiers du système
      // Copy color to system clipboard
      await navigator.clipboard.writeText(hex);

      // Affiche la notification "Copied!"
      // Display "Copied!" notification
      this.copiedVisible = true;

      // Masque la notification après 1.5 secondes
      // Hide notification after 1.5 seconds
      setTimeout(() => {
        this.copiedVisible = false;
      }, 1500);
    } catch (err) {
      // Affiche l'erreur dans la console si la copie échoue
      // Display error in console if copy fails
      console.error('Failed to copy:', err);
    }
  },

  // Méthode pour synchroniser le store Alpine avec le store Tauri
  // Method to synchronize Alpine store with Tauri store
  updateFromTauriStore(this: ColorPickerStore, store: ColorStore) {
    // Si le store Tauri contient une couleur de premier plan
    // If Tauri store contains a foreground color
    if (store.foreground) {
      // Met à jour la couleur de premier plan (format hex)
      // Update foreground color (hex format)
      this.foreground = store.foreground;

      // Convertit et stocke la version RGB pour l'affichage
      // Convert and store RGB version for display
      this.foregroundRgb = hexToRgb(store.foreground);

      // Affiche la section des résultats
      // Display results section
      this.resultVisible = true;
    }

    // Si le store Tauri contient une couleur d'arrière-plan
    // If Tauri store contains a background color
    if (store.background) {
      // Met à jour la couleur d'arrière-plan (format hex)
      // Update background color (hex format)
      this.background = store.background;

      // Convertit et stocke la version RGB pour l'affichage
      // Convert and store RGB version for display
      this.backgroundRgb = hexToRgb(store.background);

      // Affiche la section des résultats
      // Display results section
      this.resultVisible = true;
    }
  }
};
