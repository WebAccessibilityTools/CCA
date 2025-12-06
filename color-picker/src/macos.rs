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
// Bindings Objective-C modernes (objc2 crate)
// -----------------------------------------------------------------------------
// API moderne et type-safe pour déclarer des classes Objective-C en Rust
// Modern type-safe API for declaring Objective-C classes in Rust
use objc2::{define_class, msg_send, msg_send_id, ClassType, DefinedClass, MainThreadOnly}; // Class declaration macros
use objc2::rc::{Allocated, Retained};                                          // Smart pointers for ObjC objects

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
    NSApplicationActivationOptions,      // Options d'activation (ActivateAllWindows, etc.)
    NSApplicationActivationPolicy,       // Politique d'activation (Regular, Accessory, etc.)
    NSBezierPath,                        // Chemins vectoriels pour le dessin
    NSColor,                             // Couleurs
    NSCursor,                            // Curseur de la souris
    NSEvent,                             // Événements (souris, clavier, etc.)
    NSEventModifierFlags,                // Modificateurs (Shift, Ctrl, etc.)
    NSFont,                              // Polices de caractères
    NSGraphicsContext,                   // Contexte de dessin
    NSRunningApplication,                // Application en cours d'exécution
    NSScreen,                            // Écran (pour récupérer les dimensions)
    NSStringDrawing,                     // Extension pour dessiner du texte
    NSView,                              // Vue de base
    NSWindow as NSWindow2,               // Fenêtre (renommée pour éviter conflit)
    NSWindowSharingType,                 // Type de partage de fenêtre (None, ReadOnly, ReadWrite)
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

/// Type AnyObject de objc2 pour les APIs objc2 modernes
/// Utilisé pour les casts vers les classes objc2
/// AnyObject type from objc2 for modern objc2 APIs
/// Used for casts to objc2 classes
use objc2::runtime::AnyObject;

/// Type Bool de objc2 pour les booléens Objective-C
/// Remplace objc::runtime::BOOL qui est moins type-safe
/// Bool type from objc2 for Objective-C booleans
/// Replaces objc::runtime::BOOL which is less type-safe
use objc2::runtime::Bool;

// =============================================================================
// CLASSES PERSONNALISÉES OBJECTIVE-C
// =============================================================================

// -----------------------------------------------------------------------------
// ColorPickerView - Vue personnalisée pour le color picker
// -----------------------------------------------------------------------------

