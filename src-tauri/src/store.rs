// =============================================================================
// store.rs - Store management module
// =============================================================================

use tauri::{AppHandle, Emitter};
use std::sync::Mutex;
use serde::{Serialize, Deserialize};
use crate::config;
use crate::picker;

// =============================================================================
// STORE - État global partagé
// STORE - Shared global state
// =============================================================================

/// Structure du store - contient toutes les données réactives
/// Store structure - contains all reactive data
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ColorStore {
    /// Couleur de premier plan au format RGB (r, g, b)
    /// Foreground color in RGB format (r, g, b)
    pub foreground_rgb: (u8, u8, u8),

    /// Couleur d'arrière-plan au format RGB (r, g, b)
    /// Background color in RGB format (r, g, b)
    pub background_rgb: (u8, u8, u8),

    /// Mode continue activé
    /// Continue mode enabled
    pub continue_mode: bool,
}

impl Default for ColorStore {
    fn default() -> Self {
        Self {
            foreground_rgb: config::DEFAULT_FOREGROUND_RGB,
            background_rgb: config::DEFAULT_BACKGROUND_RGB,
            continue_mode: false,
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
pub fn pick_color(app: AppHandle, state: tauri::State<AppState>, fg: bool) -> picker::common::ColorPickerResult {
    // Lance le picker natif
    // Launch the native picker
    let result = picker::run(fg);

    // Met à jour le store avec les couleurs sélectionnées
    // Update the store with selected colors
    {
        // Verrouille le mutex
        // Lock the mutex
        let mut store = state.store.lock().unwrap();

        // Met à jour foreground si sélectionné
        // Update foreground if selected
        if let Some((r, g, b)) = result.foreground {
            store.foreground_rgb = (r, g, b);
        }

        // Met à jour background si sélectionné
        // Update background if selected
        if let Some((r, g, b)) = result.background {
            store.background_rgb = (r, g, b);
        }

        // Met à jour le mode continue
        // Update continue mode
        store.continue_mode = result.continue_mode;

        // Émet l'événement "store-updated" avec le nouveau state
        // Emit "store-updated" event with the new state
        let _ = app.emit("store-updated", store.clone());
    }

    result
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
            "foreground" => store.foreground_rgb = (r, g, b),
            "background" => store.background_rgb = (r, g, b),
            _ => return, // Clé inconnue / Unknown key
        }

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
