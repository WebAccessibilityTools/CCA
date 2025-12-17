// =============================================================================
// picker/mod.rs - Color picker module
// =============================================================================

/// Code commun entre plateformes (types, fonctions utilitaires)
/// Common code between platforms (types, utility functions)
pub mod common;

/// Implémentation macOS
/// macOS implementation
#[cfg(target_os = "macos")]
pub mod macos;

/// Implémentation Windows
/// Windows implementation
#[cfg(target_os = "windows")]
pub mod windows;

/// Implémentation Linux (non implémentée)
/// Linux implementation (not implemented)
#[cfg(target_os = "linux")]
pub mod linux;

// =============================================================================
// FONCTION PUBLIQUE
// PUBLIC FUNCTION
// =============================================================================

/// Lance le color picker natif selon la plateforme
/// Launches the native color picker based on the platform
///
/// # Arguments
/// * `fg` - true pour foreground, false pour background
///
/// # Returns
/// * `ColorPickerResult` - Résultat avec les couleurs sélectionnées
pub fn run(fg: bool) -> common::ColorPickerResult {
    #[cfg(target_os = "macos")]
    {
        macos::run(fg)
    }

    #[cfg(target_os = "windows")]
    {
        windows::run(fg)
    }

    #[cfg(target_os = "linux")]
    {
        linux::run(fg)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        common::ColorPickerResult::default()
    }
}
