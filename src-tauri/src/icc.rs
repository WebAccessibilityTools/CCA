// =============================================================================
// pour.rs - Gestion des profils ICC / ICC Profile Management
// =============================================================================
//
// Ce module gère la liste et la sélection des profils ICC disponibles sur le système.
// Sur macOS, il utilise NSColorSpace.availableColorSpaces pour obtenir les profils.
// This module manages listing and selecting ICC profiles available on the system.
// On macOS, it uses NSColorSpace.availableColorSpaces to get the profiles.

// Import de serde pour la sérialisation JSON
// Import serde for JSON serialization
use serde::{Deserialize, Serialize};

// Import de Mutex pour la synchronisation thread-safe
// Import Mutex for thread-safe synchronization
use std::sync::Mutex;

// =============================================================================
// STRUCTURES
// =============================================================================

/// Structure représentant un profil ICC
/// Structure representing an ICC profile
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ICCProfile {
    /// Nom du profil (identifiant unique)
    /// Profile name (unique identifier)
    pub name: String,

    /// Description lisible du profil
    /// Human-readable profile description
    pub description: String,

    /// Indique si ce profil est actuellement sélectionné
    /// Indicates if this profile is currently selected
    pub is_current: bool,
}

// =============================================================================
// ÉTAT GLOBAL
// GLOBAL STATE
// =============================================================================

/// Profil ICC sélectionné globalement (protégé par Mutex)
/// Globally selected ICC profile (protected by Mutex)
static SELECTED_PROFILE: Mutex<Option<String>> = Mutex::new(None);

// =============================================================================
// IMPLÉMENTATION macOS
// macOS IMPLEMENTATION
// =============================================================================

/// Liste tous les profils ICC disponibles sur macOS via NSColorSpace
/// Lists all available ICC profiles on macOS via NSColorSpace
#[cfg(target_os = "macos")]
fn get_system_color_spaces() -> Vec<ICCProfile> {
    // Import des types nécessaires pour macOS
    // Import required types for macOS
    use objc2_app_kit::NSColorSpace;
    use objc2_app_kit::NSColorSpaceModel;
    use objc2_foundation::NSArray;

    // Vecteur pour stocker les profils trouvés
    // Vector to store found profiles
    let mut profiles: Vec<ICCProfile> = Vec::new();

    // Ajoute d'abord le profil "Auto" (détection automatique)
    // First add the "Auto" profile (automatic detection)
    profiles.push(ICCProfile {
        name: "Auto".to_string(),
        description: "Automatic color space detection".to_string(),
        is_current: false,
    });

    // Récupère le tableau des espaces colorimétriques RGB disponibles
    // Get the array of available RGB color spaces
    let color_spaces: objc2::rc::Retained<NSArray<NSColorSpace>> = 
        NSColorSpace::availableColorSpacesWithModel(
            // NSColorSpaceModelRGB pour les espaces RGB uniquement
            // NSColorSpaceModelRGB for RGB spaces only
            NSColorSpaceModel::RGB,
        );

    // Récupère le nombre d'éléments dans le tableau
    // Get the number of elements in the array
    let count = color_spaces.count();

    // Itère sur chaque espace colorimétrique
    // Iterate over each color space
    for i in 0..count {
        // Récupère l'espace colorimétrique à l'index i via objectAtIndex:
        // Get the color space at index i via objectAtIndex:
        let color_space = color_spaces.objectAtIndex(i);

        // Récupère le nom localisé de l'espace colorimétrique
        // Get the localized name of the color space
        if let Some(name_ns) = color_space.localizedName() {
            // Convertit le NSString en String Rust
            // Convert NSString to Rust String
            let name = name_ns.to_string();

            // Crée la description (même que le nom pour l'instant)
            // Create description (same as name for now)
            let description = name.clone();

            // Ajoute le profil à la liste
            // Add profile to the list
            profiles.push(ICCProfile {
                name,
                description,
                is_current: false,
            });
        }
    }

    // Retourne la liste des profils
    // Return the list of profiles
    profiles
}

