// =============================================================================
// lib.rs - Backend Tauri avec store réactif
// lib.rs - Tauri backend with reactive store
// =============================================================================

use tauri::{AppHandle, Emitter};
use std::sync::Mutex;
use serde::{Serialize, Deserialize};

// =============================================================================
// MODULES
// =============================================================================

/// Configuration partagée (constantes)
/// Shared configuration (constants)
mod config;

/// Code commun entre plateformes (types, fonctions utilitaires)
/// Common code between platforms (types, utility functions)
mod common;

/// Implémentation macOS
/// macOS implementation
#[cfg(target_os = "macos")]
mod macos;

/// Implémentation Windows
/// Windows implementation
#[cfg(target_os = "windows")]
mod windows;

/// Implémentation Linux (non implémentée)
/// Linux implementation (not implemented)
#[cfg(target_os = "linux")]
mod linux;

// =============================================================================
// STORE - État global partagé
// STORE - Shared global state
// =============================================================================

/// Structure du store - contient toutes les données réactives
/// Store structure - contains all reactive data
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct ColorStore {
    /// Couleur de premier plan sélectionnée (format "#RRGGBB")
    /// Selected foreground color (format "#RRGGBB")
    pub foreground: Option<String>,
    
    /// Couleur d'arrière-plan sélectionnée (format "#RRGGBB")
    /// Selected background color (format "#RRGGBB")
    pub background: Option<String>,
    
    /// Mode continue activé
    /// Continue mode enabled
    pub continue_mode: bool,
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
fn get_store(state: tauri::State<AppState>) -> ColorStore {
    // Verrouille le mutex et clone le contenu
    // Lock the mutex and clone the content
    state.store.lock().unwrap().clone()
}

/// Lance le color picker et met à jour le store automatiquement
/// Launches the color picker and automatically updates the store
#[tauri::command]
fn pick_color(app: AppHandle, state: tauri::State<AppState>, fg: bool) -> common::ColorPickerResult {
    // Lance le picker natif
    // Launch the native picker
    #[cfg(target_os = "macos")]
    let result = macos::run(fg);
    
    #[cfg(not(target_os = "macos"))]
    let result = common::ColorPickerResult::default();
    
    // Met à jour le store avec les couleurs sélectionnées
    // Update the store with selected colors
    {
        // Verrouille le mutex
        // Lock the mutex
        let mut store = state.store.lock().unwrap();
        
        // Met à jour foreground si sélectionné
        // Update foreground if selected
        if let Some((r, g, b)) = result.foreground {
            store.foreground = Some(format!("#{:02X}{:02X}{:02X}", r, g, b));
        }
        
        // Met à jour background si sélectionné
        // Update background if selected
        if let Some((r, g, b)) = result.background {
            store.background = Some(format!("#{:02X}{:02X}{:02X}", r, g, b));
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
fn update_store(app: AppHandle, state: tauri::State<AppState>, key: String, value: String) {
    {
        let mut store = state.store.lock().unwrap();
        
        // Met à jour la clé correspondante
        // Update the corresponding key
        match key.as_str() {
            "foreground" => store.foreground = Some(value),
            "background" => store.background = Some(value),
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
fn clear_store(app: AppHandle, state: tauri::State<AppState>) {
    {
        let mut store = state.store.lock().unwrap();
        *store = ColorStore::default();
        let _ = app.emit("store-updated", store.clone());
    }
}

// =============================================================================
// INITIALISATION
// INITIALIZATION
// =============================================================================
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Initialise l'état global
        // Initialize global state
        .manage(AppState {
            store: Mutex::new(ColorStore::default()),
        })
        // Enregistre les commandes
        // Register commands
        .invoke_handler(tauri::generate_handler![
            get_store,
            pick_color,
            update_store,
            clear_store,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}