//! =============================================================================
//! MACOS.RS - Implémentation macOS du Color Picker
//! =============================================================================
//!
//! Ce module contient tout le code spécifique à macOS utilisant Cocoa et Core Graphics.
//! Il crée une fenêtre overlay plein écran qui capture l'écran et affiche
//! une vue agrandie des pixels autour du curseur.
//!
//! # Architecture
//! - ColorPickerView: Vue personnalisée NSView qui gère le dessin et les événements
//! - KeyableWindow: Fenêtre personnalisée qui peut devenir la fenêtre clé
//! - État global: Mutex pour partager les données entre les callbacks
//!
//! # Flux d'exécution
//! 1. run() crée l'application et les fenêtres overlay
//! 2. Les événements souris/clavier sont capturés par ColorPickerView
//! 3. À chaque mouvement, la couleur est extraite et la vue redessinée
//! 4. Clic ou Entrée termine l'application et retourne la couleur

// =============================================================================
// IMPORTS
// =============================================================================

// -----------------------------------------------------------------------------
// Bindings Objective-C legacy (objc crate)
// -----------------------------------------------------------------------------
// Utilisé pour msg_send! là où objc2 ne suffit pas encore
use objc::{class, msg_send, sel, sel_impl}; // Macros pour appeler des méthodes Objective-C
use objc::runtime::Object;                   // Type Object requis par msg_send! legacy

// -----------------------------------------------------------------------------
// Bindings Objective-C modernes (objc2 crate)
// -----------------------------------------------------------------------------
// API moderne et type-safe pour déclarer des classes Objective-C en Rust
use objc2::{declare_class, mutability, ClassType, DeclaredClass}; // Macros de déclaration de classe
use objc2::rc::Retained;                                           // Smart pointer pour objets ObjC

// Types Foundation (équivalent de la bibliothèque standard ObjC)
use objc2_foundation::{
    MainThreadMarker,    // Marqueur pour garantir l'exécution sur le thread principal
    NSAffineTransform,   // Transformations 2D (rotation, translation, échelle)
    NSCopying,           // Protocole de copie
    NSPoint,             // Point 2D (x, y)
    NSRect,              // Rectangle (origin + size)
    NSSize,              // Taille 2D (width, height)
    NSString,            // Chaîne de caractères Objective-C
};

// Types AppKit (framework UI de macOS)
use objc2_app_kit::{
    NSAffineTransformNSAppKitAdditions, // Extensions AppKit pour NSAffineTransform
    NSApplication,                       // Application principale
    NSApplicationActivationPolicy,       // Politique d'activation (Regular, Accessory, etc.)
    NSBezierPath,                        // Chemins vectoriels pour le dessin
    NSColor,                             // Couleurs
    NSCursor,                            // Curseur de la souris
    NSEvent,                             // Événements (souris, clavier, etc.)
    NSEventModifierFlags,                // Modificateurs (Shift, Ctrl, etc.)
    NSGraphicsContext,                   // Contexte de dessin
    NSStringDrawing,                     // Extension pour dessiner du texte
    NSView,                              // Vue de base
    NSWindow as NSWindow2,               // Fenêtre (renommée pour éviter conflit)
    NSWindowStyleMask,                   // Styles de fenêtre (Borderless, etc.)
};

// -----------------------------------------------------------------------------
// Core Graphics (capture d'écran et manipulation d'images)
// -----------------------------------------------------------------------------
use core_graphics::display::CGDisplay; // Accès aux écrans
use core_graphics::image::CGImage;     // Images bitmap

// -----------------------------------------------------------------------------
// Bibliothèque standard Rust
// -----------------------------------------------------------------------------
use std::sync::Mutex; // Mutex pour synchronisation thread-safe

// -----------------------------------------------------------------------------
// Configuration partagée
// -----------------------------------------------------------------------------
// Importe toutes les constantes du module config
use crate::config::*;

// =============================================================================
// ALIAS DE TYPES ET CONSTANTES
// =============================================================================

/// Alias pour un pointeur vers un objet Objective-C (version legacy)
/// Utilisé avec msg_send! de la crate objc
type Id = *mut Object;

/// Type AnyObject de objc2 pour les APIs objc2 modernes
/// Utilisé pour les casts vers les classes objc2
use objc2::runtime::AnyObject;

/// Type Bool de objc2 pour les booléens Objective-C
/// Remplace objc::runtime::BOOL qui est moins type-safe
use objc2::runtime::Bool;

// =============================================================================
// CLASSES PERSONNALISÉES OBJECTIVE-C
// =============================================================================

// -----------------------------------------------------------------------------
// ColorPickerView - Vue personnalisée pour le color picker
// -----------------------------------------------------------------------------

/// Variables d'instance pour ColorPickerView
/// Ici vide car on utilise l'état global via Mutex
pub struct ColorPickerViewIvars;

