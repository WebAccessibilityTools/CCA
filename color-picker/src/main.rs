//! CCA Color Picker
//!
//! This application creates a fullscreen overlay that captures the color
//! under the mouse cursor and displays a magnified view with the hex color value.
//!
//! Features:
//! - Circular magnifier following the cursor
//! - Colored border showing the current pixel color
//! - Hex color code displayed along the border arc
//! - Keyboard navigation (arrow keys, Shift+arrows for faster movement)
//! - Scroll wheel zoom control
//! - Click or ESC to exit and copy color
//! 
//! # Controls
//! - Mouse: Move to pick color
//! - Click: Exit and copy color
//! - ESC: Exit
//! - Arrow keys: Fine movement (1 pixel)
//! - Shift + Arrow keys: Fast movement (50 pixels)
//! - Scroll wheel: Zoom in/out

mod config; // Configuration constants module

#[cfg(target_os = "macos")]
mod macos; // macOS-specific implementation

#[cfg(target_os = "windows")]
mod windows; // Windows-specific implementation

#[cfg(target_os = "linux")]
mod linux; // Linux-specific implementation

/// Helper function to format a color as hex string
/// Fonction utilitaire pour formater une couleur en chaîne hexadécimale
fn format_color(r: u8, g: u8, b: u8) -> String {
    format!("#{:02X}{:02X}{:02X}", r, g, b) // Format as #RRGGBB
}

fn main() {
    // Parse command line arguments for fg mode
    // Par défaut fg=true (arc du haut), utiliser --bg pour arc du bas
    // Default fg=true (top arc), use --bg for bottom arc
    let args: Vec<String> = std::env::args().collect(); // Collect command line arguments
    let fg = !args.contains(&"--bg".to_string()); // fg=true unless --bg is passed

    // Variable to store the result from the color picker
    // Variable pour stocker le résultat du color picker
    
    #[cfg(target_os = "macos")]
    let result = macos::run(fg); // Call macOS implementation with fg parameter

    #[cfg(target_os = "windows")]
    let result = {
        // Windows supporte maintenant le mode fg
        // Windows now supports fg mode
        windows::run(fg) // Call Windows implementation with fg parameter
    };

    #[cfg(target_os = "linux")]
    let result = {
        let _ = fg; // Suppress unused variable warning
        // Linux doesn't support fg mode yet, return empty result
        // Linux ne supporte pas encore le mode fg, retourne un résultat vide
        macos::ColorPickerResult {
            foreground: linux::run(), // Use old API for now
            background: None,
        }
    };

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        eprintln!("Unsupported platform"); // Print error message
        std::process::exit(1); // Exit with error code
    }

    // Display the result based on which field is populated
    // Affiche le résultat selon le champ rempli
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        // Check if foreground color was selected
        // Vérifie si une couleur de premier plan a été sélectionnée
        if let Some((r, g, b)) = result.foreground {
            let hex = format_color(r, g, b); // Format as hex
            println!("Foreground: RGB({}, {}, {}) | HEX: {}", r, g, b, hex); // Print foreground
        }

        // Check if background color was selected
        // Vérifie si une couleur d'arrière-plan a été sélectionnée
        if let Some((r, g, b)) = result.background {
            let hex = format_color(r, g, b); // Format as hex
            println!("Background: RGB({}, {}, {}) | HEX: {}", r, g, b, hex); // Print background
        }

        // Exit with error if no color was selected (user pressed ESC)
        // Quitte avec erreur si aucune couleur n'a été sélectionnée (utilisateur a appuyé ESC)
        if result.foreground.is_none() && result.background.is_none() {
            std::process::exit(1); // Exit with error code
        }
    }
}