/// Récupère le NSColorSpace correspondant au profil sélectionné
/// Gets the NSColorSpace corresponding to the selected profile
///
/// # Returns
/// * `Option<Retained<NSColorSpace>>` - L'espace colorimétrique ou None si Auto/non trouvé
/// * `Option<Retained<NSColorSpace>>` - The color space or None if Auto/not found
#[cfg(target_os = "macos")]
pub fn get_selected_nscolorspace() -> Option<objc2::rc::Retained<objc2_app_kit::NSColorSpace>> {
    // Import des types nécessaires pour macOS
    // Import required types for macOS
    use objc2_app_kit::NSColorSpace;
    use objc2_app_kit::NSColorSpaceModel;
    use objc2_foundation::NSArray;

    // Récupère le nom du profil sélectionné
    // Get the selected profile name
    let selected_name = if let Ok(selected) = SELECTED_PROFILE.lock() {
        // Clone le nom pour pouvoir libérer le lock
        // Clone the name to release the lock
        selected.clone()
    } else {
        // En cas d'erreur, retourne None (utilise Auto)
        // On error, return None (use Auto)
        return None;
    };

    // Si aucun profil sélectionné ou "Auto", retourne None
    // If no profile selected or "Auto", return None
    let profile_name = match selected_name {
        Some(name) if name != "Auto" => name,
        _ => return None,
    };

    // Récupère le tableau des espaces colorimétriques RGB disponibles
    // Get the array of available RGB color spaces
    let color_spaces: objc2::rc::Retained<NSArray<NSColorSpace>> = 
        NSColorSpace::availableColorSpacesWithModel(
            // NSColorSpaceModelRGB pour les espaces RGB uniquement
            // NSColorSpaceModelRGB for RGB spaces only
            NSColorSpaceModel::RGB,
        );

    // Récupère le nombre d'éléments dans le tableau
    // Get the number of elements in the array
    let count = color_spaces.count();

    // Cherche l'espace colorimétrique correspondant au nom
    // Find the color space matching the name
    for i in 0..count {
        // Récupère l'espace colorimétrique à l'index i
        // Get the color space at index i
        let color_space = color_spaces.objectAtIndex(i);

        // Récupère le nom localisé de l'espace colorimétrique
        // Get the localized name of the color space
        if let Some(name_ns) = color_space.localizedName() {
            // Convertit le NSString en String Rust
            // Convert NSString to Rust String
            let name = name_ns.to_string();

            // Compare avec le profil recherché
            // Compare with the searched profile
            if name == profile_name {
                // Retourne l'espace colorimétrique trouvé
                // Return the found color space
                return Some(color_space.clone());
            }
        }
    }

    // Profil non trouvé
    // Profile not found
    None
}