// Macro pour déclarer une classe Objective-C en Rust
declare_class!(
    /// Vue personnalisée qui gère tout le rendu et les événements du color picker
    pub struct ColorPickerView;

    // SAFETY: ColorPickerView n'utilise que des références immuables
    // et est limité au thread principal (MainThreadOnly)
    unsafe impl ClassType for ColorPickerView {
        type Super = NSView;                        // Hérite de NSView
        type Mutability = mutability::MainThreadOnly; // Utilisable uniquement sur le main thread
        const NAME: &'static str = "ColorPickerView"; // Nom de la classe ObjC
    }

    // Déclare les variables d'instance (ivars)
    impl DeclaredClass for ColorPickerView {
        type Ivars = ColorPickerViewIvars;
    }

    // Implémentation des méthodes Objective-C
    unsafe impl ColorPickerView {
        // ---------------------------------------------------------------------
        // acceptsFirstResponder - Permet à la vue de recevoir les événements clavier
        // ---------------------------------------------------------------------
        /// Indique que cette vue peut devenir le "first responder"
        /// Nécessaire pour recevoir les événements clavier
        #[method(acceptsFirstResponder)]
        fn accepts_first_responder(&self) -> bool {
            true // Oui, cette vue accepte d'être le premier répondeur
        }

        // ---------------------------------------------------------------------
        // mouseDown: - Gère les clics de souris
        // ---------------------------------------------------------------------
        /// Appelé quand l'utilisateur clique avec la souris
        /// Sauvegarde la couleur actuelle et termine l'application
        #[method(mouseDown:)]
        fn mouse_down(&self, _event: &NSEvent) {
            // Verrouille le mutex pour accéder à l'état de la souris
            if let Ok(state) = MOUSE_STATE.lock() {
                // Si on a des informations sur la couleur actuelle
                if let Some(ref info) = *state {
                    // Verrouille le mutex de la couleur sélectionnée
                    if let Ok(mut selected) = SELECTED_COLOR.lock() {
                        // Sauvegarde la couleur RGB actuelle
                        *selected = Some((info.r, info.g, info.b));
                    }
                }
            }
            // Arrête l'application
            stop_application();
        }

        // ---------------------------------------------------------------------
        // mouseMoved: - Gère les mouvements de souris
        // ---------------------------------------------------------------------
        /// Appelé quand la souris se déplace
        /// Met à jour la position et la couleur, puis redessine
        #[method(mouseMoved:)]
        fn mouse_moved(&self, event: &NSEvent) {
            // Récupère la position de la souris dans les coordonnées de la fenêtre
            let location: NSPoint = unsafe { event.locationInWindow() };

            // Récupère la fenêtre parente de cette vue
            let window_opt: Option<Retained<NSWindow2>> = self.window();

            // Si on a une fenêtre valide
            if let Some(window) = window_opt {
                // Convertit les coordonnées fenêtre en coordonnées écran
                let screen_location: NSPoint = unsafe { window.convertPointToScreen(location) };

                // Récupère la couleur du pixel à la position du curseur
                if let Some((r, g, b)) = get_pixel_color(screen_location.x, screen_location.y) {
                    // Convertit les valeurs flottantes [0.0-1.0] en entiers [0-255]
                    let r_int = (r * 255.0) as u8;
                    let g_int = (g * 255.0) as u8;
                    let b_int = (b * 255.0) as u8;

                    // Formate la couleur en hexadécimal (#RRGGBB)
                    let hex_color = format!("#{:02X}{:02X}{:02X}", r_int, g_int, b_int);

                    // Met à jour l'état global
                    if let Ok(mut state) = MOUSE_STATE.lock() {
                        // Récupère le facteur d'échelle de l'écran (pour Retina)
                        let scale_factor: f64 = if let Some(screen) = window.screen() {
                            screen.backingScaleFactor() // 2.0 pour Retina, 1.0 sinon
                        } else {
                            1.0 // Valeur par défaut si pas d'écran
                        };

                        // Crée la nouvelle structure d'état
                        *state = Some(MouseColorInfo {
                            x: location.x,           // Position X dans la fenêtre
                            y: location.y,           // Position Y dans la fenêtre
                            screen_x: screen_location.x, // Position X sur l'écran
                            screen_y: screen_location.y, // Position Y sur l'écran
                            r: r_int,                // Composante rouge [0-255]
                            g: g_int,                // Composante verte [0-255]
                            b: b_int,                // Composante bleue [0-255]
                            hex_color: hex_color.clone(), // Code hex "#RRGGBB"
                            scale_factor,            // Facteur d'échelle Retina
                        });
                    }

                    // Demande un rafraîchissement de l'affichage
                    unsafe { self.setNeedsDisplay(true) };
                }
            }
        }

        // ---------------------------------------------------------------------
        // scrollWheel: - Gère la molette de défilement
        // ---------------------------------------------------------------------
        /// Appelé quand l'utilisateur utilise la molette de défilement
        /// Ajuste le niveau de zoom de la loupe
        #[method(scrollWheel:)]
        fn scroll_wheel(&self, event: &NSEvent) {
            // Récupère le delta vertical de la molette
            let delta_y: f64 = unsafe { event.deltaY() };

            // Si la molette a bougé
            if delta_y != 0.0 {
                // Verrouille le mutex du zoom
                if let Ok(mut zoom) = CURRENT_ZOOM.lock() {
                    // Calcule le nouveau zoom en ajoutant le delta * pas de zoom
                    let new_zoom = *zoom + delta_y * ZOOM_STEP;
                    // Limite le zoom entre ZOOM_MIN et ZOOM_MAX
                    *zoom = new_zoom.clamp(ZOOM_MIN, ZOOM_MAX);
                }

                // Demande un rafraîchissement pour afficher le nouveau zoom
                unsafe { self.setNeedsDisplay(true) };
            }
        }

        // ---------------------------------------------------------------------
        // keyDown: - Gère les touches du clavier
        // ---------------------------------------------------------------------
        /// Appelé quand une touche est pressée
        /// Gère ESC (annuler), Entrée (confirmer), et flèches (déplacer)
        #[method(keyDown:)]
        fn key_down(&self, event: &NSEvent) {
            // Récupère le code de la touche pressée
            let key_code: u16 = unsafe { event.keyCode() };
            // Récupère les modificateurs (Shift, Ctrl, etc.)
            let modifier_flags: NSEventModifierFlags = unsafe { event.modifierFlags() };

            // Vérifie si Shift est pressé
            let shift_pressed = modifier_flags.contains(NSEventModifierFlags::NSEventModifierFlagShift);
            // Détermine la distance de déplacement (50px si Shift, 1px sinon)
            let move_amount = if shift_pressed { SHIFT_MOVE_PIXELS } else { 1.0 };

            // Codes des touches: ESC = 53, Enter/Return = 36
            if key_code == 53 {
                // ESC - Annule la sélection
                stop_application();
            } else if key_code == 36 {
                // Enter - Confirme la sélection
                if let Ok(state) = MOUSE_STATE.lock() {
                    if let Some(ref info) = *state {
                        if let Ok(mut selected) = SELECTED_COLOR.lock() {
                            // Sauvegarde la couleur actuelle
                            *selected = Some((info.r, info.g, info.b));
                        }
                    }
                }
                stop_application();
            } else {
                // Codes des touches fléchées: gauche=123, droite=124, bas=125, haut=126
                let (dx, dy): (f64, f64) = match key_code {
                    123 => (-move_amount, 0.0),  // Gauche: déplace vers la gauche
                    124 => (move_amount, 0.0),   // Droite: déplace vers la droite
                    125 => (0.0, -move_amount),  // Bas: déplace vers le bas
                    126 => (0.0, move_amount),   // Haut: déplace vers le haut
                    _ => (0.0, 0.0),             // Autre touche: pas de déplacement
                };

                // Si un déplacement est demandé
                if dx != 0.0 || dy != 0.0 {
                    // Déplace le curseur et met à jour l'état
                    if let Ok(state) = MOUSE_STATE.lock() {
                        if let Some(ref info) = *state {
                            // Calcule la nouvelle position
                            let new_x = info.screen_x + dx;
                            let new_y = info.screen_y + dy;

                            // Récupère la hauteur de l'écran pour la conversion de coordonnées
                            let main_display = CGDisplay::main();
                            let screen_height = main_display.pixels_high() as f64;

                            // Convertit les coordonnées Cocoa en coordonnées Core Graphics
                            // Cocoa: origine en bas à gauche
                            // Core Graphics: origine en haut à gauche
                            let cg_y = screen_height - new_y;

                            // Déplace le curseur de la souris à la nouvelle position
                            let _ = CGDisplay::warp_mouse_cursor_position(
                                core_graphics::geometry::CGPoint::new(new_x, cg_y)
                            );

                            // Libère le verrou avant de récupérer la nouvelle couleur
                            drop(state);

                            // Récupère la couleur à la nouvelle position
                            if let Some((r, g, b)) = get_pixel_color(new_x, new_y) {
                                let r_int = (r * 255.0) as u8;
                                let g_int = (g * 255.0) as u8;
                                let b_int = (b * 255.0) as u8;

                                let hex_color = format!("#{:02X}{:02X}{:02X}", r_int, g_int, b_int);

                                // Met à jour l'état avec la nouvelle position et couleur
                                if let Ok(mut state) = MOUSE_STATE.lock() {
                                    if let Some(window) = self.window() {
                                        // Convertit les coordonnées écran en coordonnées fenêtre
                                        let screen_point = NSPoint::new(new_x, new_y);
                                        let window_point: NSPoint = window.convertPointFromScreen(screen_point);

                                        // Récupère le facteur d'échelle
                                        let scale_factor: f64 = if let Some(screen) = window.screen() {
                                            screen.backingScaleFactor()
                                        } else {
                                            1.0
                                        };

                                        // Met à jour l'état
                                        *state = Some(MouseColorInfo {
                                            x: window_point.x,
                                            y: window_point.y,
                                            screen_x: new_x,
                                            screen_y: new_y,
                                            r: r_int,
                                            g: g_int,
                                            b: b_int,
                                            hex_color,
                                            scale_factor,
                                        });
                                    }
                                }

                                // Demande un rafraîchissement
                                unsafe { self.setNeedsDisplay(true) };
                            }
                        }
                    }
                }
            }
        }

        // ---------------------------------------------------------------------
        // drawRect: - Dessine le contenu de la vue
        // ---------------------------------------------------------------------
        /// Appelé par le système quand la vue doit être redessinée
        /// Délègue à la fonction draw_view()
        #[method(drawRect:)]
        fn draw_rect(&self, _rect: NSRect) {
            // Appelle la fonction de dessin principale
            draw_view(self);
        }
    }
);

