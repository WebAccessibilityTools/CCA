// =============================================================================
// i18n.rs - Traductions des menus natifs
// i18n.rs - Native menu translations
// =============================================================================

/// Retourne la traduction d'une clé de menu pour une locale donnée
/// Returns the translation of a menu key for a given locale
pub fn menu_t(locale: &str, key: &str) -> &'static str {
    match (locale, key) {
        // === English ===
        ("en", "about") => "About CCA",
        ("en", "hide") => "Hide",
        ("en", "hide_others") => "Hide Others",
        ("en", "show_all") => "Show All",
        ("en", "quit") => "Quit",
        ("en", "colour_profiles") => "Colour Profiles",
        ("en", "language") => "Language",
        ("en", "settings") => "Settings\u{2026}",
        ("en", "settings_title") => "Settings",
        ("en", "edit") => "Edit",
        ("en", "copy_templates") => "Copy Templates",

        // === Français ===
        ("fr", "about") => "À propos de CCA",
        ("fr", "hide") => "Masquer",
        ("fr", "hide_others") => "Masquer les autres",
        ("fr", "show_all") => "Tout afficher",
        ("fr", "quit") => "Quitter",
        ("fr", "colour_profiles") => "Profils de couleurs",
        ("fr", "language") => "Langue",
        ("fr", "settings") => "Pr\u{00e9}f\u{00e9}rences\u{2026}",
        ("fr", "settings_title") => "Pr\u{00e9}f\u{00e9}rences",
        ("fr", "edit") => "\u{00c9}dition",
        ("fr", "copy_templates") => "Mod\u{00e8}les de copie",

        // Fallback vers l'anglais / Fallback to English
        (_, "about") => "About CCA",
        (_, "hide") => "Hide",
        (_, "hide_others") => "Hide Others",
        (_, "show_all") => "Show All",
        (_, "quit") => "Quit",
        (_, "colour_profiles") => "Colour Profiles",
        (_, "language") => "Language",
        (_, "settings") => "Settings\u{2026}",
        (_, "settings_title") => "Settings",
        (_, "edit") => "Edit",
        (_, "copy_templates") => "Copy Templates",

        // Clé inconnue / Unknown key
        _ => "?",
    }
}
