//! =============================================================================
//! COMMON.RS - Code partagé entre les plateformes
//! COMMON.RS - Shared code between platforms
//! =============================================================================
//!
//! Ce module contient les types et fonctions utilisés par macOS et Windows.
//! This module contains types and functions used by both macOS and Windows.

// =============================================================================
// STRUCTURES DE RÉSULTAT
// RESULT STRUCTURES
// =============================================================================

/// Résultat retourné par le color picker
/// Result returned by the color picker
/// 
/// Contient les couleurs sélectionnées pour le foreground et le background.
/// Contains selected colors for foreground and background.
#[derive(Clone, Debug, Default)]
pub struct ColorPickerResult {
    /// Couleur de premier plan (foreground) - RGB
    /// Foreground color - RGB
    pub foreground: Option<(u8, u8, u8)>,
    
    /// Couleur d'arrière-plan (background) - RGB
    /// Background color - RGB
    pub background: Option<(u8, u8, u8)>,
    
    /// Indique si le mode continue était activé
    /// Indicates if continue mode was enabled
    pub continue_mode: bool,
}

// =============================================================================
// FONCTIONS DE CALCUL DE COULEUR
// COLOR CALCULATION FUNCTIONS
// =============================================================================

/// Calcule la luminance relative d'une couleur RGB
/// Calculates the relative luminance of an RGB color
/// 
/// Utilise la formule standard ITU-R BT.601:
/// Uses the standard ITU-R BT.601 formula:
/// Y = 0.299 * R + 0.587 * G + 0.114 * B
/// 
/// # Arguments
/// * `r` - Composante rouge (0-255) / Red component (0-255)
/// * `g` - Composante verte (0-255) / Green component (0-255)
/// * `b` - Composante bleue (0-255) / Blue component (0-255)
/// 
/// # Returns
/// Luminance entre 0.0 (noir) et 255.0 (blanc)
/// Luminance between 0.0 (black) and 255.0 (white)
#[inline]
fn calculate_luminance(r: u8, g: u8, b: u8) -> f64 {
    0.299 * (r as f64) + 0.587 * (g as f64) + 0.114 * (b as f64)
}

/// Détermine si le texte doit être noir ou blanc selon la couleur de fond
/// Determines if text should be black or white based on background color
/// 
/// # Arguments
/// * `r`, `g`, `b` - Couleur de fond / Background color
/// 
/// # Returns
/// `true` si le texte doit être noir, `false` si blanc
/// `true` if text should be black, `false` if white
#[inline]
pub fn should_use_dark_text(r: u8, g: u8, b: u8) -> bool {
    calculate_luminance(r, g, b) > 128.0
}

// =============================================================================
// FONCTIONS DE FORMATAGE
// FORMATTING FUNCTIONS
// =============================================================================

/// Formate une couleur RGB en chaîne hexadécimale
/// Formats an RGB color as a hex string
/// 
/// # Arguments
/// * `r`, `g`, `b` - Composantes RGB / RGB components
/// 
/// # Returns
/// Chaîne au format "#RRGGBB" / String in "#RRGGBB" format
#[inline]
pub fn format_hex_color(r: u8, g: u8, b: u8) -> String {
    format!("#{:02X}{:02X}{:02X}", r, g, b)
}

/// Formate une couleur avec un préfixe (Foreground/Background)
/// Formats a color with a prefix (Foreground/Background)
/// 
/// # Arguments
/// * `prefix` - Préfixe ("Foreground" ou "Background") / Prefix
/// * `r`, `g`, `b` - Composantes RGB / RGB components
/// 
/// # Returns
/// Chaîne au format "Prefix - #RRGGBB" / String in "Prefix - #RRGGBB" format
#[inline]
pub fn format_labeled_hex_color(prefix: &str, r: u8, g: u8, b: u8) -> String {
    format!("{} - #{:02X}{:02X}{:02X}", prefix, r, g, b)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_luminance() {
        // Noir / Black
        assert!((calculate_luminance(0, 0, 0) - 0.0).abs() < 0.001);
        // Blanc / White
        assert!((calculate_luminance(255, 255, 255) - 255.0).abs() < 0.001);
        // Rouge pur / Pure red
        assert!((calculate_luminance(255, 0, 0) - 76.245).abs() < 0.001);
    }
    
    #[test]
    fn test_dark_text() {
        // Fond blanc -> texte noir / White background -> black text (dark)
        assert!(should_use_dark_text(255, 255, 255));
        // Fond noir -> texte blanc / Black background -> white text (not dark)
        assert!(!should_use_dark_text(0, 0, 0));
    }
    
    #[test]
    fn test_format_hex() {
        assert_eq!(format_hex_color(255, 0, 128), "#FF0080");
        assert_eq!(format_hex_color(0, 0, 0), "#000000");
    }
    
    #[test]
    fn test_format_labeled() {
        assert_eq!(format_labeled_hex_color("Foreground", 255, 0, 0), "Foreground - #FF0000");
        assert_eq!(format_labeled_hex_color("Background", 0, 255, 0), "Background - #00FF00");
    }
}