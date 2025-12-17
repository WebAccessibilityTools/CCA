// =============================================================================
// color.rs - Color manipulation and store update functions
// =============================================================================

use bigcolor::BigColor;
use crate::store::ColorStore;
use crate::picker::common::ColorPickerResult;
use crate::config;

/// Met à jour les résultats du store à partir du résultat du picker
/// Updates the store results from picker result
///
/// # Arguments
/// * `store` - Le store à mettre à jour / The store to update
/// * `result` - Le résultat du color picker / The color picker result
pub fn update_results_from_picker(store: &mut ColorStore, result: &ColorPickerResult) {
    // Met à jour foreground si sélectionné
    // Update foreground if selected
    if let Some((r, g, b)) = result.foreground {
        store.foreground_rgb = (r, g, b);
        store.foreground_hex = format!("#{:02X}{:02X}{:02X}", r, g, b);
        store.foreground = BigColor::from_rgb(r, g, b, 1.0);
    }

    // Met à jour background si sélectionné
    // Update background if selected
    if let Some((r, g, b)) = result.background {
        store.background_rgb = (r, g, b);
        store.background_hex = format!("#{:02X}{:02X}{:02X}", r, g, b);
        store.background = BigColor::from_rgb(r, g, b, 1.0);
    }

    // Calcule le ratio de contraste
    // Calculate contrast ratio
    store.contrast_ratio_raw = store.foreground.get_contrast_ratio(&store.background);

    // Round the contrast ratio, to 3 decimal
    store.contrast_ratio_rounded = (store.contrast_ratio_raw * config::ROUNDING_FACTOR).round() / config::ROUNDING_FACTOR;
}
