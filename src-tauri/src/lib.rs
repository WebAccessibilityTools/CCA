// =============================================================================
// lib.rs - Backend Tauri avec store réactif
// lib.rs - Tauri backend with reactive store
// =============================================================================

// Import de Mutex pour la synchronisation thread-safe
// Import Mutex for thread-safe synchronization
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

/// Gestion des profils ICC
/// ICC profile management
mod icc;

// =============================================================================
// INITIALISATION
// INITIALIZATION
// =============================================================================
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

// Import pour le système de menu
// Import for the menu system
use tauri::menu::{CheckMenuItemBuilder, Menu, PredefinedMenuItem, Submenu, SubmenuBuilder, AboutMetadata};

// Import pour l'émission d'événements
// Import for event emission
use tauri::Emitter;

/// Préfixe utilisé pour les IDs des éléments de menu ICC
/// Prefix used for ICC menu item IDs
const ICC_MENU_PREFIX: &str = "icc_profile_";

/// Convertit un nom de profil en ID de menu
/// Converts a profile name to a menu ID
///
/// # Arguments
/// * `name` - Nom du profil ICC / ICC profile name
///
/// # Returns
/// * ID de menu formaté / Formatted menu ID
fn profile_name_to_menu_id(name: &str) -> String {
    // Concatène le préfixe avec le nom en minuscules et espaces remplacés par underscores
    // Concatenate prefix with lowercase name and spaces replaced by underscores
    format!("{}{}", ICC_MENU_PREFIX, name.to_lowercase().replace(' ', "_"))
}

/// Extrait le nom du profil depuis un ID de menu
/// Extracts profile name from a menu ID
///
/// # Arguments
/// * `menu_id` - ID de l'élément de menu / Menu item ID
///
/// # Returns
/// * Option contenant le nom du profil si trouvé / Option containing profile name if found
fn menu_id_to_profile_name(menu_id: &str) -> Option<String> {
    // Vérifie si l'ID commence par le préfixe ICC
    // Check if ID starts with ICC prefix
    if menu_id.starts_with(ICC_MENU_PREFIX) {
        // Récupère la liste des profils pour trouver le nom exact
        // Get profile list to find exact name
        let profiles = icc::list_icc_profiles();

        // Cherche le profil dont l'ID correspond
        // Find profile whose ID matches
        for profile in profiles {
            // Compare l'ID généré avec l'ID reçu
            // Compare generated ID with received ID
            if profile_name_to_menu_id(&profile.name) == menu_id {
                // Retourne le nom du profil
                // Return profile name
                return Some(profile.name);
            }
        }
    }

    // Aucun profil trouvé
    // No profile found
    None
}