// -----------------------------------------------------------------------------
// KeyableWindow - Fenêtre qui peut recevoir les événements clavier
// -----------------------------------------------------------------------------

/// Variables d'instance pour KeyableWindow (aucune nécessaire)
pub struct KeyableWindowIvars;

declare_class!(
    /// Fenêtre personnalisée qui peut devenir la fenêtre clé
    /// Par défaut, les fenêtres borderless ne peuvent pas devenir key window
    pub struct KeyableWindow;

    // SAFETY: KeyableWindow n'utilise que des références immuables
    unsafe impl ClassType for KeyableWindow {
        type Super = NSWindow2;                      // Hérite de NSWindow
        type Mutability = mutability::MainThreadOnly; // Main thread only
        const NAME: &'static str = "KeyableWindow";   // Nom de la classe
    }

    impl DeclaredClass for KeyableWindow {
        type Ivars = KeyableWindowIvars;
    }

    unsafe impl KeyableWindow {
        // Surcharge canBecomeKeyWindow pour retourner true
        // Permet à cette fenêtre sans bordure de recevoir les événements clavier
        #[method(canBecomeKeyWindow)]
        fn can_become_key_window(&self) -> bool {
            true // Oui, cette fenêtre peut devenir la fenêtre clé
        }
    }
);

