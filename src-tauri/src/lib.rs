// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
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


#[tauri::command]
fn pick_color(fg: bool) -> common::ColorPickerResult {
    #[cfg(target_os = "macos")]
    {
        return macos::run(fg); // Appelle l'implémentation macOS avec le paramètre fg
    }

    #[cfg(target_os = "windows")]
    {
        return windows::run(fg); // Appelle l'implémentation Windows avec le paramètre fg
    }

    #[cfg(target_os = "linux")]
    {
        return linux::run(fg); // Appelle l'implémentation Linux avec le paramètre fg
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        // Plateforme non supportée, retourne un résultat vide
        // Unsupported platform, return empty result
        common::ColorPickerResult::default()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![pick_color])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
