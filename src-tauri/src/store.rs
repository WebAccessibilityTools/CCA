// =============================================================================
// store.rs - Store management module
// =============================================================================

use tauri::{AppHandle, Emitter};
use std::sync::Mutex;
use serde::{Serialize, Deserialize};
use crate::config;
use crate::picker;
use crate::color;
use bigcolor::BigColor;

// =============================================================================
// STORE - État global partagé
// STORE - Shared global state
// =============================================================================

/// Structure du store - contient toutes les données réactives
/// Store structure - contains all reactive data
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ColorStore {
    /// Plateforme actuelle (macos, windows, linux)
    /// Current platform (macos, windows, linux)
    pub platform: &'static str,

    /// Couleur de premier plan (BigColor) - ignorée par la sérialisation
    /// Foreground color (BigColor) - ignored by serialization
    #[serde(skip)]
    pub foreground: BigColor,

    /// Couleur d'arrière-plan (BigColor) - ignorée par la sérialisation
    /// Background color (BigColor) - ignored by serialization
    #[serde(skip)]
    pub background: BigColor,

    /// Couleur de premier plan au format RGB (r, g, b)
    /// Foreground color in RGB format (r, g, b)
    pub foreground_rgb: (u8, u8, u8),

    /// Couleur de premier plan au format hexadécimal
    /// Foreground color in hexadecimal format
    pub foreground_hex: String,

    /// Si la couleur est sombre
    /// If the colour is dark
    pub foreground_is_dark: bool,

    /// Couleur d'arrière-plan au format RGB (r, g, b)
    /// Background color in RGB format (r, g, b)
    pub background_rgb: (u8, u8, u8),

    /// Couleur d'arrière-plan au format hexadécimal
    /// Background color in hexadecimal format
    pub background_hex: String,

    /// Si la couleur est sombre
    /// If the colour is dark
    pub background_is_dark: bool,

    /// Mode continue activé
    /// Continue mode enabled
    pub continue_mode: bool,

    // Contast Ratio value, not rounded
    // Valeur du Ratio de Contraste, non arrondi
    #[serde(skip)]
    pub contrast_ratio_raw: f32,

    // Contast Ratio value, rounded
    // Valeur du Ratio de Contraste, arrondi
    pub contrast_ratio_rounded: f32,
}

impl Default for ColorStore {
    fn default() -> Self {
        let (fr, fg, fb) = config::DEFAULT_FOREGROUND_RGB;
        let (br, bg, bb) = config::DEFAULT_BACKGROUND_RGB;
        let fc = BigColor::from_rgb(fr, fg, fb, 1.0);
        let bc = BigColor::from_rgb(br, bg, bb, 1.0);
        let contrast_ratio = fc.get_contrast_ratio(&bc);
        let contrast_ratio_rounded = (contrast_ratio * config::ROUNDING_FACTOR).round() / config::ROUNDING_FACTOR;
        Self {
            // Plateforme détectée à la compilation
            // Platform detected at compile time
            #[cfg(target_os = "macos")]
            platform: "macos",
            #[cfg(target_os = "windows")]
            platform: "windows",
            #[cfg(target_os = "linux")]
            platform: "linux",
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            platform: "unknown",
            foreground: fc,
            background: bc,
            foreground_rgb: config::DEFAULT_FOREGROUND_RGB,
            foreground_hex: format!("#{:02X}{:02X}{:02X}", fr, fg, fb),
            foreground_is_dark: true,
            background_rgb: config::DEFAULT_BACKGROUND_RGB,
            background_hex: format!("#{:02X}{:02X}{:02X}", br, bg, bb),
            background_is_dark: false,
            continue_mode: false,
            contrast_ratio_raw: contrast_ratio,
            contrast_ratio_rounded: contrast_ratio_rounded,
        }
    }
}

/// État de l'application wrappé dans un Mutex pour thread-safety
/// Application state wrapped in Mutex for thread-safety
pub struct AppState {
    pub store: Mutex<ColorStore>,
}

// =============================================================================
// COMMANDES TAURI
// TAURI COMMANDS
// =============================================================================

/// Récupère l'état actuel du store
/// Gets the current store state
#[tauri::command]
pub fn get_store(state: tauri::State<AppState>) -> ColorStore {
    // Verrouille le mutex et clone le contenu
    // Lock the mutex and clone the content
    state.store.lock().unwrap().clone()
}

/// Lance le color picker et met à jour le store automatiquement
/// Launches the color picker and automatically updates the store
#[tauri::command]
pub fn pick_color(app: AppHandle, state: tauri::State<AppState>, fg: bool) {
    // Lance le picker natif
    // Launch the native picker
    let result = picker::run(fg);

    // Met à jour le store avec les couleurs sélectionnées
    // Update the store with selected colors
    {
        // Verrouille le mutex
        // Lock the mutex
        let mut store = state.store.lock().unwrap();

        // Met à jour les couleurs à partir du résultat du picker
        // Update colors from picker result
        color::update_results_from_picker(&mut store, &result);

        // Met à jour le mode continue
        // Update continue mode
        store.continue_mode = result.continue_mode;

        // Émet l'événement "store-updated" avec le nouveau state
        // Emit "store-updated" event with the new state
        let _ = app.emit("store-updated", store.clone());
    }
}

/// Met à jour une valeur du store manuellement
/// Manually updates a store value
#[tauri::command]
pub fn update_store(app: AppHandle, state: tauri::State<AppState>, key: String, r: u8, g: u8, b: u8) {
    {
        let mut store = state.store.lock().unwrap();

        // Met à jour la clé correspondante
        // Update the corresponding key
        match key.as_str() {
            "foreground" => {
                store.foreground_rgb = (r, g, b);
                store.foreground_hex = format!("#{:02X}{:02X}{:02X}", r, g, b);
                store.foreground = BigColor::from_rgb(r, g, b, 1.0);
                store.foreground_is_dark = store.foreground.is_dark();
            }
            "background" => {
                store.background_rgb = (r, g, b);
                store.background_hex = format!("#{:02X}{:02X}{:02X}", r, g, b);
                store.background = BigColor::from_rgb(r, g, b, 1.0);
                store.background_is_dark = store.background.is_dark();
            }
            _ => return, // Clé inconnue / Unknown key
        }

        // Recalcule le ratio de contraste
        // Recalculate contrast ratio
        store.contrast_ratio_raw = store.foreground.get_contrast_ratio(&store.background);

        // Émet l'événement
        // Emit the event
        let _ = app.emit("store-updated", store.clone());
    }
}

/// Efface le store
/// Clears the store
#[tauri::command]
pub fn clear_store(app: AppHandle, state: tauri::State<AppState>) {
    {
        let mut store = state.store.lock().unwrap();
        *store = ColorStore::default();
        let _ = app.emit("store-updated", store.clone());
    }
}