// =============================================================================
// ÉTAT GLOBAL
// =============================================================================

/// État global protégé par Mutex pour la position de la souris et la couleur
/// Mutex permet un accès thread-safe depuis les différents callbacks
static MOUSE_STATE: Mutex<Option<MouseColorInfo>> = Mutex::new(None);

/// État global pour le niveau de zoom actuel
/// Initialisé avec le facteur de zoom par défaut
static CURRENT_ZOOM: Mutex<f64> = Mutex::new(INITIAL_ZOOM_FACTOR);

/// Stocke la couleur finale sélectionnée quand l'utilisateur clique ou appuie Entrée
static SELECTED_COLOR: Mutex<Option<(u8, u8, u8)>> = Mutex::new(None);

/// Structure contenant toutes les informations sur la position et la couleur actuelles
struct MouseColorInfo {
    x: f64,          // Position X dans les coordonnées de la fenêtre
    y: f64,          // Position Y dans les coordonnées de la fenêtre
    screen_x: f64,   // Position X dans les coordonnées de l'écran
    screen_y: f64,   // Position Y dans les coordonnées de l'écran
    r: u8,           // Composante rouge (0-255)
    g: u8,           // Composante verte (0-255)
    b: u8,           // Composante bleue (0-255)
    hex_color: String, // Code couleur hexadécimal (#RRGGBB)
    scale_factor: f64, // Facteur d'échelle de l'écran (2.0 pour Retina)
}

// =============================================================================
// FONCTIONS DE CAPTURE D'ÉCRAN
// =============================================================================

/// Capture une zone carrée de pixels autour des coordonnées données
///
/// # Arguments
/// * `x` - Coordonnée X du centre (coordonnées Cocoa, origine en bas à gauche)
/// * `y` - Coordonnée Y du centre
/// * `size` - Taille du carré à capturer (en pixels)
///
/// # Retourne
/// * `Some(CGImage)` - L'image capturée si la capture a réussi
/// * `None` - Si la capture a échoué
fn capture_zoom_area(x: f64, y: f64, size: f64) -> Option<CGImage> {
    // Importe les types géométriques de Core Graphics
    use core_graphics::geometry::{CGRect, CGPoint as CGPointStruct, CGSize};

    // Récupère l'écran principal
    let main_display = CGDisplay::main();
    // Récupère la hauteur de l'écran en pixels
    let screen_height = main_display.pixels_high() as f64;
    // Convertit Y de Cocoa (bas) vers Core Graphics (haut)
    let cg_y = screen_height - y;

    // Arrondit les coordonnées pour éviter les artefacts de sous-pixel
    let center_x = x.round();
    let center_y = cg_y.round();
    let capture_size = size.round();
    // Calcule la moitié de la taille pour centrer le rectangle
    let half_size = (capture_size / 2.0).floor();

    // Crée le rectangle de capture centré sur le point
    let rect = CGRect::new(
        &CGPointStruct::new(center_x - half_size, center_y - half_size),
        &CGSize::new(capture_size, capture_size)
    );

    // Capture l'image dans le rectangle spécifié
    main_display.image_for_rect(rect)
}