// Macro pour déclarer une classe Objective-C en Rust avec la nouvelle syntaxe define_class! (objc2 0.6+)
// New define_class! macro syntax for objc2 0.6+
define_class!(
    // SAFETY:
    // - The superclass NSView does not have any subclassing requirements that we violate.
    // - ColorPickerView does not implement Drop.
    #[unsafe(super = NSView)]                    // Inherit from NSView (parent class)
    #[thread_kind = MainThreadOnly]              // Can only be used on the main thread
    #[name = "ColorPickerView"]                  // Objective-C class name

    /// Vue personnalisée qui gère tout le rendu et les événements du color picker
    /// Custom view that handles all rendering and events for the color picker
    pub struct ColorPickerView;

    // Implémentation des méthodes Objective-C
    // Implementation of Objective-C methods
    impl ColorPickerView {
        // ---------------------------------------------------------------------
        // acceptsFirstResponder - Permet à la vue de recevoir les événements clavier
        // acceptsFirstResponder - Allows the view to receive keyboard events
        // ---------------------------------------------------------------------
        /// Indique que cette vue peut devenir le "first responder"
        /// Nécessaire pour recevoir les événements clavier
        /// Indicates that this view can become the "first responder"
        /// Required to receive keyboard events
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true // Yes, this view accepts being the first responder
        }

        // ---------------------------------------------------------------------
        // mouseDown: - Gère les clics de souris
        // mouseDown: - Handles mouse clicks
        // ---------------------------------------------------------------------
        /// Appelé quand l'utilisateur clique avec la souris
        /// Sauvegarde la couleur actuelle et termine l'application
        /// Called when the user clicks with the mouse
        /// Saves the current color and terminates the application
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            // Lock the mutex to access the mouse state
            if let Ok(state) = MOUSE_STATE.lock() {
                // If we have information about the current color
                if let Some(ref info) = *state {
                    // Lock the selected color mutex
                    if let Ok(mut selected) = SELECTED_COLOR.lock() {
                        // Save the current RGB color
                        *selected = Some((info.r, info.g, info.b));
                    }
                }
            }
            // Stop the application
            stop_application();
        }

        // ---------------------------------------------------------------------
        // mouseMoved: - Gère les mouvements de souris
        // mouseMoved: - Handles mouse movements
        // ---------------------------------------------------------------------
        /// Appelé quand la souris se déplace
        /// Met à jour la position et la couleur, puis redessine
        /// Called when the mouse moves
        /// Updates the position and color, then redraws
        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            // Get the mouse position in window coordinates
            let location: NSPoint = unsafe { event.locationInWindow() };

            // Get the parent window of this view
            let window_opt: Option<Retained<NSWindow2>> = self.window();

            // If we have a valid window
            if let Some(window) = window_opt {
                // Convert window coordinates to screen coordinates
                let screen_location: NSPoint = unsafe { window.convertPointToScreen(location) };

                // Get the pixel color at the cursor position
                if let Some((r, g, b)) = get_pixel_color(screen_location.x, screen_location.y) {
                    // Convert float values [0.0-1.0] to integers [0-255]
                    let r_int = (r * 255.0) as u8;
                    let g_int = (g * 255.0) as u8;
                    let b_int = (b * 255.0) as u8;

                    // Format the color in hexadecimal (#RRGGBB)
                    let hex_color = format!("#{:02X}{:02X}{:02X}", r_int, g_int, b_int);

                    // Update the global state
                    if let Ok(mut state) = MOUSE_STATE.lock() {
                        // Get the screen scale factor (for Retina)
                        let scale_factor: f64 = if let Some(screen) = window.screen() {
                            screen.backingScaleFactor() // 2.0 for Retina, 1.0 otherwise
                        } else {
                            1.0 // Default value if no screen
                        };

                        // Create the new state structure
                        *state = Some(MouseColorInfo {
                            x: location.x,           // X position in window
                            y: location.y,           // Y position in window
                            screen_x: screen_location.x, // X position on screen
                            screen_y: screen_location.y, // Y position on screen
                            r: r_int,                // Red component [0-255]
                            g: g_int,                // Green component [0-255]
                            b: b_int,                // Blue component [0-255]
                            hex_color: hex_color.clone(), // Hex code "#RRGGBB"
                            scale_factor,            // Retina scale factor
                        });
                    }

                    // Request a display refresh
                    unsafe { self.setNeedsDisplay(true) };
                }
            }
        }

        // ---------------------------------------------------------------------
        // scrollWheel: - Gère la molette de défilement
        // scrollWheel: - Handles scroll wheel
        // ---------------------------------------------------------------------
        /// Appelé quand l'utilisateur utilise la molette de défilement
        /// Ajuste le niveau de zoom de la loupe
        /// Called when the user uses the scroll wheel
        /// Adjusts the magnifier zoom level
        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            // Get the vertical delta of the scroll wheel
            let delta_y: f64 = unsafe { event.deltaY() };

            // If the wheel moved
            if delta_y != 0.0 {
                // Lock the zoom mutex
                if let Ok(mut zoom) = CURRENT_ZOOM.lock() {
                    // Calculate new zoom by adding delta * zoom step
                    let new_zoom = *zoom + delta_y * ZOOM_STEP;
                    // Clamp zoom between ZOOM_MIN and ZOOM_MAX
                    *zoom = new_zoom.clamp(ZOOM_MIN, ZOOM_MAX);
                }

                // Request a refresh to display the new zoom
                unsafe { self.setNeedsDisplay(true) };
            }
        }

        // ---------------------------------------------------------------------
        // keyDown: - Gère les touches du clavier
        // keyDown: - Handles keyboard keys
        // ---------------------------------------------------------------------
        /// Appelé quand une touche est pressée
        /// Gère ESC (annuler), Entrée (confirmer), et flèches (déplacer)
        /// Called when a key is pressed
        /// Handles ESC (cancel), Enter (confirm), and arrows (move)
        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            // Get the key code of the pressed key
            let key_code: u16 = unsafe { event.keyCode() };
            // Get the modifiers (Shift, Ctrl, etc.)
            let modifier_flags: NSEventModifierFlags = unsafe { event.modifierFlags() };

            // Check if Shift is pressed
            // In objc2-app-kit 0.3, the constant is NSEventModifierFlags::Shift
            let shift_pressed = modifier_flags.contains(NSEventModifierFlags::Shift);
            // Determine movement distance (50px if Shift, 1px otherwise)
            let move_amount = if shift_pressed { SHIFT_MOVE_PIXELS } else { 1.0 };

            // Key codes: ESC = 53, Enter/Return = 36
            if key_code == 53 {
                // ESC - Cancel the selection
                stop_application();
            } else if key_code == 36 {
                // Enter - Confirm the selection
                if let Ok(state) = MOUSE_STATE.lock() {
                    if let Some(ref info) = *state {
                        if let Ok(mut selected) = SELECTED_COLOR.lock() {
                            // Save the current color
                            *selected = Some((info.r, info.g, info.b));
                        }
                    }
                }
                stop_application();
            } else {
                // Arrow key codes: left=123, right=124, down=125, up=126
                let (dx, dy): (f64, f64) = match key_code {
                    123 => (-move_amount, 0.0),  // Left: move left
                    124 => (move_amount, 0.0),   // Right: move right
                    125 => (0.0, -move_amount),  // Down: move down
                    126 => (0.0, move_amount),   // Up: move up
                    _ => (0.0, 0.0),             // Other key: no movement
                };

                // If movement is requested
                if dx != 0.0 || dy != 0.0 {
                    // Move the cursor and update the state
                    if let Ok(state) = MOUSE_STATE.lock() {
                        if let Some(ref info) = *state {
                            // Calculate the new position
                            let new_x = info.screen_x + dx;
                            let new_y = info.screen_y + dy;

                            // Get the screen height for coordinate conversion
                            let main_display = CGDisplay::main();
                            let screen_height = main_display.pixels_high() as f64;

                            // Convert Cocoa coordinates to Core Graphics coordinates
                            // Cocoa: origin at bottom left
                            // Core Graphics: origin at top left
                            let cg_y = screen_height - new_y;

                            // Move the mouse cursor to the new position
                            let _ = CGDisplay::warp_mouse_cursor_position(
                                core_graphics::geometry::CGPoint::new(new_x, cg_y)
                            );

                            // Release the lock before getting the new color
                            drop(state);

                            // Get the color at the new position
                            if let Some((r, g, b)) = get_pixel_color(new_x, new_y) {
                                let r_int = (r * 255.0) as u8;
                                let g_int = (g * 255.0) as u8;
                                let b_int = (b * 255.0) as u8;

                                let hex_color = format!("#{:02X}{:02X}{:02X}", r_int, g_int, b_int);

                                // Update the state with the new position and color
                                if let Ok(mut state) = MOUSE_STATE.lock() {
                                    if let Some(window) = self.window() {
                                        // Convert screen coordinates to window coordinates
                                        let screen_point = NSPoint::new(new_x, new_y);
                                        let window_point: NSPoint = window.convertPointFromScreen(screen_point);

                                        // Get the scale factor
                                        let scale_factor: f64 = if let Some(screen) = window.screen() {
                                            screen.backingScaleFactor()
                                        } else {
                                            1.0
                                        };

                                        // Update the state
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

                                // Request a refresh
                                unsafe { self.setNeedsDisplay(true) };
                            }
                        }
                    }
                }
            }
        }

        // ---------------------------------------------------------------------
        // drawRect: - Dessine le contenu de la vue
        // drawRect: - Draws the view content
        // ---------------------------------------------------------------------
        /// Appelé par le système quand la vue doit être redessinée
        /// Délègue à la fonction draw_view()
        /// Called by the system when the view needs to be redrawn
        /// Delegates to the draw_view() function
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _rect: NSRect) {
            // Call the main drawing function
            draw_view(self);
        }
    }
);