/// Convertit une couleur RGB depuis l'espace colorimétrique source vers sRGB
/// Converts an RGB color from source color space to sRGB
///
/// # Arguments
/// * `r`, `g`, `b` - Composantes RGB en u8 (0-255)
/// * `source_colorspace` - Espace colorimétrique source (ou None pour Auto)
///
/// # Returns
/// * `(u8, u8, u8)` - Composantes RGB converties en sRGB
#[cfg(target_os = "macos")]
pub fn convert_color_to_srgb(r: u8, g: u8, b: u8, source_colorspace: Option<&objc2_app_kit::NSColorSpace>) -> (u8, u8, u8) {
    // Import des types nécessaires
    // Import required types
    use objc2_app_kit::{NSColor, NSColorSpace};
    use std::ptr::NonNull;

    // Si pas d'espace colorimétrique source, retourne les couleurs inchangées
    // If no source color space, return colors unchanged
    let source_cs = match source_colorspace {
        Some(cs) => cs,
        None => return (r, g, b),
    };

    // Bloc unsafe pour les appels Objective-C
    // Unsafe block for Objective-C calls
    unsafe {
        // Convertit les valeurs u8 en CGFloat (0.0 - 1.0)
        // Convert u8 values to CGFloat (0.0 - 1.0)
        let r_f: f64 = r as f64 / 255.0;
        let g_f: f64 = g as f64 / 255.0;
        let b_f: f64 = b as f64 / 255.0;
        let a_f: f64 = 1.0;

        // Crée un tableau de composantes [R, G, B, A]
        // Create components array [R, G, B, A]
        let components: [f64; 4] = [r_f, g_f, b_f, a_f];

        // Crée un NonNull pointer vers les composantes
        // Create NonNull pointer to components
        let components_ptr = NonNull::new(components.as_ptr() as *mut f64);

        // Vérifie que le pointeur est valide
        // Check that pointer is valid
        let components_ptr = match components_ptr {
            Some(ptr) => ptr,
            None => return (r, g, b), // Retourne les couleurs inchangées si échec
        };

        // Crée une couleur dans l'espace colorimétrique source
        // Create a color in the source color space
        let source_color = NSColor::colorWithColorSpace_components_count(
            source_cs,
            components_ptr,
            4, // 4 composantes (RGBA) / 4 components (RGBA)
        );

        // Récupère l'espace colorimétrique sRGB de destination
        // Get the destination sRGB color space
        let srgb_cs = NSColorSpace::sRGBColorSpace();

        // Convertit la couleur vers sRGB
        // Convert the color to sRGB
        let srgb_color = match source_color.colorUsingColorSpace(&srgb_cs) {
            Some(c) => c,
            None => return (r, g, b), // Retourne les couleurs inchangées si la conversion échoue
        };

        // Extrait les composantes de la couleur convertie
        // Extract components from the converted color
        let srgb_r = srgb_color.redComponent();
        let srgb_g = srgb_color.greenComponent();
        let srgb_b = srgb_color.blueComponent();

        // Convertit les valeurs CGFloat (0.0 - 1.0) en u8 (0 - 255)
        // Convert CGFloat values (0.0 - 1.0) to u8 (0 - 255)
        let r_out = (srgb_r * 255.0).round().clamp(0.0, 255.0) as u8;
        let g_out = (srgb_g * 255.0).round().clamp(0.0, 255.0) as u8;
        let b_out = (srgb_b * 255.0).round().clamp(0.0, 255.0) as u8;

        (r_out, g_out, b_out)
    }
}

/// Liste tous les profils ICC disponibles sur Windows
/// Lists all available ICC profiles on Windows
#[cfg(target_os = "windows")]
fn get_system_color_spaces() -> Vec<ICCProfile> {
    // Sur Windows, retourne une liste statique pour l'instant
    // On Windows, return a static list for now
    // TODO: Implémenter la récupération des profils ICC via Windows API
    // TODO: Implement ICC profile retrieval via Windows API
    vec![
        ICCProfile {
            name: "Auto".to_string(),
            description: "Automatic color space detection".to_string(),
            is_current: false,
        },
        ICCProfile {
            name: "sRGB".to_string(),
            description: "sRGB IEC61966-2.1 (Standard web)".to_string(),
            is_current: false,
        },
        ICCProfile {
            name: "Adobe RGB".to_string(),
            description: "Adobe RGB (1998)".to_string(),
            is_current: false,
        },
    ]
}

/// Liste tous les profils ICC disponibles sur Linux
/// Lists all available ICC profiles on Linux
#[cfg(target_os = "linux")]
fn get_system_color_spaces() -> Vec<ICCProfile> {
    // Sur Linux, retourne une liste statique pour l'instant
    // On Linux, return a static list for now
    // TODO: Implémenter la récupération des profils ICC via colord ou similaire
    // TODO: Implement ICC profile retrieval via colord or similar
    vec![
        ICCProfile {
            name: "Auto".to_string(),
            description: "Automatic color space detection".to_string(),
            is_current: false,
        },
        ICCProfile {
            name: "sRGB".to_string(),
            description: "sRGB IEC61966-2.1 (Standard web)".to_string(),
            is_current: false,
        },
    ]
}