/// Capture la couleur d'un seul pixel aux coordonnées données
///
/// # Arguments
/// * `x` - Coordonnée X (coordonnées Cocoa)
/// * `y` - Coordonnée Y (coordonnées Cocoa)
///
/// # Retourne
/// * `Some((r, g, b))` - Les composantes RGB normalisées [0.0-1.0]
/// * `None` - Si la capture a échoué
fn get_pixel_color(x: f64, y: f64) -> Option<(f64, f64, f64)> {
    use core_graphics::geometry::{CGRect, CGPoint as CGPointStruct, CGSize};

    // Récupère l'écran principal et sa hauteur
    let main_display = CGDisplay::main();
    let screen_height = main_display.pixels_high() as f64;
    // Convertit les coordonnées
    let cg_y = screen_height - y;

    // Arrondit pour cibler un pixel exact
    let center_x = x.round();
    let center_y = cg_y.round();

    // Crée un rectangle de 1x1 pixel
    let rect = CGRect::new(
        &CGPointStruct::new(center_x, center_y),
        &CGSize::new(1.0, 1.0)
    );

    // Capture l'image du pixel
    let image = main_display.image_for_rect(rect)?;
    // Récupère les données brutes de l'image
    let data = image.data();
    let data_len = data.len() as usize;

    // Vérifie qu'on a au moins 4 octets (BGRA)
    if data_len >= 4 {
        // Les données sont en format BGRA (Blue, Green, Red, Alpha)
        let b = data[0] as f64 / 255.0; // Bleu normalisé
        let g = data[1] as f64 / 255.0; // Vert normalisé
        let r = data[2] as f64 / 255.0; // Rouge normalisé
        Some((r, g, b)) // Retourne en ordre RGB
    } else {
        None // Pas assez de données
    }
}

// =============================================================================
// API PUBLIQUE
// =============================================================================

/// Fonction helper pour arrêter l'application et réafficher le curseur
fn stop_application() {
    // Réaffiche le curseur de la souris
    unsafe {
        NSCursor::unhide();
    }

    // Récupère le marqueur de thread principal
    if let Some(mtm) = MainThreadMarker::new() {
        // Récupère l'instance partagée de l'application
        let app = NSApplication::sharedApplication(mtm);
        // Arrête la boucle d'événements
        app.stop(None);

        // Crée un événement factice pour forcer la sortie de la boucle run
        // Sans cela, stop() ne prend effet qu'au prochain événement
        unsafe {
            use objc2_app_kit::NSEventType;

            // Crée un événement de type ApplicationDefined
            let dummy_event = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
                NSEventType::ApplicationDefined, // Type d'événement
                NSPoint::new(0.0, 0.0),          // Position (ignorée)
                NSEventModifierFlags::empty(),   // Pas de modificateurs
                0.0,                             // Timestamp
                0,                               // Numéro de fenêtre
                None,                            // Contexte graphique
                0,                               // Sous-type
                0,                               // Data1
                0                                // Data2
            );

            // Poste l'événement en tête de la queue
            if let Some(event) = dummy_event {
                app.postEvent_atStart(&event, true);
            }
        }
    }
}