/// Crée le sous-menu ICC avec tous les profils disponibles
/// Creates the ICC submenu with all available profiles
///
/// # Arguments
/// * `app` - Handle de l'application Tauri / Tauri application handle
///
/// # Returns
/// * `Result<Submenu<tauri::Wry>, tauri::Error>` - Le sous-menu ICC créé
fn create_icc_submenu<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<Submenu<R>, tauri::Error> {
    // Récupère la liste des profils ICC disponibles sur le système
    // Get the list of ICC profiles available on the system
    let profiles = icc::list_icc_profiles();

    // Crée le constructeur du sous-menu ICC
    // Create the ICC submenu builder
    let mut icc_submenu_builder = SubmenuBuilder::new(app, "Colour Profiles");

    // Itère sur chaque profil pour créer un élément de menu
    // Iterate over each profile to create a menu item
    for profile in &profiles {
        // Génère un ID unique pour l'élément de menu
        // Generate a unique ID for the menu item
        let menu_id = profile_name_to_menu_id(&profile.name);

        // Crée un élément de menu avec case à cocher
        // Create a check menu item
        let menu_item = CheckMenuItemBuilder::with_id(menu_id, &profile.name)
            // Coche l'élément si c'est le profil actuel
            // Check item if it's the current profile
            .checked(profile.is_current)
            // Construit l'élément de menu
            // Build the menu item
            .build(app)?;

        // Ajoute l'élément au sous-menu
        // Add item to submenu
        icc_submenu_builder = icc_submenu_builder.item(&menu_item);
    }

    // Log le nombre de profils chargés
    // Log the number of loaded profiles
    println!("Loaded {} ICC profiles into menu", profiles.len());

    // Construit et retourne le sous-menu ICC
    // Build and return the ICC submenu
    icc_submenu_builder.build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Initialise l'état global du color store
        // Initialize global color store state
        .manage(store::AppState {
            store: Mutex::new(store::ResultStore::default()),
        })
        // Configure le menu de l'application
        // Configure the application menu
        .setup(|app| {
            // Récupère le handle de l'application
            // Get the application handle
            let handle = app.handle();

            // === MENU APPLICATION (premier menu sur macOS) ===
            // === APPLICATION MENU (first menu on macOS) ===
            // Crée l'élément "À propos" avec métadonnées / Create "About" item with metadata
            let about = PredefinedMenuItem::about(
                app,
                Some("About CCA"), // Titre / Title
                Some(AboutMetadata {
                    name: Some("CCA".to_string()),    // Nom de l'app / App name
                    version: Some("1.0.0".to_string()),           // Version / Version
                    copyright: Some("xxx Licence".to_string()), // Copyright / Copyright
                    authors: Some(vec!["Cédric Trévisan".to_string()]), // Auteurs / Authors
                    ..Default::default()                          // Autres champs par défaut / Other fields default
                }),
            )?;

            // Éléments standards du menu Application / Standard Application menu items
            let separator1 = PredefinedMenuItem::separator(app)?;           // Séparateur / Separator
            let hide = PredefinedMenuItem::hide(app, Some("Masquer"))?;     // Masquer l'app / Hide app
            let hide_others = PredefinedMenuItem::hide_others(app, Some("Masquer les autres"))?; // Masquer autres / Hide others
            let show_all = PredefinedMenuItem::show_all(app, Some("Tout afficher"))?; // Tout afficher / Show all
            let separator3 = PredefinedMenuItem::separator(app)?;           // Séparateur / Separator
            let quit = PredefinedMenuItem::quit(app, Some("Quitter"))?;     // Quitter / Quit
            
            // Construit le sous-menu Application / Build Application submenu
            let app_menu = Submenu::with_items(
                app,
                "CCA",  // Nom affiché dans la barre de menu / Name shown in menu bar
                true,       // Activé / Enabled
                &[
                    &about,       // À propos / About
                    &separator1,  // --- / ---
                    &hide,        // Masquer / Hide
                    &hide_others, // Masquer les autres / Hide others
                    &show_all,    // Tout afficher / Show all
                    &separator3,  // --- / ---
                    &quit,        // Quitter / Quit
                ],
            )?;

            #[cfg(target_os = "macos")]
            {
                // Crée le sous-menu ICC avec les profils
                // Create the ICC submenu with profiles
                let icc_submenu = create_icc_submenu(handle)?;

                // Crée le menu de l'application
                // Get the application menu
                let root_menu = Menu::with_items(app,&[
                    &app_menu,
                    &icc_submenu,
                ])?;
                // Applique le menu à l'application
                // Apply menu to the application
                app.set_menu(root_menu)?;
            }

            #[cfg(any(target_os = "windows", target_os = "linux"))]
            {
                // Crée le menu de l'application
                // Get the application menu
                let root_menu = Menu::with_items(app,&[
                    &app_menu,
                ])?;
                // Applique le menu à l'application
                // Apply menu to the application
                app.set_menu(root_menu)?;
            }

            // Retourne Ok pour indiquer le succès
            // Return Ok to indicate success
            Ok(())
        })
        // Gestionnaire d'événements de menu
        // Menu event handler
        .on_menu_event(|app, event| {
            // Récupère l'ID de l'élément de menu cliqué
            // Get the clicked menu item ID
            let menu_id = event.id().as_ref();

            // Tente d'extraire le nom du profil depuis l'ID
            // Try to extract profile name from ID
            if let Some(profile_name) = menu_id_to_profile_name(menu_id) {
                // Met à jour le profil sélectionné dans le backend
                // Update the selected profile in the backend
                let _ = icc::select_icc_profile(profile_name.clone());

                // Récupère tous les profils disponibles
                // Get all available profiles
                let profiles = icc::list_icc_profiles();

                // Déselectionne tous les profils d'abord
                // Deselect all profiles first
                for profile in &profiles {
                    let id = profile_name_to_menu_id(&profile.name);

                    // Essaie de trouver l'item dans le menu principal
                    // Try to find item in main menu
                    if let Some(menu) = app.menu() {
                        // Cherche d'abord dans le menu principal
                        // First search in main menu
                        if let Some(item) = menu.get(&id) {
                            if let Some(check_item) = item.as_check_menuitem() {
                                let _ = check_item.set_checked(false);
                            }
                        }
                        // Sinon, cherche dans tous les items du menu récursivement
                        // Otherwise, search recursively in all menu items
                        else if let Ok(items) = menu.items() {
                            for menu_item in items {
                                // Si c'est un sous-menu, cherche dedans
                                // If it's a submenu, search inside
                                if let Some(submenu) = menu_item.as_submenu() {
                                    if let Some(subitem) = submenu.get(&id) {
                                        if let Some(check_item) = subitem.as_check_menuitem() {
                                            let _ = check_item.set_checked(false);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Maintenant, coche uniquement le profil sélectionné
                // Now, check only the selected profile
                let selected_id = profile_name_to_menu_id(&profile_name);
                if let Some(menu) = app.menu() {
                    if let Some(item) = menu.get(&selected_id) {
                        if let Some(check_item) = item.as_check_menuitem() {
                            let _ = check_item.set_checked(true);
                        }
                    } else if let Ok(items) = menu.items() {
                        for menu_item in items {
                            if let Some(submenu) = menu_item.as_submenu() {
                                if let Some(subitem) = submenu.get(&selected_id) {
                                    if let Some(check_item) = subitem.as_check_menuitem() {
                                        let _ = check_item.set_checked(true);
                                    }
                                }
                            }
                        }
                    }
                }

                // Émet un événement pour notifier le frontend du changement
                // Emit event to notify frontend of the change
                let _ = app.emit("icc-profile-changed", &profile_name);

                // Log le changement de profil
                // Log profile change
                println!("ICC Profile changed via menu: {}", profile_name);
            }
        })
        // Enregistre les commandes Tauri
        // Register Tauri commands
        .invoke_handler(tauri::generate_handler![
            store::get_store,
            store::pick_color,
            store::update_store,
            store::clear_store,
            icc::list_icc_profiles,
            icc::select_icc_profile,
            icc::get_selected_icc_profile,
        ])
        // Lance l'application Tauri
        // Run the Tauri application
        .run(tauri::generate_context!())
        // Affiche un message d'erreur si le lancement échoue
        // Display error message if launch fails
        .expect("error while running tauri application");
}