/// Fallback pour les autres plateformes
/// Fallback for other platforms
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn get_system_color_spaces() -> Vec<ICCProfile> {
    // Retourne uniquement Auto pour les plateformes non supportées
    // Return only Auto for unsupported platforms
    vec![ICCProfile {
        name: "Auto".to_string(),
        description: "Automatic color space detection".to_string(),
        is_current: false,
    }]
}

// =============================================================================
// COMMANDES TAURI
// TAURI COMMANDS
// =============================================================================

/// Liste tous les profils ICC disponibles
/// Lists all available ICC profiles
///
/// # Returns
/// * `Vec<ICCProfile>` - Liste des profils avec leur état de sélection
/// * `Vec<ICCProfile>` - List of profiles with their selection state
#[tauri::command]
pub fn list_icc_profiles() -> Vec<ICCProfile> {
    // Récupère les profils du système
    // Get system profiles
    let mut profiles = get_system_color_spaces();

    // Récupère le profil actuellement sélectionné
    // Get the currently selected profile
    if let Ok(selected) = SELECTED_PROFILE.lock() {
        // Détermine le nom du profil sélectionné (Auto par défaut)
        // Determine the selected profile name (Auto by default)
        let current_name = selected.as_deref().unwrap_or("Auto");

        // Marque le profil sélectionné comme courant
        // Mark the selected profile as current
        for profile in &mut profiles {
            // Compare le nom du profil avec le profil sélectionné
            // Compare profile name with selected profile
            profile.is_current = profile.name == current_name;
        }
    }

    // Retourne la liste des profils
    // Return the list of profiles
    profiles
}

/// Sélectionne un profil ICC
/// Selects an ICC profile
///
/// # Arguments
/// * `profile_name` - Nom du profil à sélectionner / Name of the profile to select
///
/// # Returns
/// * `Result<(), String>` - Ok si succès, Err avec message si échec
/// * `Result<(), String>` - Ok if success, Err with message if failure
#[tauri::command]
pub fn select_icc_profile(profile_name: String) -> Result<(), String> {
    // Tente de verrouiller le mutex
    // Try to lock the mutex
    if let Ok(mut selected) = SELECTED_PROFILE.lock() {
        // Met à jour le profil sélectionné
        // Update the selected profile
        *selected = Some(profile_name.clone());

        // Log pour debug
        // Debug log
        println!("ICC Profile selected: {}", profile_name);

        // Retourne succès
        // Return success
        Ok(())
    } else {
        // Retourne erreur si le mutex ne peut pas être verrouillé
        // Return error if mutex cannot be locked
        Err("Failed to lock profile mutex / Échec du verrouillage du mutex".to_string())
    }
}

/// Récupère le profil ICC actuellement sélectionné
/// Gets the currently selected ICC profile
///
/// # Returns
/// * `Option<String>` - Nom du profil sélectionné ou None
/// * `Option<String>` - Selected profile name or None
#[tauri::command]
pub fn get_selected_icc_profile() -> Option<String> {
    // Verrouille le mutex et retourne une copie du profil sélectionné
    // Lock the mutex and return a copy of the selected profile
    SELECTED_PROFILE.lock().ok().and_then(|s| s.clone())
}

// =============================================================================
// FONCTIONS UTILITAIRES
// UTILITY FUNCTIONS
// =============================================================================

/// Retourne le nom du profil ICC actuellement sélectionné (usage interne)
/// Returns the currently selected ICC profile name (internal use)
///
/// # Returns
/// * Le nom du profil sélectionné ou "Auto" par défaut
/// * The selected profile name or "Auto" as default
#[allow(dead_code)]
pub fn get_current_profile_name() -> String {
    // Verrouille le mutex pour accéder au profil sélectionné
    // Lock the mutex to access the selected profile
    if let Ok(selected) = SELECTED_PROFILE.lock() {
        // Retourne le profil sélectionné ou "Auto" par défaut
        // Return the selected profile or "Auto" as default
        selected.as_deref().unwrap_or("Auto").to_string()
    } else {
        // En cas d'erreur de verrouillage, retourne "Auto"
        // On lock error, return "Auto"
        "Auto".to_string()
    }
}