/// Exécute l'application color picker sur macOS
///
/// # Retourne
/// * `Some((r, g, b))` - La couleur RGB sélectionnée si l'utilisateur a cliqué ou appuyé Entrée
/// * `None` - Si l'utilisateur a appuyé ESC pour annuler
pub fn run() -> Option<(u8, u8, u8)> {
    // Réinitialise la couleur sélectionnée
    if let Ok(mut color) = SELECTED_COLOR.lock() {
        *color = None;
    }

    // Récupère le marqueur de thread principal - requis pour les opérations UI
    let mtm = MainThreadMarker::new().expect("Must be called from main thread");

    // Récupère l'instance partagée de l'application
    let app = NSApplication::sharedApplication(mtm);

    // Configure la politique d'activation sur "Regular"
    // L'app apparaît dans le dock et peut recevoir le focus
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    // Crée des fenêtres overlay pour chaque écran
    unsafe {
        // Récupère la liste de tous les écrans
        let screens: Id = msg_send![class!(NSScreen), screens];
        let count: usize = msg_send![screens, count];

        // Pour chaque écran
        for i in 0..count {
            // Récupère l'écran à l'index i
            let screen: Id = msg_send![screens, objectAtIndex: i];
            // Récupère les dimensions de l'écran
            let frame: NSRect = msg_send![screen, frame];

            // Crée une fenêtre KeyableWindow via msg_send (car objc2 ne supporte pas initWithContentRect)
            // Convertit la classe objc2 en pointeur raw pour msg_send!
            let window_cls = KeyableWindow::class() as *const objc2::runtime::AnyClass as *const Object;
            let window_alloc: Id = msg_send![window_cls, alloc];
            let window: Id = msg_send![window_alloc,
                initWithContentRect: frame                    // Rectangle de la fenêtre
                styleMask: NSWindowStyleMask::Borderless      // Pas de bordure
                backing: 2u64                                 // NSBackingStoreBuffered
                defer: Bool::NO                                // Ne pas différer la création
            ];

            // Convertit en référence objc2 pour les appels de méthodes
            let window_ref: &KeyableWindow = &*(window as *const KeyableWindow);

            // Configure la fenêtre
            window_ref.setLevel(1000);                        // Niveau très élevé (au-dessus de tout)

            let clear_color = NSColor::clearColor();          // Couleur transparente
            window_ref.setBackgroundColor(Some(&clear_color)); // Fond transparent

            window_ref.setOpaque(false);                      // Non opaque
            window_ref.setHasShadow(false);                   // Pas d'ombre
            window_ref.setIgnoresMouseEvents(false);          // Reçoit les événements souris
            window_ref.setAcceptsMouseMovedEvents(true);      // Reçoit mouseMoved
            let _: () = msg_send![window, setSharingType: 0u64]; // Pas de partage d'écran

            // Crée la vue ColorPickerView
            let view_cls = ColorPickerView::class() as *const objc2::runtime::AnyClass as *const Object;
            let view_alloc: Id = msg_send![view_cls, alloc];
            let view: Id = msg_send![view_alloc, initWithFrame: frame];

            let view_ref: &ColorPickerView = &*(view as *const ColorPickerView);

            // Configure la fenêtre avec la vue
            window_ref.setContentView(Some(view_ref));        // Définit la vue de contenu
            window_ref.makeKeyAndOrderFront(None);            // Affiche et met au premier plan
            window_ref.makeFirstResponder(Some(view_ref));    // La vue reçoit les événements
        }
    }

    // Active l'application
    unsafe {
        let running_app: Id = msg_send![class!(NSRunningApplication), currentApplication];
        let _: () = msg_send![running_app, activateWithOptions: 0u64];
    }

    // Cache le curseur de la souris
    unsafe {
        NSCursor::hide();
    }

    // Lance la boucle d'événements (bloque jusqu'à stop())
    unsafe {
        app.run();
    }

    // Retourne la couleur sélectionnée (si elle existe)
    if let Ok(color) = SELECTED_COLOR.lock() {
        color.clone()
    } else {
        None
    }
}

// =============================================================================
// DESSIN
// =============================================================================

