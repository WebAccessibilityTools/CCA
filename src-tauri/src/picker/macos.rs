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
use objc2::{define_class, msg_send, ClassType, MainThreadOnly}; // Class declaration macros
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

// -----------------------------------------------------------------------------
// Code commun entre plateformes
// Common code shared between platforms
// -----------------------------------------------------------------------------
use super::common::{
    ColorPickerResult,
    should_use_dark_text,
    format_hex_color,
    format_labeled_hex_color,
};

// =============================================================================
// ALIAS DE TYPES ET CONSTANTES
// =============================================================================

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
        /// En mode normal: sauvegarde la couleur et termine l'application
        /// En mode continue: sauvegarde la couleur et bascule fg/bg
        /// Called when the user clicks with the mouse
        /// In normal mode: saves the current color and terminates the application
        /// In continue mode: saves the current color and toggles fg/bg
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            // Vérifie si le mode continue est activé
            // Check if continue mode is enabled
            let is_continue_mode = if let Ok(mode) = CONTINUE_MODE.lock() {
                *mode // Copy the boolean value
            } else {
                false // Default to disabled if lock fails
            };

            // Récupère le mode fg actuel
            // Get the current fg mode
            let is_fg_mode = if let Ok(mode) = FG_MODE.lock() {
                *mode // Copy the boolean value
            } else {
                true // Default to fg if lock fails
            };

            // Vérifie si la couleur opposée a déjà été sélectionnée (= on a déjà basculé)
            // Check if the opposite color has already been selected (= we already toggled)
            // Si fg_mode=true, on vérifie BG_COLOR. Si fg_mode=false, on vérifie FG_COLOR.
            // If fg_mode=true, check BG_COLOR. If fg_mode=false, check FG_COLOR.
            let has_already_toggled = if is_fg_mode {
                // On est en mode fg, vérifie si bg a déjà été sélectionné
                // We're in fg mode, check if bg was already selected
                if let Ok(bg) = BG_COLOR.lock() {
                    bg.is_some() // True if background color exists
                } else {
                    false
                }
            } else {
                // On est en mode bg, vérifie si fg a déjà été sélectionné
                // We're in bg mode, check if fg was already selected
                if let Ok(fg) = FG_COLOR.lock() {
                    fg.is_some() // True if foreground color exists
                } else {
                    false
                }
            };

            // Lock the mutex to access the mouse state
            if let Ok(state) = MOUSE_STATE.lock() {
                // If we have information about the current color
                if let Some(ref info) = *state {
                    // Stocke la couleur dans la variable appropriée selon fg_mode
                    // Store the color in the appropriate variable based on fg_mode
                    if is_fg_mode {
                        // Stocke dans FG_COLOR
                        // Store in FG_COLOR
                        if let Ok(mut fg_color) = FG_COLOR.lock() {
                            *fg_color = Some((info.r, info.g, info.b));
                        }
                    } else {
                        // Stocke dans BG_COLOR
                        // Store in BG_COLOR
                        if let Ok(mut bg_color) = BG_COLOR.lock() {
                            *bg_color = Some((info.r, info.g, info.b));
                        }
                    }
                }
            }

            if is_continue_mode && !has_already_toggled {
                // Mode continue, premier clic: bascule entre fg et bg
                // Continue mode, first click: toggle between fg and bg
                if let Ok(mut fg_mode) = FG_MODE.lock() {
                    *fg_mode = !*fg_mode; // Toggle fg mode
                }
                // Demande un rafraîchissement pour mettre à jour l'affichage
                // Request a refresh to update the display
                self.setNeedsDisplay(true);
            } else {
                // Mode normal OU mode continue après toggle: termine l'application
                // Normal mode OR continue mode after toggle: stop the application
                stop_application();
            }
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
            let location: NSPoint = event.locationInWindow();

            // Get the parent window of this view
            let window_opt: Option<Retained<NSWindow2>> = self.window();

            // If we have a valid window
            if let Some(window) = window_opt {
                // Convert window coordinates to screen coordinates
                let screen_location: NSPoint = window.convertPointToScreen(location);

                // Get the screen scale factor (for Retina)
                let scale_factor: f64 = if let Some(screen) = window.screen() {
                    screen.backingScaleFactor() // 2.0 for Retina, 1.0 otherwise
                } else {
                    1.0 // Default value if no screen
                };

                // Récupère le nombre de pixels capturés pour la taille de capture
                // Get captured pixels count for capture size
                let captured_pixels = match CURRENT_CAPTURED_PIXELS.lock() {
                    Ok(p) => *p,
                    Err(_) => CAPTURED_PIXELS,
                };
                
                // Taille de capture en points (ajustée pour Retina)
                // Capture size in points (adjusted for Retina)
                let capture_size = captured_pixels / scale_factor;

                // Capture la zone et extrait la couleur du pixel central
                // Capture the area and extract the center pixel color
                if let Some((_image, r, g, b)) = capture_and_get_center_color(screen_location.x, screen_location.y, capture_size, captured_pixels) {
                    // Format the color in hexadecimal (#RRGGBB)
                    // Utilise format_hex_color du module common
                    // Uses format_hex_color from common module
                    let hex_color = format_hex_color(r, g, b);

                    // Update the global state
                    if let Ok(mut state) = MOUSE_STATE.lock() {
                        // Create the new state structure
                        *state = Some(MouseColorInfo {
                            x: location.x,           // X position in window
                            y: location.y,           // Y position in window
                            screen_x: screen_location.x, // X position on screen
                            screen_y: screen_location.y, // Y position on screen
                            r,                       // Red component [0-255]
                            g,                       // Green component [0-255]
                            b,                       // Blue component [0-255]
                            hex_color: hex_color.clone(), // Hex code "#RRGGBB"
                            scale_factor,            // Retina scale factor
                        });
                    }

                    // Request a display refresh
                    self.setNeedsDisplay(true);
                }
            }
        }

        // ---------------------------------------------------------------------
        // scrollWheel: - Gère la molette de défilement
        // scrollWheel: - Handles scroll wheel
        // ---------------------------------------------------------------------
        /// Appelé quand l'utilisateur utilise la molette de défilement
        /// Sans Shift: ajuste le niveau de zoom
        /// Avec Shift: ajuste le nombre de pixels capturés
        /// Called when the user uses the scroll wheel
        /// Without Shift: adjusts zoom level
        /// With Shift: adjusts captured pixels count
        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            // Get the vertical delta of the scroll wheel
            let delta_y: f64 = event.deltaY();

            // If the wheel moved
            if delta_y != 0.0 {
                // Get modifier flags to check for Shift
                // Récupère les modificateurs pour vérifier Shift
                let modifier_flags: NSEventModifierFlags = event.modifierFlags();
                let shift_pressed = modifier_flags.contains(NSEventModifierFlags::Shift);

                if shift_pressed {
                    // Shift + molette: ajuste le nombre de pixels capturés
                    // Shift + wheel: adjust captured pixels count
                    if let Ok(mut pixels) = CURRENT_CAPTURED_PIXELS.lock() {
                        // Calcule la nouvelle valeur (direction inversée pour UX intuitive)
                        // Calculate new value (inverted direction for intuitive UX)
                        let direction = if delta_y > 0.0 { 1.0 } else { -1.0 };
                        let new_pixels = *pixels + direction * CAPTURED_PIXELS_STEP;
                        // Clamp entre min et max
                        // Clamp between min and max
                        *pixels = new_pixels.clamp(CAPTURED_PIXELS_MIN, CAPTURED_PIXELS_MAX);
                    }
                } else {
                    // Molette seule: ajuste le zoom
                    // Wheel alone: adjust zoom
                    if let Ok(mut zoom) = CURRENT_ZOOM.lock() {
                        // Calculate new zoom by adding delta * zoom step
                        let new_zoom = *zoom + delta_y * ZOOM_STEP;
                        // Clamp zoom between ZOOM_MIN and ZOOM_MAX
                        *zoom = new_zoom.clamp(ZOOM_MIN, ZOOM_MAX);
                    }
                }

                // Request a refresh to display the change
                self.setNeedsDisplay(true);
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
            let key_code: u16 = event.keyCode();
            // Get the modifiers (Shift, Ctrl, etc.)
            let modifier_flags: NSEventModifierFlags = event.modifierFlags();

            // Check if Shift is pressed
            // In objc2-app-kit 0.3, the constant is NSEventModifierFlags::Shift
            let shift_pressed = modifier_flags.contains(NSEventModifierFlags::Shift);
            
            // Get the scale factor to adjust movement for Retina displays
            // Sur Retina (scale_factor=2.0), 1 pixel = 0.5 point
            // On Retina (scale_factor=2.0), 1 pixel = 0.5 point
            let scale_factor = if let Ok(state) = MOUSE_STATE.lock() {
                if let Some(ref info) = *state {
                    info.scale_factor
                } else {
                    1.0
                }
            } else {
                1.0
            };
            
            // Determine movement distance in points
            // 1 pixel = 1/scale_factor points
            // Sans Shift: 1 pixel, avec Shift: SHIFT_MOVE_PIXELS pixels
            // Without Shift: 1 pixel, with Shift: SHIFT_MOVE_PIXELS pixels
            let pixels_to_move = if shift_pressed { SHIFT_MOVE_PIXELS } else { 1.0 };
            let move_amount = pixels_to_move / scale_factor;

            // Key codes: ESC = 53, Enter/Return = 36, C = 8, I = 34, O = 31
            if key_code == 53 {
                // ESC - Cancel the selection
                stop_application();
            } else if key_code == 36 {
                // Enter - Confirm the selection and exit
                // Entrée - Confirme la sélection et quitte
                // Récupère le mode fg actuel
                // Get the current fg mode
                let is_fg_mode = if let Ok(mode) = FG_MODE.lock() {
                    *mode
                } else {
                    true
                };

                if let Ok(state) = MOUSE_STATE.lock() {
                    if let Some(ref info) = *state {
                        // Stocke la couleur dans la variable appropriée selon fg_mode
                        // Store the color in the appropriate variable based on fg_mode
                        if is_fg_mode {
                            if let Ok(mut fg_color) = FG_COLOR.lock() {
                                *fg_color = Some((info.r, info.g, info.b));
                            }
                        } else {
                            if let Ok(mut bg_color) = BG_COLOR.lock() {
                                *bg_color = Some((info.r, info.g, info.b));
                            }
                        }
                    }
                }
                stop_application();
            } else if key_code == 8 {
                // C key - Toggle continue mode
                // Touche C - Bascule le mode continue
                if let Ok(mut continue_mode) = CONTINUE_MODE.lock() {
                    *continue_mode = !*continue_mode; // Toggle the mode
                }
                // Request a refresh to update the display
                // Demande un rafraîchissement pour mettre à jour l'affichage
                self.setNeedsDisplay(true);
            } else if key_code == 34 {
                // I key - Zoom in or increase captured pixels
                // Touche I - Zoom avant ou augmente les pixels capturés
                if shift_pressed {
                    // Shift+I: augmente le nombre de pixels capturés
                    // Shift+I: increase captured pixels count
                    if let Ok(mut pixels) = CURRENT_CAPTURED_PIXELS.lock() {
                        *pixels = (*pixels + CAPTURED_PIXELS_STEP).min(CAPTURED_PIXELS_MAX);
                    }
                } else {
                    // I seul: zoom avant
                    // I alone: zoom in
                    if let Ok(mut zoom) = CURRENT_ZOOM.lock() {
                        *zoom = (*zoom + ZOOM_STEP).min(ZOOM_MAX);
                    }
                }
                // Request a refresh to update the display
                // Demande un rafraîchissement pour mettre à jour l'affichage
                self.setNeedsDisplay(true);
            } else if key_code == 31 {
                // O key - Zoom out or decrease captured pixels
                // Touche O - Zoom arrière ou diminue les pixels capturés
                if shift_pressed {
                    // Shift+O: diminue le nombre de pixels capturés
                    // Shift+O: decrease captured pixels count
                    if let Ok(mut pixels) = CURRENT_CAPTURED_PIXELS.lock() {
                        *pixels = (*pixels - CAPTURED_PIXELS_STEP).max(CAPTURED_PIXELS_MIN);
                    }
                } else {
                    // O seul: zoom arrière
                    // O alone: zoom out
                    if let Ok(mut zoom) = CURRENT_ZOOM.lock() {
                        *zoom = (*zoom - ZOOM_STEP).max(ZOOM_MIN);
                    }
                }
                // Request a refresh to update the display
                // Demande un rafraîchissement pour mettre à jour l'affichage
                self.setNeedsDisplay(true);
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
                            // Calculate the new position (in points)
                            let new_x = info.screen_x + dx;
                            let new_y = info.screen_y + dy;

                            // Get scale factor for pixel conversion
                            let scale_factor = info.scale_factor;

                            // Récupère le nombre de pixels capturés pour la taille de capture
                            // Get captured pixels count for capture size
                            let captured_pixels = match CURRENT_CAPTURED_PIXELS.lock() {
                                Ok(p) => *p,
                                Err(_) => CAPTURED_PIXELS,
                            };
                            
                            // Taille de capture en points (ajustée pour Retina)
                            // Capture size in points (adjusted for Retina)
                            let capture_size = captured_pixels / scale_factor;

                            // Get screen height in points from the window's screen
                            // Récupère la hauteur de l'écran en points depuis l'écran de la fenêtre
                            let screen_height_points = if let Some(window) = self.window() {
                                if let Some(screen) = window.screen() {
                                    screen.frame().size.height
                                } else {
                                    if let Some(mtm) = objc2_foundation::MainThreadMarker::new() {
                                        if let Some(main_screen) = NSScreen::mainScreen(mtm) {
                                            main_screen.frame().size.height
                                        } else {
                                            let main_display = CGDisplay::main();
                                            main_display.pixels_high() as f64 / scale_factor
                                        }
                                    } else {
                                        let main_display = CGDisplay::main();
                                        main_display.pixels_high() as f64 / scale_factor
                                    }
                                }
                            } else {
                                let main_display = CGDisplay::main();
                                main_display.pixels_high() as f64 / scale_factor
                            };

                            // Convert Cocoa coordinates (origin bottom-left, in points) to 
                            // Core Graphics coordinates (origin top-left, in points)
                            // CGEvent uses POINTS, not pixels
                            // Convertit les coordonnées Cocoa (origine en bas, en points) vers
                            // les coordonnées Core Graphics (origine en haut, en points)
                            let cg_x = new_x;
                            let cg_y = screen_height_points - new_y;

                            // Move the mouse cursor using CGEvent (more reliable than warp)
                            // Déplace le curseur de la souris en utilisant CGEvent (plus fiable que warp)
                            use core_graphics::event::{CGEvent, CGEventType, CGMouseButton};
                            use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
                            use core_graphics::geometry::CGPoint as CGPointCG;
                            
                            if let Ok(source) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
                                let point = CGPointCG::new(cg_x, cg_y);
                                if let Ok(event) = CGEvent::new_mouse_event(
                                    source,
                                    CGEventType::MouseMoved,
                                    point,
                                    CGMouseButton::Left
                                ) {
                                    event.post(core_graphics::event::CGEventTapLocation::HID);
                                }
                            }

                            // Release the lock before getting the new color
                            drop(state);

                            // Capture la zone et extrait la couleur du pixel central
                            // Capture the area and extract the center pixel color
                            if let Some((_image, r, g, b)) = capture_and_get_center_color(new_x, new_y, capture_size, captured_pixels) {
                                // Utilise format_hex_color du module common
                                // Uses format_hex_color from common module
                                let hex_color = format_hex_color(r, g, b);

                                // Update the state with the new position and color
                                if let Ok(mut state) = MOUSE_STATE.lock() {
                                    if let Some(window) = self.window() {
                                        // Convert screen coordinates to window coordinates
                                        let screen_point = NSPoint::new(new_x, new_y);
                                        let window_point: NSPoint = window.convertPointFromScreen(screen_point);

                                        // Update the state
                                        *state = Some(MouseColorInfo {
                                            x: window_point.x,
                                            y: window_point.y,
                                            screen_x: new_x,
                                            screen_y: new_y,
                                            r,
                                            g,
                                            b,
                                            hex_color,
                                            scale_factor,
                                        });
                                    }
                                }

                                // Request a refresh
                                self.setNeedsDisplay(true);
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

/// État global pour le nombre de pixels capturés
/// Initialisé avec la valeur par défaut de config
/// Global state for captured pixels count
/// Initialized with default value from config
static CURRENT_CAPTURED_PIXELS: Mutex<f64> = Mutex::new(CAPTURED_PIXELS);

/// Nombre minimum de pixels capturés (doit être impair)
/// Minimum captured pixels (must be odd)
const CAPTURED_PIXELS_MIN: f64 = 9.0;

/// Nombre maximum de pixels capturés (doit être impair)
/// Maximum captured pixels (must be odd)
const CAPTURED_PIXELS_MAX: f64 = 21.0;

/// Pas d'incrément pour les pixels capturés (2 pour rester impair)
/// Increment step for captured pixels (2 to stay odd)
const CAPTURED_PIXELS_STEP: f64 = 2.0;

/// Stocke la couleur de premier plan sélectionnée (foreground)
/// Stores the selected foreground color
static FG_COLOR: Mutex<Option<(u8, u8, u8)>> = Mutex::new(None);

/// Stocke la couleur d'arrière-plan sélectionnée (background)
/// Stores the selected background color
static BG_COLOR: Mutex<Option<(u8, u8, u8)>> = Mutex::new(None);

/// Mode d'affichage: true = arc du haut (foreground), false = arc du bas (background)
/// Display mode: true = top arc (foreground), false = bottom arc (background)
static FG_MODE: Mutex<bool> = Mutex::new(true);

/// Mode continue: true = activé, false = désactivé
/// Continue mode: true = enabled, false = disabled
/// Quand activé, une pastille "C" rouge est affichée avant le texte hex
/// When enabled, a red "C" badge is displayed before the hex text
static CONTINUE_MODE: Mutex<bool> = Mutex::new(false);

// ColorPickerResult est maintenant défini dans common.rs
// ColorPickerResult is now defined in common.rs

/// Structure contenant toutes les informations sur la position et la couleur actuelles
/// Structure containing all information about current position and color
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
/// * `x` - Coordonnée X du centre (coordonnées Cocoa en points, origine en bas à gauche)
/// * `y` - Coordonnée Y du centre (coordonnées Cocoa en points)
/// * `size` - Taille du carré à capturer (en points)
///
/// # Retourne
/// * `Some(CGImage)` - L'image capturée si la capture a réussi
/// * `None` - Si la capture a échoué
fn capture_zoom_area(x: f64, y: f64, size: f64) -> Option<CGImage> {
    // Importe les types géométriques de Core Graphics
    use core_graphics::geometry::{CGRect, CGPoint as CGPointStruct, CGSize};

    // Récupère l'écran principal
    let main_display = CGDisplay::main();
    let screen_height_pixels = main_display.pixels_high() as f64;
    
    // Récupère la hauteur en points de l'écran principal
    let main_screen_height_points = if let Some(mtm) = objc2_foundation::MainThreadMarker::new() {
        if let Some(main_screen) = NSScreen::mainScreen(mtm) {
            main_screen.frame().size.height
        } else {
            screen_height_pixels / 2.0 // Default Retina
        }
    } else {
        screen_height_pixels / 2.0 // Default Retina
    };
    
    // Convertit Y de Cocoa (origine en bas) vers CG (origine en haut)
    let cg_y = main_screen_height_points - y;

    // Coordonnées en points pour CG
    let center_x = x;
    let center_y = cg_y;

    // La taille de capture en points
    let capture_size = size;
    let half_size = capture_size / 2.0;

    // Crée le rectangle de capture centré sur le point (en points)
    let rect = CGRect::new(
        &CGPointStruct::new(center_x - half_size, center_y - half_size),
        &CGSize::new(capture_size, capture_size)
    );

    // Capture l'image dans le rectangle spécifié
    main_display.image_for_rect(rect)
}

/// Extrait la couleur du pixel central d'une image CGImage
///
/// # Arguments
/// * `image` - L'image capturée
///
/// # Retourne
/// * `Some((r, g, b))` - Les composantes RGB en u8 [0-255]
/// * `None` - Si l'extraction a échoué
fn get_center_pixel_from_image(image: &CGImage, target_pixels: f64) -> Option<(u8, u8, u8)> {
    // Récupère les dimensions de l'image
    let img_width = image.width() as f64;
    let img_height = image.height() as f64;
    
    // Calcule le décalage de crop comme dans draw_view
    // Calculate crop offset like in draw_view
    // Note: Dans draw_view, crop_x et crop_y sont utilisés pour NSImage.drawInRect:fromRect:
    // où l'origine est en BAS à gauche (convention Cocoa)
    // Mais les données CGImage sont stockées avec l'origine en HAUT à gauche
    // In draw_view, crop_x and crop_y are used for NSImage.drawInRect:fromRect:
    // where origin is at BOTTOM-left (Cocoa convention)
    // But CGImage data is stored with origin at TOP-left
    
    let crop_x = if img_width > target_pixels {
        ((img_width - target_pixels) / 2.0).floor()
    } else {
        0.0
    };
    
    // Pour NSImage (draw_view), crop_y est depuis le bas
    // Pour CGImage (données), on doit calculer depuis le haut
    // For NSImage (draw_view), crop_y is from bottom
    // For CGImage (data), we need to calculate from top
    let crop_y_from_bottom = if img_height > target_pixels {
        ((img_height - target_pixels) / 2.0).floor()
    } else {
        0.0
    };
    
    // Taille effective après crop
    // Effective size after crop
    let use_width = if img_width > target_pixels { target_pixels } else { img_width };
    let use_height = if img_height > target_pixels { target_pixels } else { img_height };
    
    // Le pixel central en X (même convention)
    // Center pixel in X (same convention)
    let center_x = (crop_x + use_width / 2.0).floor() as usize;
    
    // Le pixel central en Y : convertir de "depuis le bas" vers "depuis le haut"
    // Center pixel in Y: convert from "from bottom" to "from top"
    // Dans NSImage: le centre est à crop_y_from_bottom + use_height/2 depuis le bas
    // Dans CGImage: on veut la distance depuis le haut
    // In NSImage: center is at crop_y_from_bottom + use_height/2 from bottom
    // In CGImage: we want distance from top
    let center_y_from_bottom = crop_y_from_bottom + use_height / 2.0;
    let center_y = (img_height - center_y_from_bottom).floor() as usize;
    
    // Récupère les données brutes de l'image
    let data = image.data();
    let bytes_per_row = image.bytes_per_row() as usize;
    let bits_per_pixel = image.bits_per_pixel() as usize;
    let bytes_per_pixel = bits_per_pixel / 8;
    
    // Calcule l'offset du pixel central dans les données
    let offset = (center_y * bytes_per_row) + (center_x * bytes_per_pixel);
    
    // Vérifie qu'on a assez de données
    let data_len = data.len() as usize;
    if offset + bytes_per_pixel <= data_len {
        // Les données sont en format BGRA (Blue, Green, Red, Alpha)
        let b = data[offset];
        let g = data[offset + 1];
        let r = data[offset + 2];
        Some((r, g, b))
    } else {
        None
    }
}

/// Capture une zone et retourne à la fois l'image et la couleur du pixel central
///
/// # Arguments
/// * `x` - Coordonnée X du centre (coordonnées Cocoa en points)
/// * `y` - Coordonnée Y du centre (coordonnées Cocoa en points)
/// * `size` - Taille du carré à capturer (en points)
/// * `target_pixels` - Nombre de pixels cibles pour le crop (utilisé pour trouver le centre)
///
/// # Retourne
/// * `Some((CGImage, r, g, b))` - L'image et les composantes RGB du pixel central
/// * `None` - Si la capture a échoué
fn capture_and_get_center_color(x: f64, y: f64, size: f64, target_pixels: f64) -> Option<(CGImage, u8, u8, u8)> {
    // Capture la zone
    let image = capture_zoom_area(x, y, size)?;
    
    // Extrait la couleur du pixel central (en tenant compte du crop)
    let (r, g, b) = get_center_pixel_from_image(&image, target_pixels)?;
    
    Some((image, r, g, b))
}

// =============================================================================
// API PUBLIQUE
// =============================================================================

/// Flag global pour signaler l'arrêt du picker
/// Global flag to signal picker stop
static SHOULD_STOP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Fonction helper pour arrêter le picker et réafficher le curseur
/// Helper function to stop the picker and show cursor again
fn stop_application() {
    // Signale l'arrêt via le flag atomique
    // Signal stop via the atomic flag
    SHOULD_STOP.store(true, std::sync::atomic::Ordering::SeqCst);
    
    // Réaffiche le curseur de la souris
    // Show the mouse cursor again
    NSCursor::unhide();
}

/// Exécute l'application color picker sur macOS
/// Runs the color picker application on macOS
///
/// # Arguments
/// * `fg` - Si true, commence en mode foreground. Si false, commence en mode background.
/// * `fg` - If true, starts in foreground mode. If false, starts in background mode.
///
/// # Retourne / Returns
/// * `ColorPickerResult` avec foreground et/ou background remplis selon les sélections
/// * `ColorPickerResult` with foreground and/or background filled based on selections
/// * Les deux champs sont None si l'utilisateur a appuyé ESC pour annuler
/// * Both fields are None if user pressed ESC to cancel
pub fn run(fg: bool) -> ColorPickerResult {
    // Stocke le mode fg dans la variable globale
    // Store the fg mode in the global variable
    if let Ok(mut mode) = FG_MODE.lock() {
        *mode = fg; // Set the display mode (true = top arc, false = bottom arc)
    }

    // Réinitialise les couleurs sélectionnées
    // Reset the selected colors
    if let Ok(mut color) = FG_COLOR.lock() {
        *color = None; // Clear any previously selected foreground color
    }
    if let Ok(mut color) = BG_COLOR.lock() {
        *color = None; // Clear any previously selected background color
    }

    // Réinitialise le mode continue
    // Reset the continue mode
    if let Ok(mut mode) = CONTINUE_MODE.lock() {
        *mode = false; // Disable continue mode at start
    }

    // Réinitialise le nombre de pixels capturés à la valeur par défaut
    // Reset captured pixels to default value
    if let Ok(mut pixels) = CURRENT_CAPTURED_PIXELS.lock() {
        *pixels = CAPTURED_PIXELS; // Reset to default from config
    }

    // Réinitialise le zoom à la valeur par défaut
    // Reset zoom to default value
    if let Ok(mut zoom) = CURRENT_ZOOM.lock() {
        *zoom = INITIAL_ZOOM_FACTOR; // Reset to default from config
    }

    // Réinitialise le flag d'arrêt
    // Reset the stop flag
    SHOULD_STOP.store(false, std::sync::atomic::Ordering::SeqCst);

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
            let screen: Retained<NSScreen> = msg_send![&*screens, objectAtIndex: i];
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
                // Use msg_send! for init methods that return Retained
                let initialized: Retained<KeyableWindow> = {
                    msg_send![
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
                // Initialize with frame - use msg_send! for init methods that return Retained
                let initialized: Retained<ColorPickerView> = {
                    msg_send![allocated, initWithFrame: frame] // Initialize with frame
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
    {
        // Récupère l'instance de l'application en cours d'exécution
        let running_app = NSRunningApplication::currentApplication();
        // Active l'application avec les options par défaut (vide = activation standard)
        running_app.activateWithOptions(NSApplicationActivationOptions::empty());
    }

    // Initialise MOUSE_STATE avec la position actuelle de la souris
    // Initialize MOUSE_STATE with the current mouse position
    {
        use core_graphics::event::CGEvent;
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
        
        // Récupère la position actuelle de la souris via Core Graphics
        // Get current mouse position via Core Graphics
        if let Ok(source) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
            let cg_event = CGEvent::new(source);
            if let Ok(event) = cg_event {
                // CGEvent.location() retourne des coordonnées en POINTS (Global Display Coordinates)
                // avec l'origine en haut à gauche
                // CGEvent.location() returns coordinates in POINTS (Global Display Coordinates)
                // with origin at top-left
                let cg_point = event.location();
                
                // Récupère le scale factor et la hauteur en points
                // Get the scale factor and height in points
                let scale_factor = if let Some(main_screen) = NSScreen::mainScreen(mtm) {
                    main_screen.backingScaleFactor()
                } else {
                    2.0 // Default to Retina
                };
                
                let screen_height_points = if let Some(main_screen) = NSScreen::mainScreen(mtm) {
                    main_screen.frame().size.height
                } else {
                    let main_display = CGDisplay::main();
                    main_display.pixels_high() as f64 / scale_factor
                };
                
                // Convertit CG (origine en haut) vers Cocoa (origine en bas)
                // Convert CG (origin at top) to Cocoa (origin at bottom)
                let cocoa_x = cg_point.x;
                let cocoa_y = screen_height_points - cg_point.y;
                
                // Récupère le nombre de pixels capturés pour la taille de capture
                // Get captured pixels count for capture size
                let captured_pixels = match CURRENT_CAPTURED_PIXELS.lock() {
                    Ok(p) => *p,
                    Err(_) => CAPTURED_PIXELS,
                };
                
                // Taille de capture en points (ajustée pour Retina)
                // Capture size in points (adjusted for Retina)
                let capture_size = captured_pixels / scale_factor;
                
                // Capture la zone et extrait la couleur du pixel central
                // Capture the area and extract the center pixel color
                if let Some((_image, r, g, b)) = capture_and_get_center_color(cocoa_x, cocoa_y, capture_size, captured_pixels) {
                    // Utilise format_hex_color du module common
                    // Uses format_hex_color from common module
                    let hex_color = format_hex_color(r, g, b);
                    
                    // Initialise MOUSE_STATE
                    // Initialize MOUSE_STATE
                    if let Ok(mut state) = MOUSE_STATE.lock() {
                        *state = Some(MouseColorInfo {
                            x: cocoa_x,        // Position X dans les coordonnées de la fenêtre
                            y: cocoa_y,        // Position Y dans les coordonnées de la fenêtre
                            screen_x: cocoa_x, // Position X dans les coordonnées de l'écran
                            screen_y: cocoa_y, // Position Y dans les coordonnées de l'écran
                            r,
                            g,
                            b,
                            hex_color,
                            scale_factor,
                        });
                    }
                }
            }
        }
    }

    // Cache le curseur de la souris
    NSCursor::hide();

    // Boucle d'événements personnalisée (au lieu de app.run() qui fermerait Tauri)
    // Custom event loop (instead of app.run() which would close Tauri)
    unsafe {
        use objc2_foundation::NSDate;
        
        while !SHOULD_STOP.load(std::sync::atomic::Ordering::SeqCst) {
            // Timeout court pour vérifier le flag régulièrement
            // Short timeout to check the flag regularly
            let timeout: Retained<NSDate> = msg_send![
                NSDate::class(),
                dateWithTimeIntervalSinceNow: 0.016f64  // ~60fps
            ];
            
            let event = app.nextEventMatchingMask_untilDate_inMode_dequeue(
                objc2_app_kit::NSEventMask::Any,
                Some(&timeout),
                objc2_foundation::NSDefaultRunLoopMode,
                true
            );
            
            if let Some(event) = event {
                app.sendEvent(&event);
            }
            
            app.updateWindows();
        }
    }
    
    // Ferme toutes les fenêtres de l'application qui sont au niveau 1000 (nos fenêtres picker)
    // Close all application windows that are at level 1000 (our picker windows)
    unsafe {
        let windows = app.windows();
        let count: usize = windows.count();
        for i in 0..count {
            let win: Retained<NSWindow2> = msg_send![&*windows, objectAtIndex: i];
            if win.level() == 1000 {
                win.orderOut(None);
            }
        }
    }

    // Récupère les couleurs sélectionnées
    // Get the selected colors
    let fg_color = if let Ok(color) = FG_COLOR.lock() {
        color.clone() // Clone the Option<(u8, u8, u8)>
    } else {
        None // Return None if lock fails
    };

    let bg_color = if let Ok(color) = BG_COLOR.lock() {
        color.clone() // Clone the Option<(u8, u8, u8)>
    } else {
        None // Return None if lock fails
    };

    // Récupère l'état du mode continue
    // Get the continue mode state
    let was_continue_mode = if let Ok(mode) = CONTINUE_MODE.lock() {
        *mode // Copy the boolean value
    } else {
        false // Default to false if lock fails
    };

    // Construit le résultat avec les deux couleurs et le mode continue
    // Build the result with both colors and continue mode
    ColorPickerResult {
        foreground: fg_color,       // Foreground color (may be None)
        background: bg_color,       // Background color (may be None)
        continue_mode: was_continue_mode, // Whether continue mode was enabled
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
    let overlay_color = NSColor::colorWithCalibratedWhite_alpha(0.0, 0.05);
    // Définit comme couleur de remplissage
    overlay_color.set();

    // Récupère les limites de la vue
    let bounds: NSRect = view.bounds();
    // Crée un chemin rectangulaire couvrant toute la vue
    let bounds_path = NSBezierPath::bezierPathWithRect(bounds);
    // Remplit avec la couleur overlay
    bounds_path.fill();

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

            // Récupère le nombre de pixels capturés actuel
            // Get the current captured pixels count
            let captured_pixels = match CURRENT_CAPTURED_PIXELS.lock() {
                Ok(p) => *p,
                Err(_) => CAPTURED_PIXELS, // Fallback to default constant
            };

            // Calcule la taille de la loupe à afficher
            // mag_size = nombre de pixels capturés × facteur de zoom
            let mag_size = captured_pixels * current_zoom;
            // Taille de capture ajustée pour le facteur d'échelle Retina
            let capture_size = captured_pixels / info.scale_factor;

            // Capture la zone de pixels autour du curseur
            if let Some(cg_image) = capture_zoom_area(info.screen_x, info.screen_y, capture_size) {
                // Dimensions de l'image capturée
                let img_width = cg_image.width() as f64;
                let img_height = cg_image.height() as f64;
                let target_pixels = captured_pixels;

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
                    let ns_image_ptr: *mut AnyObject = msg_send![ns_image_alloc, initWithCGImage: cg_image_ref, size: full_size];
                    
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
                    let _: () = msg_send![
                        &*ns_image,
                        drawInRect: mag_rect,
                        fromRect: from_rect,
                        operation: NS_COMPOSITING_OPERATION_SOURCE_OVER,
                        fraction: 1.0_f64
                    ];

                    // Restaure l'état graphique
                    // Restore graphics state
                    NSGraphicsContext::restoreGraphicsState_class();

                    // -------------------------------------------------------------
                    // Dessine le réticule central
                    // Draw the central reticle
                    // -------------------------------------------------------------
                    // Centre de la loupe
                    // Center of the magnifier
                    let center_x = mag_x + mag_size / 2.0;
                    let center_y = mag_y + mag_size / 2.0;

                    // Taille du réticule: FIXE, basée uniquement sur current_zoom
                    // Reticle size: FIXED, based only on current_zoom
                    let reticle_size = current_zoom;
                    let half_reticle = reticle_size / 2.0;

                    // Le réticule est toujours centré dans la loupe
                    // The reticle is always centered in the magnifier
                    let reticle_center_x = center_x;
                    let reticle_center_y = center_y;

                    // Rectangle du réticule
                    // Reticle rectangle
                    let square_rect = NSRect::new(
                        NSPoint::new(reticle_center_x - half_reticle, reticle_center_y - half_reticle),
                        NSSize::new(reticle_size, reticle_size)
                    );

                    // Couleur grise pour le réticule
                    // Gray color for the reticle
                    let gray_color = NSColor::colorWithCalibratedRed_green_blue_alpha(0.5, 0.5, 0.5, 1.0);
                    gray_color.setStroke();

                    // Dessine le carré du réticule
                    // Draw the reticle square
                    let reticle_path = NSBezierPath::bezierPathWithRect(square_rect);
                    reticle_path.setLineWidth(1.0);
                    reticle_path.stroke();
                    
                    // Garde use_width pour référence si nécessaire
                    // Keep use_width for reference if needed
                    let _actual_pixels = use_width;

                    // -------------------------------------------------------------
                    // Dessine la bordure colorée (arc haut ou bas selon fg_mode)
                    // Draw the colored border (top or bottom arc based on fg_mode)
                    // -------------------------------------------------------------
                    // Parse la couleur hex actuelle
                    let hex = &info.hex_color[1..]; // Enlève le # / Remove the #
                    let r_val = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f64 / 255.0; // Red component
                    let g_val = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f64 / 255.0; // Green component
                    let b_val = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f64 / 255.0; // Blue component

                    // Récupère le mode fg depuis la variable globale
                    // Get the fg mode from the global variable
                    let fg_mode = if let Ok(mode) = FG_MODE.lock() {
                        *mode // Copy the boolean value
                    } else {
                        true // Default to top arc if lock fails
                    };
                    
                    // Récupère le mode continue
                    // Get continue mode
                    let is_continue_mode = if let Ok(mode) = CONTINUE_MODE.lock() {
                        *mode
                    } else {
                        false
                    };
                    
                    // Récupère les couleurs déjà capturées
                    // Get already captured colors
                    let captured_fg = if let Ok(color) = FG_COLOR.lock() {
                        *color
                    } else {
                        None
                    };
                    let captured_bg = if let Ok(color) = BG_COLOR.lock() {
                        *color
                    } else {
                        None
                    };

                    // Rayon du cercle de bordure (centre de l'épaisseur de la bordure)
                    // Radius of the border circle (center of the border thickness)
                    // On soustrait 0.5px pour que le bord intérieur chevauche légèrement la loupe
                    // Subtract 0.5px so the inner edge slightly overlaps the magnifier
                    let border_radius = mag_size / 2.0 + BORDER_WIDTH / 2.0 - 0.5;

                    // En mode continue, dessine d'abord l'arc de la couleur déjà capturée (si elle existe)
                    // In continue mode, first draw the arc for the already captured color (if it exists)
                    if is_continue_mode {
                        // Si on est en mode fg (arc du haut), dessiner la couleur bg capturée en bas
                        // If we're in fg mode (top arc), draw the captured bg color at bottom
                        // Si on est en mode bg (arc du bas), dessiner la couleur fg capturée en haut
                        // If we're in bg mode (bottom arc), draw the captured fg color at top
                        let (captured_color, captured_start, captured_end) = if fg_mode {
                            // On capture fg (haut), donc afficher bg capturé (bas) si existant
                            // Capturing fg (top), so show captured bg (bottom) if exists
                            (captured_bg, 180.0_f64, 360.0_f64)
                        } else {
                            // On capture bg (bas), donc afficher fg capturé (haut) si existant
                            // Capturing bg (bottom), so show captured fg (top) if exists
                            (captured_fg, 0.0_f64, 180.0_f64)
                        };
                        
                        if let Some((cap_r, cap_g, cap_b)) = captured_color {
                            // Dessine l'arc de la couleur capturée
                            // Draw the arc for the captured color
                            let cap_r_val = cap_r as f64 / 255.0;
                            let cap_g_val = cap_g as f64 / 255.0;
                            let cap_b_val = cap_b as f64 / 255.0;
                            
                            let captured_color_ns = NSColor::colorWithCalibratedRed_green_blue_alpha(
                                cap_r_val, cap_g_val, cap_b_val, 1.0
                            );
                            captured_color_ns.setStroke();
                            
                            let captured_arc_path = NSBezierPath::bezierPath();
                            let _: () = msg_send![
                                &*captured_arc_path,
                                appendBezierPathWithArcWithCenter: NSPoint::new(center_x, center_y),
                                radius: border_radius,
                                startAngle: captured_start,
                                endAngle: captured_end,
                                clockwise: Bool::NO
                            ];
                            captured_arc_path.setLineWidth(BORDER_WIDTH);
                            captured_arc_path.stroke();
                        }
                    }

                    // Couleur de la bordure = couleur du pixel actuel
                    // Border color = current pixel color
                    let border_color = NSColor::colorWithCalibratedRed_green_blue_alpha(r_val, g_val, b_val, 1.0);
                    border_color.setStroke(); // Set as stroke color

                    // Crée le chemin pour l'arc actuel (haut ou bas selon fg_mode)
                    // Create the path for the current arc (top or bottom based on fg_mode)
                    let arc_path = NSBezierPath::bezierPath(); // Create empty bezier path
                    
                    // Angles pour les arcs (en degrés, sens anti-horaire depuis l'axe X positif)
                    // Angles for arcs (in degrees, counter-clockwise from positive X axis)
                    // Arc du haut: de 0° à 180° (demi-cercle supérieur)
                    // Top arc: from 0° to 180° (upper half-circle)
                    // Arc du bas: de 180° à 360° (demi-cercle inférieur)
                    // Bottom arc: from 180° to 360° (lower half-circle)
                    let (start_angle, end_angle) = if fg_mode {
                        (0.0_f64, 180.0_f64) // Top arc (foreground)
                    } else {
                        (180.0_f64, 360.0_f64) // Bottom arc (background)
                    };

                    // Ajoute l'arc au chemin
                    // Add the arc to the path
                    // appendBezierPathWithArcWithCenter:radius:startAngle:endAngle:clockwise:
                    // Note: Dans Cocoa, clockwise=NO signifie sens anti-horaire (sens mathématique positif)
                    // Note: In Cocoa, clockwise=NO means counter-clockwise (positive mathematical direction)
                    let _: () = msg_send![
                        &*arc_path,
                        appendBezierPathWithArcWithCenter: NSPoint::new(center_x, center_y), // Center point
                        radius: border_radius,    // Arc radius
                        startAngle: start_angle,  // Start angle in degrees
                        endAngle: end_angle,      // End angle in degrees
                        clockwise: Bool::NO       // Counter-clockwise direction
                    ];

                    arc_path.setLineWidth(BORDER_WIDTH); // Set the line width
                    arc_path.stroke(); // Draw the arc

                    // Crée la police système pour le texte
                    // Create system font for text
                    let font: Retained<NSFont> = NSFont::systemFontOfSize(HEX_FONT_SIZE);

                    // -------------------------------------------------------------
                    // Dessine le texte en arc pour l'arc de la couleur capturée (si existe)
                    // Draw arc text for the captured color arc (if exists)
                    // -------------------------------------------------------------
                    if is_continue_mode {
                        let (captured_color_for_text, captured_fg_mode_for_text) = if fg_mode {
                            (captured_bg, false) // On affiche bg en bas
                        } else {
                            (captured_fg, true) // On affiche fg en haut
                        };
                        
                        if let Some((cap_r, cap_g, cap_b)) = captured_color_for_text {
                            // Utilise format_labeled_hex_color du module common
                            // Uses format_labeled_hex_color from common module
                            let cap_label = if captured_fg_mode_for_text {
                                format_labeled_hex_color("Foreground", cap_r, cap_g, cap_b)
                            } else {
                                format_labeled_hex_color("Background", cap_r, cap_g, cap_b)
                            };
                            
                            // Couleur du texte basée sur la luminance de la couleur capturée
                            // Text color based on captured color luminance
                            // Utilise should_use_dark_text du module common
                            // Uses should_use_dark_text from common module
                            let cap_text_color = if should_use_dark_text(cap_r, cap_g, cap_b) {
                                NSColor::colorWithCalibratedRed_green_blue_alpha(0.0, 0.0, 0.0, 1.0)
                            } else {
                                NSColor::colorWithCalibratedRed_green_blue_alpha(1.0, 1.0, 1.0, 1.0)
                            };
                            
                            // Dessine le texte de la couleur capturée (sans badge C)
                            draw_arc_text(
                                &cap_label,
                                center_x, center_y,
                                border_radius,
                                captured_fg_mode_for_text,
                                &font,
                                &cap_text_color,
                                false, // Pas de badge C pour la couleur capturée
                            );
                        }
                    }

                    // -------------------------------------------------------------
                    // Dessine le texte hexadécimal en arc (haut ou bas selon fg_mode)
                    // Draw the hex text on arc (top or bottom based on fg_mode)
                    // -------------------------------------------------------------
                    // Couleur du texte basée sur la luminance
                    // Text color based on luminance
                    // Utilise should_use_dark_text du module common
                    // Uses should_use_dark_text from common module
                    let text_color = if should_use_dark_text(info.r, info.g, info.b) {
                        NSColor::colorWithCalibratedRed_green_blue_alpha(0.0, 0.0, 0.0, 1.0)
                    } else {
                        NSColor::colorWithCalibratedRed_green_blue_alpha(1.0, 1.0, 1.0, 1.0)
                    };

                    // Construit le texte avec label Foreground/Background
                    // Build text with Foreground/Background label
                    // Utilise format_labeled_hex_color du module common
                    // Uses format_labeled_hex_color from common module
                    let label = if fg_mode {
                        format_labeled_hex_color("Foreground", info.r, info.g, info.b)
                    } else {
                        format_labeled_hex_color("Background", info.r, info.g, info.b)
                    };
                    
                    // Dessine le texte de la couleur actuelle (avec badge C si mode continue)
                    draw_arc_text(
                        &label,
                        center_x, center_y,
                        border_radius,
                        fg_mode,
                        &font,
                        &text_color,
                        is_continue_mode, // Badge C si mode continue activé
                    );
                }
            }
        }
    }
}

/// Dessine du texte en arc autour d'un cercle
/// Draw text along an arc around a circle
///
/// # Arguments
/// * `text` - Le texte à dessiner
/// * `center_x`, `center_y` - Centre du cercle
/// * `radius` - Rayon de l'arc de texte
/// * `is_top_arc` - true pour arc du haut, false pour arc du bas
/// * `font` - Police à utiliser
/// * `text_color` - Couleur du texte
/// * `show_badge` - Afficher le badge "C" à la fin
fn draw_arc_text(
    text: &str,
    center_x: f64,
    center_y: f64,
    radius: f64,
    is_top_arc: bool,
    font: &NSFont,
    text_color: &NSColor,
    show_badge: bool,
) {
    use objc2_foundation::NSDictionary;
    use objc2::runtime::AnyObject;
    
    // Nombre de caractères + espace pour badge si nécessaire
    // Character count + space for badge if needed
    let badge_extra_chars = if show_badge { 2.0 } else { 0.0 };
    let char_count = text.len() as f64 + badge_extra_chars;
    
    // Calcule l'angle entre chaque caractère
    // Calculate angle between each character
    let angle_step = CHAR_SPACING_PIXELS / radius;
    
    // Arc total occupé par le texte
    // Total arc occupied by text
    let total_arc = angle_step * (char_count - 1.0);
    
    // Angle de départ selon l'arc (haut ou bas)
    // Start angle based on arc (top or bottom)
    let text_start_angle: f64 = if is_top_arc {
        std::f64::consts::PI / 2.0 + total_arc / 2.0
    } else {
        -std::f64::consts::PI / 2.0 - total_arc / 2.0
    };

    // Sauvegarde l'état graphique
    // Save graphics state
    NSGraphicsContext::saveGraphicsState_class();

    // Index de caractère courant
    // Current character index
    let mut char_index: f64 = 0.0;

    // Dessine chaque caractère du texte
    // Draw each character of the text
    for c in text.chars() {
        // Angle pour ce caractère
        // Angle for this character
        let angle = if is_top_arc {
            text_start_angle - angle_step * char_index
        } else {
            text_start_angle + angle_step * char_index
        };

        char_index += 1.0;

        // Position sur l'arc
        // Position on the arc
        let char_x = center_x + radius * angle.cos();
        let char_y = center_y + radius * angle.sin();

        // Convertit le caractère en NSString
        // Convert character to NSString
        let char_str = c.to_string();
        let ns_char = NSString::from_str(&char_str);

        // Crée le dictionnaire d'attributs pour le texte
        // Create the attribute dictionary for text
        let font_attr_key = NSString::from_str("NSFont");
        let color_attr_key = NSString::from_str("NSColor");
        let keys: &[&NSString] = &[&font_attr_key, &color_attr_key];
        let values: &[&AnyObject] = unsafe {
            &[
                &*(font as *const NSFont as *const AnyObject),
                &*(text_color as *const NSColor as *const AnyObject),
            ]
        };
        let attributes = NSDictionary::from_slices(keys, values);

        // Mesure la taille du caractère
        // Measure character size
        let char_size: NSSize = unsafe { ns_char.sizeWithAttributes(Some(&attributes)) };

        // Crée une transformation pour positionner et tourner le caractère
        // Create a transform to position and rotate the character
        let transform = NSAffineTransform::transform();
        transform.translateXBy_yBy(char_x, char_y);

        // Rotation selon l'arc
        // Rotation based on arc
        let rotation_angle = if is_top_arc {
            angle - std::f64::consts::PI / 2.0
        } else {
            angle + std::f64::consts::PI / 2.0
        };
        transform.rotateByRadians(rotation_angle);
        transform.concat();

        // Dessine le caractère centré
        // Draw the character centered
        let draw_point = NSPoint::new(-char_size.width / 2.0, -char_size.height / 2.0);
        unsafe { ns_char.drawAtPoint_withAttributes(draw_point, Some(&attributes)) };

        // Inverse la transformation
        // Invert the transform
        let inverse = transform.copy();
        inverse.invert();
        inverse.concat();
    }

    // Dessine le badge "C" à la fin si demandé
    // Draw the "C" badge at the end if requested
    if show_badge {
        // Avance d'un espace
        // Advance by one space
        char_index += 1.0;
        
        // Angle pour le badge (après le texte)
        // Angle for the badge (after the text)
        let badge_angle = if is_top_arc {
            text_start_angle - angle_step * char_index
        } else {
            text_start_angle + angle_step * char_index
        };

        // Position du badge sur l'arc
        // Badge position on the arc
        let badge_x = center_x + radius * badge_angle.cos();
        let badge_y = center_y + radius * badge_angle.sin();

        // Taille du badge
        // Badge size
        let badge_radius = HEX_FONT_SIZE * 0.7;

        // Dessine le cercle rouge de fond
        // Draw the red background circle
        let badge_rect = NSRect::new(
            NSPoint::new(badge_x - badge_radius, badge_y - badge_radius),
            NSSize::new(badge_radius * 2.0, badge_radius * 2.0)
        );
        let red_color = NSColor::colorWithCalibratedRed_green_blue_alpha(0.9, 0.1, 0.1, 1.0);
        red_color.setFill();
        let badge_circle = NSBezierPath::bezierPathWithOvalInRect(badge_rect);
        badge_circle.fill();

        // Dessine la lettre "C" en blanc
        // Draw the letter "C" in white
        let white_color = NSColor::colorWithCalibratedRed_green_blue_alpha(1.0, 1.0, 1.0, 1.0);
        let ns_c = NSString::from_str("C");

        let font_attr_key = NSString::from_str("NSFont");
        let color_attr_key = NSString::from_str("NSColor");
        let badge_keys: &[&NSString] = &[&font_attr_key, &color_attr_key];
        let badge_values: &[&AnyObject] = unsafe {
            &[
                &*(font as *const NSFont as *const AnyObject),
                &*(white_color.as_ref() as *const NSColor as *const AnyObject),
            ]
        };
        let badge_attributes = NSDictionary::from_slices(badge_keys, badge_values);

        let c_size: NSSize = unsafe { ns_c.sizeWithAttributes(Some(&badge_attributes)) };

        let badge_transform = NSAffineTransform::transform();
        badge_transform.translateXBy_yBy(badge_x, badge_y);

        let badge_rotation = if is_top_arc {
            badge_angle - std::f64::consts::PI / 2.0
        } else {
            badge_angle + std::f64::consts::PI / 2.0
        };
        badge_transform.rotateByRadians(badge_rotation);
        badge_transform.concat();

        let c_draw_point = NSPoint::new(-c_size.width / 2.0, -c_size.height / 2.0);
        unsafe { ns_c.drawAtPoint_withAttributes(c_draw_point, Some(&badge_attributes)) };

        let badge_inverse = badge_transform.copy();
        badge_inverse.invert();
        badge_inverse.concat();
    }

    // Restaure l'état graphique
    // Restore graphics state
    NSGraphicsContext::restoreGraphicsState_class();
}