// -----------------------------------------------------------------------------
// KeyableWindow - Fenêtre qui peut recevoir les événements clavier
// KeyableWindow - Window that can receive keyboard events
// -----------------------------------------------------------------------------

// Macro pour déclarer KeyableWindow avec la nouvelle syntaxe define_class! (objc2 0.6+)
// New define_class! macro syntax for objc2 0.6+
define_class!(
    // SAFETY:
    // - The superclass NSWindow does not have any subclassing requirements that we violate.
    // - KeyableWindow does not implement Drop.
    #[unsafe(super = NSWindow2)]                 // Inherit from NSWindow (parent class)
    #[thread_kind = MainThreadOnly]              // Can only be used on the main thread
    #[name = "KeyableWindow"]                    // Objective-C class name

    /// Fenêtre personnalisée qui peut devenir la fenêtre clé
    /// Par défaut, les fenêtres borderless ne peuvent pas devenir key window
    /// Custom window that can become the key window
    /// By default, borderless windows cannot become key windows
    pub struct KeyableWindow;

    impl KeyableWindow {
        // Surcharge canBecomeKeyWindow pour retourner true
        // Permet à cette fenêtre sans bordure de recevoir les événements clavier
        // Override canBecomeKeyWindow to return true
        // Allows this borderless window to receive keyboard events
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true // Yes, this window can become the key window
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
    // Create overlay windows for each screen
    unsafe {
        // Récupère la liste de tous les écrans via l'API objc2 native
        // Get the list of all screens using native objc2 API
        let screens = NSScreen::screens(mtm); // Returns Retained<NSArray<NSScreen>>

        // Itère sur chaque écran dans le tableau via l'API native
        // Iterate over each screen in the array via native API
        // Note: NSArray doesn't have a get() method in objc2, use objectAtIndex: via msg_send
        let count: usize = screens.count(); // Get the number of screens
        for i in 0..count {
            // Récupère l'écran à l'index i via objectAtIndex:
            // Get the screen at index i via objectAtIndex:
            let screen: Retained<NSScreen> = msg_send_id![&*screens, objectAtIndex: i];
            // Récupère les dimensions de l'écran via l'API objc2 native
            // Get the screen dimensions using native objc2 API
            let frame: NSRect = screen.frame(); // Returns NSRect directly

            // Crée une fenêtre KeyableWindow en utilisant l'API objc2 native
            // Create KeyableWindow using native objc2 API
            // For MainThreadOnly classes, use mtm.alloc::<Class>() pattern
            let window: Retained<KeyableWindow> = {
                // Allocate the window object using MainThreadMarker for MainThreadOnly classes
                let allocated: Allocated<KeyableWindow> = mtm.alloc(); // Allocate memory for the window
                // NSBackingStoreType: 0 = Retained, 1 = Nonretained, 2 = Buffered
                const NS_BACKING_STORE_BUFFERED: u64 = 2; // NSBackingStoreBuffered
                // Initialize with content rect, style mask, backing store type, and defer flag
                // Use msg_send_id! for init methods that return Retained
                let initialized: Retained<KeyableWindow> = {
                    msg_send_id![
                        allocated,
                        initWithContentRect: frame,                      // Window frame rectangle
                        styleMask: NSWindowStyleMask::Borderless,        // No border style
                        backing: NS_BACKING_STORE_BUFFERED,              // Buffered backing store
                        defer: Bool::NO                                  // Don't defer window creation
                    ]
                };
                initialized // Return the initialized window
            };

            // Cast KeyableWindow to NSWindow2 to access NSWindow methods
            // KeyableWindow inherits from NSWindow2 so this cast is safe
            let window_as_nswindow: &NSWindow2 = &window; // Deref coercion to parent class

            // Configure la fenêtre using NSWindow2 methods
            // Configure the window using NSWindow2 methods
            window_as_nswindow.setLevel(1000);                        // Very high level (above everything)

            let clear_color = NSColor::clearColor();                  // Transparent color
            window_as_nswindow.setBackgroundColor(Some(&clear_color)); // Transparent background

            window_as_nswindow.setOpaque(false);                      // Non-opaque
            window_as_nswindow.setHasShadow(false);                   // No shadow
            window_as_nswindow.setIgnoresMouseEvents(false);          // Receives mouse events
            window_as_nswindow.setAcceptsMouseMovedEvents(true);      // Receives mouseMoved
            // Disable window content sharing (prevents screen capture of this window)
            // NSWindowSharingType: 0 = None, 1 = ReadOnly, 2 = ReadWrite
            window_as_nswindow.setSharingType(NSWindowSharingType(0));

            // Crée la vue ColorPickerView en utilisant l'API objc2 native
            // Create ColorPickerView using native objc2 API
            // For MainThreadOnly classes, use mtm.alloc::<Class>() pattern
            let view: Retained<ColorPickerView> = {
                // Allocate the view object using MainThreadMarker for MainThreadOnly classes
                let allocated: Allocated<ColorPickerView> = mtm.alloc(); // Allocate memory for the view
                // Initialize with frame - use msg_send_id! for init methods that return Retained
                let initialized: Retained<ColorPickerView> = {
                    msg_send_id![allocated, initWithFrame: frame] // Initialize with frame
                };
                initialized // Return the initialized view
            };

            // Cast ColorPickerView to NSView for setContentView and makeFirstResponder
            // ColorPickerView inherits from NSView so this cast is safe
            let view_as_nsview: &NSView = &view; // Deref coercion to parent class

            // Configure la fenêtre avec la vue
            // Configure the window with the view
            window_as_nswindow.setContentView(Some(view_as_nsview));  // Set the content view
            window_as_nswindow.makeKeyAndOrderFront(None);            // Show and bring to front
            window_as_nswindow.makeFirstResponder(Some(view_as_nsview)); // View receives events
        } // End of for loop
    } // End of unsafe block

    // Active l'application en utilisant l'API objc2 native
    unsafe {
        // Récupère l'instance de l'application en cours d'exécution
        let running_app = NSRunningApplication::currentApplication();
        // Active l'application avec les options par défaut (vide = activation standard)
        running_app.activateWithOptions(NSApplicationActivationOptions::empty());
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
                    // Create an NSImage from CGImage
                    // Note: initWithCGImage:size: is not directly available in objc2-app-kit,
                    // so we use raw msg_send! with proper type handling
                    // -------------------------------------------------------------
                    use objc2_app_kit::NSImage;
                    use objc2::runtime::AnyObject;
                    use objc2::ClassType;
                    use objc2::encode::{Encoding, RefEncode};
                    
                    // Define a wrapper type for CGImage with proper Objective-C encoding
                    // This represents the opaque CGImage struct (not the pointer)
                    #[repr(C)]
                    struct OpaqueImage {
                        _private: [u8; 0], // Zero-sized opaque type
                    }
                    
                    // Implement RefEncode to tell objc2 the correct type encoding
                    // When passed as *const OpaqueImage, this becomes "^{CGImage=}"
                    unsafe impl RefEncode for OpaqueImage {
                        const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("CGImage", &[]));
                    }
                    
                    // Get the CGImage pointer and cast it to our opaque type
                    let cg_image_ref: *const OpaqueImage = {
                        // CGImage from core-graphics is a wrapper around CFTypeRef
                        // We need to extract the raw pointer
                        let ptr_addr = &cg_image as *const CGImage as *const *const OpaqueImage;
                        *ptr_addr // Dereference to get the raw CGImageRef
                    };

                    // Use msg_send! to call alloc on NSImage class
                    // This returns a raw pointer to the allocated object
                    let ns_image_alloc: *mut AnyObject = msg_send![NSImage::class(), alloc];
                    
                    // Initialize NSImage with CGImage using msg_send!
                    // The initWithCGImage:size: method takes a CGImageRef and NSSize
                    let full_size = NSSize::new(img_width, img_height);   // Full image size
                    
                    // Use msg_send! to call initWithCGImage:size:
                    // This consumes the allocated object and returns the initialized object
                    let ns_image_ptr: *mut AnyObject = msg_send![ns_image_alloc, initWithCGImage:cg_image_ref size:full_size];
                    
                    // Wrap in Retained - the init method returns a retained object
                    // SAFETY: initWithCGImage:size: returns a retained +1 object
                    let ns_image: Retained<NSImage> = Retained::from_raw(ns_image_ptr as *mut NSImage)
                        .expect("NSImage initWithCGImage:size: returned nil");
                    let cropped_size = NSSize::new(use_width, use_height); // Size to use after cropping

                    // Calcule la position de la loupe (centrée sur le curseur)
                    // Calculate magnifier position (centered on cursor)
                    let mag_x = info.x - mag_size / 2.0;                  // X position
                    let mag_y = info.y - mag_size / 2.0;                  // Y position

                    // Rectangle destination pour la loupe
                    // Destination rectangle for the magnifier
                    let mag_rect = NSRect::new(
                        NSPoint::new(mag_x, mag_y),     // Origin point
                        NSSize::new(mag_size, mag_size) // Size (square)
                    );

                    // Crée un chemin circulaire pour le clip
                    // Create a circular path for clipping
                    let circular_clip = NSBezierPath::bezierPathWithOvalInRect(mag_rect);

                    // -------------------------------------------------------------
                    // Dessine l'image dans le cercle
                    // Draw the image inside the circle
                    // -------------------------------------------------------------
                    // Sauvegarde l'état graphique actuel
                    // Save current graphics state
                    NSGraphicsContext::saveGraphicsState_class();

                    // Désactive l'interpolation pour un rendu pixelisé
                    // Disable interpolation for pixelated rendering
                    if let Some(graphics_context) = NSGraphicsContext::currentContext() {
                        graphics_context.setImageInterpolation(objc2_app_kit::NSImageInterpolation::None);
                    }

                    // Applique le clip circulaire
                    // Apply the circular clip
                    circular_clip.addClip();

                    // Rectangle source dans l'image
                    // Source rectangle in the image (defines the portion to draw from)
                    let from_rect = NSRect::new(
                        NSPoint::new(crop_x, crop_y), // Origin of source rectangle
                        cropped_size                   // Size of source rectangle
                    );

                    // Dessine l'image using objc2 msg_send!
                    // Draw the image from source rect to destination rect
                    // Use NSImage's drawInRect:fromRect:operation:fraction: method
                    // operation: 2 = NSCompositingOperationSourceOver (standard alpha blending)
                    // fraction: 1.0 = full opacity (no transparency)
                    const NS_COMPOSITING_OPERATION_SOURCE_OVER: usize = 2; // NSCompositingOperationSourceOver constant
                    let _: () = msg_send![&*ns_image, drawInRect:mag_rect
                                          fromRect:from_rect
                                          operation:NS_COMPOSITING_OPERATION_SOURCE_OVER
                                          fraction:1.0_f64];

                    // Restaure l'état graphique
                    // Restore graphics state
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
                    // Crée la police système avec objc2 NSFont API
                    let font: Retained<NSFont> = NSFont::systemFontOfSize(HEX_FONT_SIZE);

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
                        // Create the attribute dictionary for text
                        use objc2_foundation::NSDictionary;

                        let font_attr_key = NSString::from_str("NSFont");        // Font attribute key
                        let color_attr_key = NSString::from_str("NSColor");      // Color attribute key

                        // Create slices of keys and values for the dictionary
                        // from_slices expects &[&Key] and &[&Value]
                        let keys: &[&NSString] = &[&font_attr_key, &color_attr_key];
                        let values: &[&AnyObject] = &[
                            // Cast NSFont reference to AnyObject reference
                            unsafe { &*(font.as_ref() as *const NSFont as *const AnyObject) },
                            // Cast NSColor reference to AnyObject reference
                            unsafe { &*(text_color.as_ref() as *const NSColor as *const AnyObject) },
                        ];
                        let attributes = NSDictionary::from_slices(keys, values); // Create dictionary from slices

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