/// Fonction principale de dessin appelée depuis drawRect de ColorPickerView
///
/// Cette fonction dessine:
/// 1. Un overlay semi-transparent sur tout l'écran
/// 2. La loupe circulaire avec les pixels agrandis
/// 3. Le réticule central
/// 4. La bordure colorée
/// 5. Le texte hexadécimal en arc
fn draw_view(view: &NSView) {
    // -------------------------------------------------------------------------
    // Dessine l'overlay semi-transparent
    // -------------------------------------------------------------------------
    // Crée une couleur noire avec 5% d'opacité
    let overlay_color = unsafe { NSColor::colorWithCalibratedWhite_alpha(0.0, 0.05) };
    // Définit comme couleur de remplissage
    unsafe { overlay_color.set() };

    // Récupère les limites de la vue
    let bounds: NSRect = view.bounds();
    // Crée un chemin rectangulaire couvrant toute la vue
    let bounds_path = unsafe { NSBezierPath::bezierPathWithRect(bounds) };
    // Remplit avec la couleur overlay
    unsafe { bounds_path.fill() };

    // -------------------------------------------------------------------------
    // Dessine la loupe si on a des informations sur la souris
    // -------------------------------------------------------------------------
    if let Ok(state) = MOUSE_STATE.lock() {
        if let Some(ref info) = *state {
            // Récupère le zoom actuel
            let current_zoom = match CURRENT_ZOOM.lock() {
                Ok(z) => *z,
                Err(_) => INITIAL_ZOOM_FACTOR,
            };

            // Calcule la taille de la loupe à afficher
            // mag_size = nombre de pixels capturés × facteur de zoom
            let mag_size = CAPTURED_PIXELS * current_zoom;
            // Taille de capture ajustée pour le facteur d'échelle Retina
            let capture_size = CAPTURED_PIXELS / info.scale_factor;

            // Capture la zone de pixels autour du curseur
            if let Some(cg_image) = capture_zoom_area(info.screen_x, info.screen_y, capture_size) {
                // Dimensions de l'image capturée
                let img_width = cg_image.width() as f64;
                let img_height = cg_image.height() as f64;
                let target_pixels = CAPTURED_PIXELS;

                // Calcule le décalage pour centrer le recadrage
                let crop_x = if img_width > target_pixels {
                    ((img_width - target_pixels) / 2.0).floor()
                } else {
                    0.0
                };
                let crop_y = if img_height > target_pixels {
                    ((img_height - target_pixels) / 2.0).floor()
                } else {
                    0.0
                };

                // Taille effective à utiliser
                let use_width = if img_width > target_pixels { target_pixels } else { img_width };
                let use_height = if img_height > target_pixels { target_pixels } else { img_height };

                unsafe {
                    // -------------------------------------------------------------
                    // Crée une NSImage à partir de CGImage
                    // -------------------------------------------------------------
                    let ns_image_cls = class!(NSImage);
                    let ns_image: Id = msg_send![ns_image_cls, alloc];

                    // Convertit CGImage en pointeur raw pour msg_send
                    let cg_image_ptr = {
                        let ptr_addr = &cg_image as *const CGImage as *const *const core_graphics::sys::CGImage;
                        *ptr_addr
                    };

                    // Initialise NSImage avec CGImage
                    let full_size = NSSize::new(img_width, img_height);
                    let ns_image: Id = msg_send![ns_image, initWithCGImage:cg_image_ptr size:full_size];
                    let cropped_size = NSSize::new(use_width, use_height);

                    // Calcule la position de la loupe (centrée sur le curseur)
                    let mag_x = info.x - mag_size / 2.0;
                    let mag_y = info.y - mag_size / 2.0;

                    // Rectangle destination pour la loupe
                    let mag_rect = NSRect::new(
                        NSPoint::new(mag_x, mag_y),
                        NSSize::new(mag_size, mag_size)
                    );

                    // Crée un chemin circulaire pour le clip
                    let circular_clip = NSBezierPath::bezierPathWithOvalInRect(mag_rect);

                    // -------------------------------------------------------------
                    // Dessine l'image dans le cercle
                    // -------------------------------------------------------------
                    // Sauvegarde l'état graphique actuel
                    NSGraphicsContext::saveGraphicsState_class();

                    // Désactive l'interpolation pour un rendu pixelisé
                    if let Some(graphics_context) = NSGraphicsContext::currentContext() {
                        graphics_context.setImageInterpolation(objc2_app_kit::NSImageInterpolation::None);
                    }

                    // Applique le clip circulaire
                    circular_clip.addClip();

                    // Rectangle source dans l'image
                    let from_rect = NSRect::new(
                        NSPoint::new(crop_x, crop_y),
                        cropped_size
                    );

                    // Dessine l'image
                    // operation: 2 = NSCompositingOperationSourceOver
                    // fraction: 1.0 = opacité complète
                    let _: () = msg_send![ns_image, drawInRect:mag_rect
                                          fromRect:from_rect
                                          operation:2u64
                                          fraction:1.0f64];

                    // Restaure l'état graphique
                    NSGraphicsContext::restoreGraphicsState_class();

                    // -------------------------------------------------------------
                    // Dessine le réticule central
                    // -------------------------------------------------------------
                    let actual_pixels = use_width;
                    // Taille d'un pixel affiché dans la loupe
                    let pixel_size = mag_size / actual_pixels;

                    // Centre de la loupe
                    let center_x = mag_x + mag_size / 2.0;
                    let center_y = mag_y + mag_size / 2.0;

                    // Décalage pour les grilles paires (centre entre 4 pixels)
                    let offset = if (actual_pixels as i32) % 2 == 0 {
                        pixel_size / 2.0
                    } else {
                        0.0
                    };
                    let reticle_center_x = center_x + offset;
                    let reticle_center_y = center_y + offset;

                    // Rectangle du réticule (1 pixel)
                    let half_pixel = pixel_size / 2.0;
                    let square_rect = NSRect::new(
                        NSPoint::new(reticle_center_x - half_pixel, reticle_center_y - half_pixel),
                        NSSize::new(pixel_size, pixel_size)
                    );

                    // Couleur grise pour le réticule
                    let gray_color = NSColor::colorWithCalibratedRed_green_blue_alpha(0.5, 0.5, 0.5, 1.0);
                    gray_color.setStroke();

                    // Dessine le carré du réticule
                    let reticle_path = NSBezierPath::bezierPathWithRect(square_rect);
                    reticle_path.setLineWidth(1.0);
                    reticle_path.stroke();

                    // -------------------------------------------------------------
                    // Dessine la bordure colorée
                    // -------------------------------------------------------------
                    // Parse la couleur hex
                    let hex = &info.hex_color[1..]; // Enlève le #
                    let r_val = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f64 / 255.0;
                    let g_val = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f64 / 255.0;
                    let b_val = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f64 / 255.0;

                    // Rectangle pour la bordure (légèrement plus grand que la loupe)
                    let border_rect = NSRect::new(
                        NSPoint::new(mag_x - BORDER_WIDTH / 2.0, mag_y - BORDER_WIDTH / 2.0),
                        NSSize::new(mag_size + BORDER_WIDTH, mag_size + BORDER_WIDTH)
                    );

                    // Couleur de la bordure = couleur du pixel
                    let border_color = NSColor::colorWithCalibratedRed_green_blue_alpha(r_val, g_val, b_val, 1.0);
                    border_color.setStroke();

                    // Dessine le cercle de la bordure
                    let border_path = NSBezierPath::bezierPathWithOvalInRect(border_rect);
                    border_path.setLineWidth(BORDER_WIDTH);
                    border_path.stroke();

                    // -------------------------------------------------------------
                    // Dessine le texte hexadécimal en arc
                    // -------------------------------------------------------------
                    // Crée la police
                    let font_cls = class!(NSFont);
                    let font: Id = msg_send![font_cls, systemFontOfSize: HEX_FONT_SIZE weight: 0.62f64];

                    // Calcule la luminance pour choisir la couleur du texte
                    // Formule standard pour la luminance perçue
                    let luminance = 0.299 * r_val + 0.587 * g_val + 0.114 * b_val;

                    // Texte noir sur fond clair, blanc sur fond sombre
                    let text_color = if luminance > 0.5 {
                        NSColor::colorWithCalibratedRed_green_blue_alpha(0.0, 0.0, 0.0, 1.0)
                    } else {
                        NSColor::colorWithCalibratedRed_green_blue_alpha(1.0, 1.0, 1.0, 1.0)
                    };

                    // Texte à afficher
                    let hex_text = &info.hex_color;
                    let char_count = hex_text.len() as f64;
                    // Rayon de l'arc de texte (milieu de la bordure)
                    let radius = mag_size / 2.0 + BORDER_WIDTH / 2.0;

                    // Calcule l'angle entre chaque caractère
                    let angle_step = CHAR_SPACING_PIXELS / radius;
                    // Arc total occupé par le texte
                    let total_arc = angle_step * (char_count - 1.0);
                    // Angle de départ (centré en haut)
                    let start_angle: f64 = std::f64::consts::PI / 2.0 + total_arc / 2.0;

                    // Sauvegarde l'état graphique
                    NSGraphicsContext::saveGraphicsState_class();

                    // Dessine chaque caractère individuellement
                    for (i, c) in hex_text.chars().enumerate() {
                        // Angle pour ce caractère
                        let angle = start_angle - angle_step * (i as f64);

                        // Position sur l'arc
                        let char_x = center_x + radius * angle.cos();
                        let char_y = center_y + radius * angle.sin();

                        // Convertit le caractère en NSString
                        let char_str = c.to_string();
                        let ns_char = NSString::from_str(&char_str);

                        // Crée le dictionnaire d'attributs pour le texte
                        use objc2_foundation::NSDictionary;

                        let font_attr_key = NSString::from_str("NSFont");
                        let color_attr_key = NSString::from_str("NSColor");

                        let font_retained: Retained<AnyObject> =
                            Retained::retain(font as *mut AnyObject).unwrap();
                        let color_retained: Retained<AnyObject> =
                            Retained::cast(text_color.clone());

                        let keys: &[&NSString] = &[&font_attr_key, &color_attr_key];
                        let values: Vec<Retained<AnyObject>> = vec![font_retained, color_retained];
                        let attributes = NSDictionary::from_vec(keys, values);

                        // Mesure la taille du caractère
                        let char_size: NSSize = ns_char.sizeWithAttributes(Some(&attributes));

                        // Crée une transformation pour positionner et tourner le caractère
                        let transform = NSAffineTransform::transform();
                        // Déplace à la position sur l'arc
                        transform.translateXBy_yBy(char_x, char_y);

                        // Tourne pour suivre la tangente de l'arc
                        let rotation_angle = angle - std::f64::consts::PI / 2.0;
                        transform.rotateByRadians(rotation_angle);

                        // Applique la transformation
                        transform.concat();

                        // Dessine le caractère (décalé de sa taille pour le centrer)
                        let draw_point = NSPoint::new(-char_size.width, -char_size.height);
                        ns_char.drawAtPoint_withAttributes(draw_point, Some(&attributes));

                        // Inverse la transformation pour le prochain caractère
                        let inverse = transform.copy();
                        inverse.invert();
                        inverse.concat();
                    }

                    // Restaure l'état graphique
                    NSGraphicsContext::restoreGraphicsState_class();
                }
            }
        }
    }
}