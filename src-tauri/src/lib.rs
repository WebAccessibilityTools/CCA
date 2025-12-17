// =============================================================================
// lib.rs - Backend Tauri avec store réactif
// lib.rs - Tauri backend with reactive store
// =============================================================================

use std::sync::Mutex;

// =============================================================================
// MODULES
// =============================================================================

/// Configuration partagée (constantes)
/// Shared configuration (constants)
mod config;

/// Module du color picker (code commun et implémentations par plateforme)
/// Color picker module (common code and platform implementations)
mod picker;

/// Gestion du store et des commandes associées
/// Store management and associated commands
mod store;

/// Fonctions de manipulation de couleurs
/// Color manipulation functions
mod color;

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
        .manage(store::AppState {
            store: Mutex::new(store::ColorStore::default()),
        })
        // Enregistre les commandes
        // Register commands
        .invoke_handler(tauri::generate_handler![
            store::get_store,
            store::pick_color,
            store::update_store,
            store::clear_store,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}