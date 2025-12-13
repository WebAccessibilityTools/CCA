//! =============================================================================
//! CCA Color Picker - Application principale
//! CCA Color Picker - Main application
//! =============================================================================
//!
//! Cette application crée une fenêtre plein écran qui capture la couleur
//! sous le curseur et affiche une vue agrandie avec le code hexadécimal.
//!
//! This application creates a fullscreen overlay that captures the color
//! under the mouse cursor and displays a magnified view with the hex color value.
//!
//! # Fonctionnalités / Features
//! - Loupe circulaire suivant le curseur / Circular magnifier following the cursor
//! - Bordure colorée montrant la couleur actuelle / Colored border showing current color
//! - Code hexadécimal affiché le long de l'arc / Hex code displayed along the arc
//! - Navigation clavier (flèches, Shift+flèches) / Keyboard navigation (arrows, Shift+arrows)
//! - Zoom avec la molette / Scroll wheel zoom
//! - Clic ou ESC pour quitter / Click or ESC to exit
//!
//! # Contrôles / Controls
//! - Souris / Mouse: Déplacer pour choisir / Move to pick color
//! - Clic / Click: Quitter et copier / Exit and copy color
//! - ESC: Quitter / Exit
//! - Flèches / Arrow keys: Déplacement fin (1 pixel) / Fine movement (1 pixel)
//! - Shift + Flèches / Shift + Arrows: Déplacement rapide (50 pixels) / Fast movement
//! - Molette / Scroll wheel: Zoom avant/arrière / Zoom in/out
//! - Tab: Basculer FG/BG / Toggle FG/BG
//! - C: Mode continue / Continue mode

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
// FONCTIONS UTILITAIRES
// UTILITY FUNCTIONS
// =============================================================================

/// Formate une couleur RGB en chaîne hexadécimale
/// Formats an RGB color as a hex string
fn format_color(r: u8, g: u8, b: u8) -> String {
    format!("#{:02X}{:02X}{:02X}", r, g, b) // Format as #RRGGBB
}

// =============================================================================
// POINT D'ENTRÉE
// ENTRY POINT
// =============================================================================

fn main() {
    // Parse les arguments de ligne de commande pour le mode fg
    // Par défaut fg=true (arc du haut), utiliser --bg pour arc du bas
    // Parse command line arguments for fg mode
    // Default fg=true (top arc), use --bg for bottom arc
    let args: Vec<String> = std::env::args().collect(); // Collecte les arguments
    let fg = !args.contains(&"--bg".to_string()); // fg=true sauf si --bg est passé

    // Exécute le color picker selon la plateforme
    // Run the color picker according to platform
    
    #[cfg(target_os = "macos")]
    let result = macos::run(fg); // Appelle l'implémentation macOS avec le paramètre fg

    #[cfg(target_os = "windows")]
    let result = windows::run(fg); // Appelle l'implémentation Windows avec le paramètre fg

    #[cfg(target_os = "linux")]
    let result = {
        let _ = fg; // Supprime l'avertissement de variable non utilisée
        // Linux ne supporte pas encore le mode fg, retourne un résultat vide
        // Linux doesn't support fg mode yet, return empty result
        common::ColorPickerResult {
            foreground: linux::run(), // Utilise l'ancienne API pour l'instant
            background: None,
            continue_mode: false,
        }
    };

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        eprintln!("Plateforme non supportée / Unsupported platform");
        std::process::exit(1);
    }
    
    // Affiche le résultat selon les champs remplis
    // Display the result based on which fields are populated
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        // Vérifie si une couleur de premier plan a été sélectionnée
        // Check if foreground color was selected
        if let Some((r, g, b)) = result.foreground {
            let hex = format_color(r, g, b); // Formate en hex
            println!("Foreground: RGB({}, {}, {}) | HEX: {}", r, g, b, hex);
        }

        // Vérifie si une couleur d'arrière-plan a été sélectionnée
        // Check if background color was selected
        if let Some((r, g, b)) = result.background {
            let hex = format_color(r, g, b); // Formate en hex
            println!("Background: RGB({}, {}, {}) | HEX: {}", r, g, b, hex);
        }

        // Quitte avec erreur si aucune couleur n'a été sélectionnée (ESC pressé)
        // Exit with error if no color was selected (user pressed ESC)
        if result.foreground.is_none() && result.background.is_none() {
            std::process::exit(1);
        }
    }
}