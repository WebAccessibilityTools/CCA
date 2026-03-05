// =============================================================================
// lib.rs - Backend Tauri avec store réactif
// lib.rs - Tauri backend with reactive store
// =============================================================================

// Import de Mutex pour la synchronisation thread-safe
// Import Mutex for thread-safe synchronization
use std::sync::Mutex;
use tauri::Manager;
use tauri::WebviewWindowBuilder;
use tauri::WebviewUrl;

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

/// Internationalisation des menus
/// Menu internationalization
mod i18n;

// =============================================================================
// INITIALISATION
// INITIALIZATION
// =============================================================================
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

// Import pour le système de menu
// Import for the menu system
use tauri::menu::{CheckMenuItemBuilder, Menu, MenuItemBuilder, PredefinedMenuItem, Submenu, SubmenuBuilder, AboutMetadata};

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
/// * `locale` - Locale courante / Current locale
///
/// # Returns
/// * `Result<Submenu<tauri::Wry>, tauri::Error>` - Le sous-menu ICC créé
fn create_icc_submenu<R: tauri::Runtime>(app: &tauri::AppHandle<R>, locale: &str) -> Result<Submenu<R>, tauri::Error> {
    // Récupère la liste des profils ICC disponibles sur le système
    // Get the list of ICC profiles available on the system
    let profiles = icc::list_icc_profiles();

    // Crée le constructeur du sous-menu ICC
    // Create the ICC submenu builder
    let mut icc_submenu_builder = SubmenuBuilder::new(app, i18n::menu_t(locale, "colour_profiles"));

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

/// Construit et applique le menu complet de l'application
/// Builds and applies the full application menu
///
/// # Arguments
/// * `app` - Handle de l'application Tauri / Tauri application handle
/// * `locale` - Locale courante / Current locale
fn rebuild_menu(app: &tauri::AppHandle, locale: &str) -> Result<(), tauri::Error> {
    // === MENU APPLICATION (premier menu sur macOS) ===
    // === APPLICATION MENU (first menu on macOS) ===
    // Crée l'élément "À propos" avec métadonnées / Create "About" item with metadata
    let about = PredefinedMenuItem::about(
        app,
        Some(i18n::menu_t(locale, "about")), // Titre / Title
        Some(AboutMetadata {
            name: Some("CCA".to_string()),    // Nom de l'app / App name
            version: Some("1.0.0".to_string()),           // Version / Version
            copyright: Some("xxx Licence".to_string()), // Copyright / Copyright
            authors: Some(vec!["Cédric Trévisan".to_string()]), // Auteurs / Authors
            ..Default::default()                          // Autres champs par défaut / Other fields default
        }),
    )?;

    // Élément Settings avec raccourci Cmd+, / Settings item with Cmd+, shortcut
    let settings_item = MenuItemBuilder::with_id("settings", i18n::menu_t(locale, "settings"))
        .accelerator("CmdOrCtrl+,")
        .build(app)?;

    // Éléments standards du menu Application / Standard Application menu items
    let separator1 = PredefinedMenuItem::separator(app)?;
    let hide = PredefinedMenuItem::hide(app, Some(i18n::menu_t(locale, "hide")))?;
    let hide_others = PredefinedMenuItem::hide_others(app, Some(i18n::menu_t(locale, "hide_others")))?;
    let show_all = PredefinedMenuItem::show_all(app, Some(i18n::menu_t(locale, "show_all")))?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let quit = PredefinedMenuItem::quit(app, Some(i18n::menu_t(locale, "quit")))?;

    // === SOUS-MENU LANGUAGE ===
    // === LANGUAGE SUBMENU ===
    let lang_en = CheckMenuItemBuilder::with_id("lang_en", "English")
        .checked(locale == "en")
        .build(app)?;
    let lang_fr = CheckMenuItemBuilder::with_id("lang_fr", "Français")
        .checked(locale == "fr")
        .build(app)?;

    let language_submenu = SubmenuBuilder::new(app, i18n::menu_t(locale, "language"))
        .item(&lang_en)
        .item(&lang_fr)
        .build()?;

    // Construit le sous-menu Application / Build Application submenu
    let app_menu = Submenu::with_items(
        app,
        "CCA",  // Nom affiché dans la barre de menu / Name shown in menu bar
        true,       // Activé / Enabled
        &[
            &about,           // À propos / About
            &PredefinedMenuItem::separator(app)?,
            &settings_item,   // Settings… / Préférences…
            &separator1,      // --- / ---
            &hide,            // Masquer / Hide
            &hide_others, // Masquer les autres / Hide others
            &show_all,    // Tout afficher / Show all
            &separator2,  // --- / ---
            &language_submenu, // Language
            &PredefinedMenuItem::separator(app)?,
            &quit,        // Quitter / Quit
        ],
    )?;

    // === MENU ÉDITION ===
    // === EDIT MENU ===
    let edit_undo = PredefinedMenuItem::undo(app, None)?;
    let edit_redo = PredefinedMenuItem::redo(app, None)?;
    let edit_sep1 = PredefinedMenuItem::separator(app)?;
    let edit_cut = PredefinedMenuItem::cut(app, None)?;
    let edit_copy = PredefinedMenuItem::copy(app, None)?;
    let edit_paste = PredefinedMenuItem::paste(app, None)?;
    let edit_select_all = PredefinedMenuItem::select_all(app, None)?;

    let mut edit_builder = SubmenuBuilder::new(app, i18n::menu_t(locale, "edit"))
        .item(&edit_undo)
        .item(&edit_redo)
        .item(&edit_sep1)
        .item(&edit_cut)
        .item(&edit_copy)
        .item(&edit_paste)
        .item(&edit_select_all);

    // Ajoute les modèles de copie avec leurs raccourcis
    // Add copy templates with their shortcuts
    let state = app.state::<store::AppState>();
    let templates = state.templates.lock().unwrap().clone();

    if !templates.is_empty() {
        let tpl_sep = PredefinedMenuItem::separator(app)?;
        edit_builder = edit_builder.item(&tpl_sep);

        let mut tpl_submenu_builder = SubmenuBuilder::new(app, i18n::menu_t(locale, "copy_templates"));

        for (i, tpl) in templates.iter().enumerate() {
            let menu_id = format!("copy_template_{}", i);
            let name = if tpl.name.is_empty() { format!("Template {}", i + 1) } else { tpl.name.clone() };

            let item = if !tpl.shortcut.is_empty() {
                match MenuItemBuilder::with_id(&menu_id, &name)
                    .accelerator(&tpl.shortcut)
                    .build(app) {
                    Ok(item) => item,
                    Err(_) => MenuItemBuilder::with_id(&menu_id, &name).build(app)?,
                }
            } else {
                MenuItemBuilder::with_id(&menu_id, &name).build(app)?
            };

            tpl_submenu_builder = tpl_submenu_builder.item(&item);
        }

        let tpl_submenu = tpl_submenu_builder.build()?;
        edit_builder = edit_builder.item(&tpl_submenu);
    }

    let edit_submenu = edit_builder.build()?;

    #[cfg(target_os = "macos")]
    {
        // Crée le sous-menu ICC avec les profils
        // Create the ICC submenu with profiles
        let icc_submenu = create_icc_submenu(app, locale)?;

        // Crée le menu de l'application
        // Get the application menu
        let root_menu = Menu::with_items(app, &[
            &app_menu,
            &edit_submenu,
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
        let root_menu = Menu::with_items(app, &[
            &app_menu,
            &edit_submenu,
        ])?;
        // Applique le menu à l'application
        // Apply menu to the application
        app.set_menu(root_menu)?;
    }

    Ok(())
}

/// Commande Tauri pour mettre à jour les modèles de copie depuis le frontend
/// Tauri command to update copy templates from frontend
#[tauri::command]
fn set_copy_templates(app: tauri::AppHandle, state: tauri::State<store::AppState>, templates: Vec<store::CopyTemplate>) {
    {
        let mut tpls = state.templates.lock().unwrap();
        *tpls = templates;
    }
    let locale = state.locale.lock().unwrap().clone();
    let _ = rebuild_menu(&app, &locale);
}

/// Commande Tauri pour changer la locale depuis le frontend
/// Tauri command to change locale from frontend
#[tauri::command]
fn set_locale(app: tauri::AppHandle, state: tauri::State<store::AppState>, locale: String) {
    // Met à jour la locale dans l'état
    // Update locale in state
    {
        let mut current_locale = state.locale.lock().unwrap();
        if *current_locale == locale {
            return;
        }
        *current_locale = locale.clone();
    }

    // Reconstruit le menu avec la nouvelle locale
    // Rebuild menu with new locale
    let _ = rebuild_menu(&app, &locale);

    // Émet l'événement pour notifier toutes les fenêtres
    // Emit event to notify all windows
    let _ = app.emit("locale-changed", &locale);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Initialise le plugin OS pour la détection de locale
        // Initialize OS plugin for locale detection
        .plugin(tauri_plugin_os::init())
        // Initialise l'état global du color store
        // Initialize global color store state
        .manage(store::AppState {
            store: Mutex::new(store::ResultStore::default()),
            locale: Mutex::new("en".to_string()),
            templates: Mutex::new(Vec::new()),
        })
        // Configure le menu de l'application
        // Configure the application menu
        .setup(|app| {
            // Récupère le handle de l'application
            // Get the application handle
            let handle = app.handle();

            // Construit le menu initial avec la locale par défaut
            // Build initial menu with default locale
            rebuild_menu(handle, "en")?;

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

            // === Gestion du changement de langue ===
            // === Language change handling ===
            // Gestion des modèles de copie
            // Copy template handling
            if menu_id.starts_with("copy_template_") {
                if let Ok(index) = menu_id["copy_template_".len()..].parse::<usize>() {
                    let _ = app.emit("copy-template", index);
                }
                return;
            }

            match menu_id {
                "settings" => {
                    // Ouvre ou focus la fenêtre Settings
                    // Open or focus the Settings window
                    if let Some(window) = app.get_webview_window("settings") {
                        let _ = window.set_focus();
                    } else {
                        let settings_title = {
                            let state = app.state::<store::AppState>();
                            let locale = state.locale.lock().unwrap();
                            i18n::menu_t(&locale, "settings_title")
                        };
                        let _ = WebviewWindowBuilder::new(
                            app,
                            "settings",
                            WebviewUrl::App("settings.html".into()),
                        )
                        .title(settings_title)
                        .inner_size(500.0, 450.0)
                        .resizable(true)
                        .min_inner_size(400.0, 700.0)
                        .center()
                        .build();
                    }
                    return;
                }
                "lang_en" | "lang_fr" => {
                    let new_locale = if menu_id == "lang_en" { "en" } else { "fr" };

                    // Met à jour la locale dans l'état
                    // Update locale in state
                    let state = app.state::<store::AppState>();
                    {
                        let mut locale = state.locale.lock().unwrap();
                        *locale = new_locale.to_string();
                    }

                    // Reconstruit le menu avec la nouvelle locale
                    // Rebuild menu with new locale
                    let _ = rebuild_menu(app, new_locale);

                    // Émet l'événement pour notifier le frontend
                    // Emit event to notify frontend
                    let _ = app.emit("locale-changed", new_locale);

                    return;
                }
                _ => {}
            }

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
            set_locale,
            set_copy_templates,
        ])
        // Lance l'application Tauri
        // Run the Tauri application
        .run(tauri::generate_context!())
        // Affiche un message d'erreur si le lancement échoue
        // Display error message if launch fails
        .expect("error while running tauri application");
}
