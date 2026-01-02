// =============================================================================
// store.ts - Configuration du store Alpine.js
// store.ts - Alpine.js store configuration
// =============================================================================

// Import de la fonction invoke pour appeler les commandes Tauri
// Import invoke function to call Tauri commands
import { invoke } from "@tauri-apps/api/core";

// Interface pour le store Tauri (état global côté backend)
// Interface for Tauri store (global state on backend side)
export interface BackendStore {
  // Platform
  platform: string;

  // Couleur de premier plan au format RGB [r, g, b]
  // Foreground color in RGB format [r, g, b]
  foreground_rgb: [number, number, number];

  // Couleur de premier plan au format Hexa
  // Foreground color in Hexa format
  foreground_hex: string;

  /// Si la couleur est sombre
  /// If the colour is dark
  foreground_is_dark: boolean;

  // Couleur d'arrière-plan au format RGB [r, g, b]
  // Background color in RGB format [r, g, b]
  background_rgb: [number, number, number];

  // Couleur d'arrière-plan au format Hexa
  // Background color in Hexa format
  background_hex: string;

  /// Si la couleur est sombre
  /// If the colour is dark
  background_is_dark: boolean;

  // Contrast Ratio (Rounded)
  contrast_ratio_rounded: number;

  // Indique si le mode continu est activé
  // Indicates if continue mode is enabled
  continue_mode: boolean;
}

// Interface pour le store Alpine.js du color picker (état local côté frontend)
// Interface for Alpine.js color picker store (local state on frontend side)
export interface UIStore {
  // Platform
  platform: string;

  // Indique si une sélection de couleur est en cours
  // Indicates if a color selection is in progress
  isPicking: boolean;

  // Couleur de premier plan au format RGB "r, g, b" pour affichage
  // Foreground color in RGB format "r, g, b" for display
  foregroundRgb: string;

  // Couleur de premier plan au format hexadécimal
  // Foreground color in hexadecimal format
  foregroundHex: string;

  /// Si la couleur est sombre
  /// If the colour is dark
  foregroundIsDark: boolean;

  // Couleur d'arrière-plan au format RGB "r, g, b" pour affichage
  // Background color in RGB format "r, g, b" for display
  backgroundRgb: string;

  // Couleur d'arrière-plan au format hexadécimal
  // Background color in hexadecimal format
  backgroundHex: string;

  /// Si la couleur est sombre
  /// If the colour is dark
  backgroundIsDark: boolean;

  // Contrast Ratio Rounded
  contrastRatio: string;

  // Profil ICC actuellement sélectionné
  // Currently selected ICC profile
  currentICCProfile: string;


  // WCAG Levels
  level143Regular: boolean;
  level143Large: boolean;
  level146Regular: boolean;
  level146Large: boolean;
  level1411: boolean;

  // Méthode pour lancer le sélecteur de couleur
  // Method to launch the color picker
  pickColor(fg: boolean): Promise<void>;

  // Méthode pour mettre à jour le store Alpine depuis le store Tauri
  // Method to update Alpine store from Tauri store
  updateFromTauriStore(store: BackendStore): void;
}

// =============================================================================
// CONFIGURATION DU STORE
// STORE CONFIGURATION
// =============================================================================

// Configuration du store Alpine.js exportée pour utilisation dans main.ts
// Alpine.js store configuration exported for use in main.ts
export const UIStore = {
  platform: 'unknown',

  // État initial : aucune sélection en cours
  // Initial state: no selection in progress
  isPicking: false,

  // État initial : RGB de premier plan vide
  // Initial state: empty foreground RGB
  foregroundRgb: '',

  // État initial : aucune couleur de premier plan
  // Initial state: no foreground color
  foregroundHex: '',

  /// Si la couleur est sombre
  /// If the colour is dark
  foregroundIsDark: true,

  // État initial : RGB d'arrière-plan vide
  // Initial state: empty background RGB
  backgroundRgb: '',

  // État initial : aucune couleur d'arrière-plan
  // Initial state: no background color
  backgroundHex: '',

  /// Si la couleur est sombre
  /// If the colour is dark
  backgroundIsDark: false,

  // Initial state: Contrast ratio
  contrastRatio: 'xxx:1',

  // État initial : profil ICC par défaut (Auto)
  // Initial state: default ICC profile (Auto)
  currentICCProfile: 'Auto',


  // WCAG Levels
  level143Regular: true,
  level143Large: true,
  level146Regular: true,
  level146Large: true,
  level1411: true,

  // Méthode asynchrone pour lancer le sélecteur de couleur
  // Asynchronous method to launch the color picker
  async pickColor(this: UIStore, fg: boolean = true) {
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
      await invoke('pick_color', { fg });
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

  // Méthode pour synchroniser le store Alpine avec le store Tauri
  // Method to synchronize Alpine store with Tauri store
  updateFromTauriStore(this: UIStore, store: BackendStore) {
    this.platform = store.platform;

    // Déstructure le tuple RGB de la couleur de premier plan
    // Destructure RGB tuple of foreground color
    const [fr, fg, fb] = store.foreground_rgb;

    // Stocke la version RGB pour l'affichage
    // Store RGB version for display
    this.foregroundRgb = `${fr}, ${fg}, ${fb}`;

    // Met à jour la couleur de premier plan (format hex)
    // Update foreground color (hex format)
    this.foregroundHex = store.foreground_hex;

    /// Si la couleur est sombre
    /// If the colour is dark
    this.foregroundIsDark = store.foreground_is_dark;

    // Déstructure le tuple RGB de la couleur d'arrière-plan
    // Destructure RGB tuple of background color
    const [br, bg, bb] = store.background_rgb;

    // Stocke la version RGB pour l'affichage
    // Store RGB version for display
    this.backgroundRgb = `${br}, ${bg}, ${bb}`;

    // Met à jour la couleur d'arrière-plan (format hex)
    // Update background color (hex format)
    this.backgroundHex = store.background_hex;

    /// Si la couleur est sombre
    /// If the colour is dark
    this.backgroundIsDark = store.background_is_dark;

    this.contrastRatio = `${store.contrast_ratio_rounded}:1`;

    // Update WCAG Level rules, based on contrast ratio
    this.level143Regular = true;
    this.level143Large = true;
    this.level146Regular = true;
    this.level146Large = true;
    this.level1411 = true;

    if (store.contrast_ratio_rounded < 7) {
      this.level146Regular = false;
    }
    if (store.contrast_ratio_rounded < 4.5) {
      this.level143Regular = false;
      this.level146Large = false;
    }
    if (store.contrast_ratio_rounded < 3) {
      this.level143Large = false;
      this.level1411 = false;
    }
  }
};