// =============================================================================
// COLOR PICKER - VERSION WINDOWS
// =============================================================================
// Fenêtre plein écran affichant la capture d'écran + loupe
// Fullscreen window displaying screen capture + magnifier
// =============================================================================

// -----------------------------------------------------------------------------
// IMPORTS - Configuration
// -----------------------------------------------------------------------------
use crate::config::{
    BORDER_WIDTH,          // Épaisseur de la bordure colorée / Colored border thickness
    CAPTURED_PIXELS,       // Nombre de pixels capturés par défaut / Default captured pixels count
    INITIAL_ZOOM_FACTOR,   // Facteur de zoom initial / Initial zoom factor
    SHIFT_MOVE_PIXELS,     // Pixels de déplacement avec Shift / Pixels to move with Shift
    ZOOM_MIN,              // Zoom minimum / Minimum zoom
    ZOOM_MAX,              // Zoom maximum / Maximum zoom
    ZOOM_STEP,             // Incrément de zoom / Zoom increment
};

// -----------------------------------------------------------------------------
// IMPORTS - Types et fonctions communs
// IMPORTS - Common types and functions
// -----------------------------------------------------------------------------
use super::common::{
    ColorPickerResult,         // Structure de résultat avec FG/BG / Result structure with FG/BG
    should_use_dark_text,      // Détermine si texte noir ou blanc / Determines black or white text
    format_labeled_hex_color,  // Formate "Label - #RRGGBB" / Formats "Label - #RRGGBB"
};

// -----------------------------------------------------------------------------
// IMPORTS - Windows API
// -----------------------------------------------------------------------------
use windows::{
    core::*,                                    // Types de base Windows / Windows core types
    Win32::{
        Foundation::*,                          // Types fondamentaux (HWND, BOOL, etc.) / Fundamental types
        Graphics::Gdi::*,                       // GDI pour le dessin 2D / GDI for 2D drawing
        Graphics::GdiPlus,                      // GDI+ pour l'anti-aliasing / GDI+ for anti-aliasing
        System::LibraryLoader::GetModuleHandleW, // Handle du module courant / Current module handle
        UI::{
            Input::KeyboardAndMouse::*,         // Entrées clavier/souris / Keyboard/mouse input
            WindowsAndMessaging::*,             // Messages et fenêtres / Messages and windows
        },
    },
};

// -----------------------------------------------------------------------------
// IMPORTS - Bibliothèque standard Rust
// IMPORTS - Rust standard library
// -----------------------------------------------------------------------------
use std::sync::Mutex; // Mutex pour synchronisation thread-safe / Mutex for thread-safe sync

// =============================================================================
// CONSTANTES
// CONSTANTS
// =============================================================================

/// Nombre minimum de pixels capturés (zoom max)
/// Minimum captured pixels count (max zoom)
const CAPTURED_PIXELS_MIN: f64 = 9.0;

/// Nombre maximum de pixels capturés (zoom min)
/// Maximum captured pixels count (min zoom)
const CAPTURED_PIXELS_MAX: f64 = 21.0;

/// Incrément pour le nombre de pixels capturés
/// Increment for captured pixels count
const CAPTURED_PIXELS_STEP: f64 = 2.0;

/// Nom de la classe de fenêtre Windows
/// Windows window class name
const WINDOW_CLASS: &str = "ColorPickerFullscreen";

/// Identifiant du timer pour rafraîchissement
/// Timer ID for refresh
const TIMER_ID: usize = 1;

// -----------------------------------------------------------------------------
// Variables statiques globales
// Global static variables
// -----------------------------------------------------------------------------

/// Token GDI+ pour l'initialisation/fermeture
/// GDI+ token for initialization/shutdown
static GDIPLUS_TOKEN: Mutex<usize> = Mutex::new(0);

/// Handle de la fenêtre (stocké séparément car HWND n'est pas Send)
/// Window handle (stored separately because HWND is not Send)
static WINDOW_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

// =============================================================================
// ÉTAT GLOBAL
// GLOBAL STATE
// =============================================================================

/// État global du color picker protégé par Mutex
/// Global color picker state protected by Mutex
static STATE: Mutex<PickerState> = Mutex::new(PickerState::new());

/// Structure contenant l'état complet du color picker
/// Structure containing the complete color picker state
struct PickerState {
    cursor_x: i32,                      // Position X du curseur / Cursor X position
    cursor_y: i32,                      // Position Y du curseur / Cursor Y position
    color: (u8, u8, u8),                // Couleur sous le curseur (R, G, B) / Color under cursor
    fg_color: Option<(u8, u8, u8)>,     // Couleur FG sélectionnée / Selected FG color
    bg_color: Option<(u8, u8, u8)>,     // Couleur BG sélectionnée / Selected BG color
    fg_mode: bool,                      // true = mode FG, false = mode BG / true = FG mode, false = BG mode
    continue_mode: bool,                // Mode continue activé / Continue mode enabled
    zoom: f64,                          // Facteur de zoom actuel / Current zoom factor
    captured: f64,                      // Nombre de pixels capturés / Number of captured pixels
    quit: bool,                         // Flag pour quitter l'application / Flag to quit application
    screen_width: i32,                  // Largeur de l'écran en pixels / Screen width in pixels
    screen_height: i32,                 // Hauteur de l'écran en pixels / Screen height in pixels
}

/// Handle du bitmap de capture d'écran (doit être global pour WM_PAINT)
/// Screen capture bitmap handle (must be global for WM_PAINT)
static SCREEN_BITMAP: Mutex<Option<isize>> = Mutex::new(None);

/// Données brutes de l'écran capturé (BGRA)
/// Raw screen capture data (BGRA)
static SCREEN_DATA: Mutex<Vec<u8>> = Mutex::new(Vec::new());

// =============================================================================
// INITIALISATION GDI+
// GDI+ INITIALIZATION
// =============================================================================

/// Initialise GDI+ pour l'anti-aliasing et le dessin avancé
/// Initialize GDI+ for anti-aliasing and advanced drawing
fn init_gdiplus() {
    unsafe {
        let mut token: usize = 0;                              // Token retourné par GDI+ / Token returned by GDI+
        let input = GdiPlus::GdiplusStartupInput {
            GdiplusVersion: 1,                                 // Version de GDI+ (1 = standard) / GDI+ version
            DebugEventCallback: 0,                             // Pas de callback de debug / No debug callback (isize, not Option)
            SuppressBackgroundThread: FALSE,                   // Autoriser le thread de fond / Allow background thread
            SuppressExternalCodecs: FALSE,                     // Autoriser les codecs externes / Allow external codecs
        };
        
        // Démarre GDI+ et récupère le token
        // Start GDI+ and get the token
        let status = GdiPlus::GdiplusStartup(
            &mut token,                                        // Pointeur vers le token / Pointer to token
            &input,                                            // Paramètres d'entrée / Input parameters
            std::ptr::null_mut()                               // Pas de sortie / No output
        );
        
        // Si succès (Status == 0), sauvegarde le token
        // If success (Status == 0), save the token
        if status == GdiPlus::Status(0) {
            if let Ok(mut t) = GDIPLUS_TOKEN.lock() {
                *t = token;                                    // Stocke le token pour shutdown / Store token for shutdown
            }
        }
    }
}

/// Ferme GDI+ et libère les ressources
/// Shutdown GDI+ and release resources
fn shutdown_gdiplus() {
    unsafe {
        if let Ok(token) = GDIPLUS_TOKEN.lock() {
            if *token != 0 {                                   // Si GDI+ a été initialisé / If GDI+ was initialized
                GdiPlus::GdiplusShutdown(*token);              // Ferme GDI+ / Shutdown GDI+
            }
        }
    }
}

/// Implémentation de PickerState
/// PickerState implementation
impl PickerState {
    /// Crée un nouvel état avec les valeurs par défaut (const fn pour initialisation statique)
    /// Creates a new state with default values (const fn for static initialization)
    const fn new() -> Self {
        Self {
            cursor_x: 0,                           // Position initiale X / Initial X position
            cursor_y: 0,                           // Position initiale Y / Initial Y position
            color: (0, 0, 0),                      // Noir par défaut / Black by default
            fg_color: None,                        // Pas de FG sélectionné / No FG selected
            bg_color: None,                        // Pas de BG sélectionné / No BG selected
            fg_mode: true,                         // Commence en mode FG / Start in FG mode
            continue_mode: false,                  // Mode continue désactivé / Continue mode disabled
            zoom: INITIAL_ZOOM_FACTOR,             // Zoom initial depuis config / Initial zoom from config
            captured: CAPTURED_PIXELS,             // Pixels capturés depuis config / Captured pixels from config
            quit: false,                           // Ne pas quitter / Don't quit
            screen_width: 0,                       // Sera défini lors de la capture / Will be set during capture
            screen_height: 0,                      // Sera défini lors de la capture / Will be set during capture
        }
    }
    
    /// Réinitialise l'état à ses valeurs par défaut
    /// Resets state to default values
    fn reset(&mut self) {
        self.cursor_x = 0;                         // Réinitialise position X / Reset X position
        self.cursor_y = 0;                         // Réinitialise position Y / Reset Y position
        self.color = (0, 0, 0);                    // Réinitialise couleur / Reset color
        self.fg_color = None;                      // Efface FG sélectionné / Clear selected FG
        self.bg_color = None;                      // Efface BG sélectionné / Clear selected BG
        self.fg_mode = true;                       // Retour en mode FG / Back to FG mode
        self.continue_mode = false;                // Désactive mode continue / Disable continue mode
        self.zoom = INITIAL_ZOOM_FACTOR;           // Réinitialise zoom / Reset zoom
        self.captured = CAPTURED_PIXELS;           // Réinitialise pixels capturés / Reset captured pixels
        self.quit = false;                         // Ne pas quitter / Don't quit
    }
}

// =============================================================================
// CAPTURE D'ÉCRAN
// SCREEN CAPTURE
// =============================================================================

/// Capture l'écran entier dans un bitmap et extrait les données de pixels
/// Captures the entire screen into a bitmap and extracts pixel data
fn capture_screen() {
    unsafe {
        // Récupère les dimensions de l'écran principal
        // Get the main screen dimensions
        let width = GetSystemMetrics(SM_CXSCREEN);    // Largeur en pixels / Width in pixels
        let height = GetSystemMetrics(SM_CYSCREEN);   // Hauteur en pixels / Height in pixels
        
        // Crée des contextes de périphérique (DC) pour la copie
        // Create device contexts (DC) for copying
        let hdc_screen = GetDC(HWND::default());      // DC de l'écran / Screen DC
        let hdc_mem = CreateCompatibleDC(hdc_screen); // DC mémoire compatible / Compatible memory DC
        
        // Crée un bitmap compatible pour stocker la capture
        // Create a compatible bitmap to store the capture
        let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
        
        if !hbitmap.is_invalid() {
            // Sélectionne le bitmap dans le DC mémoire
            // Select the bitmap into the memory DC
            SelectObject(hdc_mem, hbitmap);
            
            // Copie l'écran dans le bitmap (BitBlt = Bit Block Transfer)
            // Copy the screen to the bitmap (BitBlt = Bit Block Transfer)
            let _ = BitBlt(hdc_mem, 0, 0, width, height, hdc_screen, 0, 0, SRCCOPY);
            
            // Stocke le handle du bitmap pour utilisation ultérieure
            // Store the bitmap handle for later use
            if let Ok(mut bmp) = SCREEN_BITMAP.lock() {
                *bmp = Some(hbitmap.0 as isize);       // Convertit HBITMAP en isize / Convert HBITMAP to isize
            }
            
            // Configure la structure BITMAPINFO pour extraire les données brutes
            // Configure BITMAPINFO structure to extract raw data
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32, // Taille de la structure / Structure size
                    biWidth: width,                    // Largeur du bitmap / Bitmap width
                    biHeight: -height,                 // Négatif = top-down (origine en haut à gauche) / Negative = top-down
                    biPlanes: 1,                       // Toujours 1 / Always 1
                    biBitCount: 32,                    // 32 bits par pixel (BGRA) / 32 bits per pixel (BGRA)
                    biCompression: BI_RGB.0,           // Pas de compression / No compression
                    ..Default::default()               // Reste à zéro / Rest zeroed
                },
                ..Default::default()
            };
            
            // Alloue un buffer pour les données de pixels (4 octets par pixel: BGRA)
            // Allocate buffer for pixel data (4 bytes per pixel: BGRA)
            let mut data: Vec<u8> = vec![0; (width * height * 4) as usize];
            
            // Extrait les données de pixels du bitmap
            // Extract pixel data from the bitmap
            let _ = GetDIBits(
                hdc_mem,                               // DC source / Source DC
                hbitmap,                               // Bitmap source / Source bitmap
                0,                                     // Première ligne / First scan line
                height as u32,                         // Nombre de lignes / Number of lines
                Some(data.as_mut_ptr() as *mut _),     // Buffer destination / Destination buffer
                &mut bmi,                              // Info du bitmap / Bitmap info
                DIB_RGB_COLORS,                        // Format RGB / RGB format
            );
            
            // Stocke les données de pixels pour lecture ultérieure
            // Store pixel data for later reading
            if let Ok(mut screen_data) = SCREEN_DATA.lock() {
                *screen_data = data;
            }
            
            // Sauvegarde les dimensions de l'écran dans l'état
            // Save screen dimensions in state
            if let Ok(mut state) = STATE.lock() {
                state.screen_width = width;
                state.screen_height = height;
            }
        }
        
        // Libère les ressources GDI
        // Release GDI resources
        let _ = DeleteDC(hdc_mem);                     // Supprime le DC mémoire / Delete memory DC
        let _ = ReleaseDC(HWND::default(), hdc_screen); // Libère le DC écran / Release screen DC
    }
}

/// Nettoie le bitmap de capture et libère la mémoire
/// Cleans up the capture bitmap and frees memory
fn cleanup_screen_bitmap() {
    // Supprime le bitmap si présent
    // Delete the bitmap if present
    if let Ok(mut bmp) = SCREEN_BITMAP.lock() {
        if let Some(h) = bmp.take() {                  // take() retire et retourne la valeur / takes and returns the value
            unsafe {
                let _ = DeleteObject(HBITMAP(h as *mut _)); // Supprime l'objet GDI / Delete GDI object
            }
        }
    }
    // Vide le buffer de données
    // Clear the data buffer
    if let Ok(mut data) = SCREEN_DATA.lock() {
        data.clear();                                  // Libère la mémoire / Free memory
    }
}

/// Récupère la couleur RGB du pixel aux coordonnées (x, y)
/// Gets the RGB color of the pixel at coordinates (x, y)
/// 
/// # Arguments
/// * `x` - Position X du pixel / Pixel X position
/// * `y` - Position Y du pixel / Pixel Y position
/// 
/// # Returns
/// Tuple (R, G, B) de la couleur du pixel / Tuple (R, G, B) of pixel color
fn get_pixel_color(x: i32, y: i32) -> (u8, u8, u8) {
    // Récupère les dimensions de l'écran
    // Get screen dimensions
    let (width, height) = {
        if let Ok(state) = STATE.lock() {
            (state.screen_width, state.screen_height)
        } else {
            return (0, 0, 0);                          // Noir si erreur / Black if error
        }
    };
    
    // Lit la couleur depuis les données capturées
    // Read color from captured data
    if let Ok(data) = SCREEN_DATA.lock() {
        // Vérifie que les coordonnées sont dans les limites
        // Check that coordinates are within bounds
        if x >= 0 && x < width && y >= 0 && y < height {
            // Calcule l'index dans le buffer (4 octets par pixel: BGRA)
            // Calculate index in buffer (4 bytes per pixel: BGRA)
            let idx = ((y * width + x) * 4) as usize;
            if idx + 2 < data.len() {
                let b = data[idx];                     // Bleu en premier (format BGRA) / Blue first (BGRA format)
                let g = data[idx + 1];                 // Vert ensuite / Green next
                let r = data[idx + 2];                 // Rouge en dernier / Red last
                return (r, g, b);                      // Retourne en ordre RGB / Return in RGB order
            }
        }
    }
    (0, 0, 0)                                          // Noir par défaut / Black by default
}

// =============================================================================
// MISE À JOUR DE LA POSITION
// POSITION UPDATE
// =============================================================================

/// Met à jour la position du curseur et la couleur correspondante
/// Updates cursor position and corresponding color
/// 
/// # Arguments
/// * `x` - Nouvelle position X / New X position
/// * `y` - Nouvelle position Y / New Y position
fn update_cursor_pos(x: i32, y: i32) {
    let color = get_pixel_color(x, y);                 // Récupère la couleur / Get color
    if let Ok(mut state) = STATE.lock() {
        state.cursor_x = x;                            // Met à jour X / Update X
        state.cursor_y = y;                            // Met à jour Y / Update Y
        state.color = color;                           // Met à jour la couleur / Update color
    }
}

// =============================================================================
// DESSIN DU TEXTE EN ARC
// CURVED TEXT DRAWING
// =============================================================================

/// Dessine du texte suivant un arc de cercle avec GDI+
/// Draws text following a circular arc with GDI+
/// 
/// Si `show_continue_badge` est true, dessine une pastille rouge avec "C" à la fin
/// If `show_continue_badge` is true, draws a red badge with "C" at the end
/// 
/// # Arguments
/// * `hdc` - Handle du contexte de périphérique / Device context handle
/// * `text` - Texte à dessiner / Text to draw
/// * `cx` - Centre X du cercle / Circle center X
/// * `cy` - Centre Y du cercle / Circle center Y
/// * `radius` - Rayon de l'arc de texte / Text arc radius
/// * `char_spacing` - Espacement entre caractères en pixels / Character spacing in pixels
/// * `upper` - true = arc supérieur, false = arc inférieur / true = upper arc, false = lower arc
/// * `color` - Couleur du texte (COLORREF) / Text color (COLORREF)
/// * `show_continue_badge` - Afficher la pastille "C" rouge / Show red "C" badge
fn draw_curved_text(
    hdc: HDC,                    // Handle du DC Windows / Windows DC handle
    text: &str,                  // Texte à afficher / Text to display
    cx: f64,                     // Centre X en pixels / Center X in pixels
    cy: f64,                     // Centre Y en pixels / Center Y in pixels
    radius: f64,                 // Rayon de l'arc / Arc radius
    char_spacing: f64,           // Espacement entre caractères / Character spacing
    upper: bool,                 // Arc supérieur ou inférieur / Upper or lower arc
    color: COLORREF,             // Couleur du texte / Text color
    show_continue_badge: bool,   // Afficher badge continue / Show continue badge
) {
    unsafe {
        // Crée un contexte graphique GDI+ à partir du HDC
        // Create a GDI+ graphics context from the HDC
        let mut graphics: *mut GdiPlus::GpGraphics = std::ptr::null_mut();
        if GdiPlus::GdipCreateFromHDC(hdc, &mut graphics) != GdiPlus::Status(0) {
            return; // Échec de création / Creation failed
        }
        
        // Active l'anti-aliasing pour un rendu de texte lisse
        // Enable anti-aliasing for smooth text rendering
        let _ = GdiPlus::GdipSetTextRenderingHint(graphics, GdiPlus::TextRenderingHint(3)); // AntiAlias
        let _ = GdiPlus::GdipSetSmoothingMode(graphics, GdiPlus::SmoothingMode(4));         // AntiAlias
        
        // Extrait les composantes RGB de COLORREF (format: 0x00BBGGRR)
        // Extract RGB components from COLORREF (format: 0x00BBGGRR)
        let r = (color.0 & 0xFF) as u8;              // Rouge dans les bits 0-7 / Red in bits 0-7
        let g = ((color.0 >> 8) & 0xFF) as u8;       // Vert dans les bits 8-15 / Green in bits 8-15
        let b = ((color.0 >> 16) & 0xFF) as u8;      // Bleu dans les bits 16-23 / Blue in bits 16-23
        
        // Convertit en format ARGB pour GDI+ (format: 0xAARRGGBB)
        // Convert to ARGB format for GDI+ (format: 0xAARRGGBB)
        let argb = 0xFF000000u32 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        
        // Crée une brosse de couleur unie pour le texte
        // Create a solid color brush for text
        let mut brush: *mut GdiPlus::GpBrush = std::ptr::null_mut();
        if GdiPlus::GdipCreateSolidFill(argb, &mut brush as *mut _ as *mut *mut GdiPlus::GpSolidFill) != GdiPlus::Status(0) {
            GdiPlus::GdipDeleteGraphics(graphics);   // Nettoie en cas d'erreur / Clean up on error
            return;
        }
        
        // Crée la famille de polices "Segoe UI"
        // Create "Segoe UI" font family
        let mut font_family: *mut GdiPlus::GpFontFamily = std::ptr::null_mut();
        let font_name: Vec<u16> = "Segoe UI".encode_utf16().chain(std::iter::once(0)).collect(); // UTF-16 + null
        let _ = GdiPlus::GdipCreateFontFamilyFromName(
            windows::core::PCWSTR(font_name.as_ptr()), // Nom de la police / Font name
            std::ptr::null_mut(),                      // Collection de polices (null = système) / Font collection
            &mut font_family                           // Pointeur de sortie / Output pointer
        );
        
        // Crée la police avec la taille spécifiée
        // Create the font with specified size
        let mut font: *mut GdiPlus::GpFont = std::ptr::null_mut();
        if !font_family.is_null() {
            let _ = GdiPlus::GdipCreateFont(
                font_family,                           // Famille de polices / Font family
                11.0,                                  // Taille en pixels / Size in pixels
                0,                                     // Style (0 = normal) / Style (0 = regular)
                GdiPlus::Unit(2),                      // Unité (2 = Pixel) / Unit (2 = Pixel)
                &mut font                              // Pointeur de sortie / Output pointer
            );
        }
        
        // Vérifie que la police a été créée avec succès
        // Check that font was created successfully
        if font.is_null() {
            GdiPlus::GdipDeleteBrush(brush);           // Libère la brosse / Free brush
            GdiPlus::GdipDeleteGraphics(graphics);     // Libère le contexte / Free context
            if !font_family.is_null() {
                GdiPlus::GdipDeleteFontFamily(font_family); // Libère la famille / Free family
            }
            return;
        }
        
        // Calcule le nombre de caractères (+ espace pour badge si nécessaire)
        // Calculate character count (+ space for badge if needed)
        let badge_space = if show_continue_badge { 2.0 } else { 0.0 }; // Espace pour la pastille / Space for badge
        let char_count = text.chars().count() as f64 + badge_space;
        let angle_step = char_spacing / radius;
        let total_arc = angle_step * (char_count - 1.0);
        
        // Pour chaque caractère
        // For each character
        for (i, c) in text.chars().enumerate() {
            let angle = if upper {
                // Arc supérieur: de gauche à droite, lettres debout
                // Upper arc: left to right, letters upright
                let start = std::f64::consts::FRAC_PI_2 + total_arc / 2.0;
                start - angle_step * (i as f64)
            } else {
                // Arc inférieur: de gauche à droite, lettres à l'envers
                // Lower arc: left to right, letters upside down
                let start = -std::f64::consts::FRAC_PI_2 - total_arc / 2.0;
                start + angle_step * (i as f64)
            };
            
            // Position sur le cercle
            // Position on circle
            let px = cx + radius * angle.cos();
            let py = cy - radius * angle.sin();
            
            // Angle de rotation pour la lettre
            // Rotation angle for the letter
            let rot_deg = if upper {
                // Haut: perpendiculaire au rayon, lettres vers l'extérieur
                // Top: perpendicular to radius, letters facing outward
                -(angle.to_degrees() - 90.0)
            } else {
                // Bas: perpendiculaire au rayon, lettres vers l'extérieur (donc inversées)
                // Bottom: perpendicular to radius, letters facing outward (so inverted)
                -(angle.to_degrees() + 90.0)
            };
            
            // Sauvegarde l'état, applique la transformation, dessine, restaure
            // Save state, apply transform, draw, restore
            let _ = GdiPlus::GdipSaveGraphics(graphics, &mut 0u32);
            
            // Translation au point, rotation, puis dessin centré
            // Translate to point, rotate, then draw centered
            let _ = GdiPlus::GdipTranslateWorldTransform(graphics, px as f32, py as f32, GdiPlus::MatrixOrder(0));
            let _ = GdiPlus::GdipRotateWorldTransform(graphics, rot_deg as f32, GdiPlus::MatrixOrder(0));
            
            // Mesure le caractère pour centrer
            // Measure character to center
            let char_str: Vec<u16> = c.to_string().encode_utf16().chain(std::iter::once(0)).collect();
            let mut bbox = GdiPlus::RectF { X: 0.0, Y: 0.0, Width: 0.0, Height: 0.0 };
            let layout_rect = GdiPlus::RectF { X: 0.0, Y: 0.0, Width: 100.0, Height: 100.0 };
            let _ = GdiPlus::GdipMeasureString(
                graphics,
                windows::core::PCWSTR(char_str.as_ptr()),
                1,
                font,
                &layout_rect,
                std::ptr::null_mut(),
                &mut bbox,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            );
            
            // Dessine le caractère centré
            // Draw character centered
            let draw_rect = GdiPlus::RectF {
                X: -bbox.Width / 2.0,
                Y: -bbox.Height / 2.0,
                Width: bbox.Width,
                Height: bbox.Height,
            };
            
            let _ = GdiPlus::GdipDrawString(
                graphics,
                windows::core::PCWSTR(char_str.as_ptr()),
                1,
                font,
                &draw_rect,
                std::ptr::null_mut(),
                brush
            );
            
            // Restaure la transformation
            // Restore transform
            let _ = GdiPlus::GdipResetWorldTransform(graphics);
        }
        
        // Dessine la pastille "C" si nécessaire
        // Draw "C" badge if needed
        if show_continue_badge {
            let text_len = text.chars().count() as f64;
            let badge_index = text_len + 1.0; // Position après le texte + espace / Position after text + space
            
            let angle = if upper {
                let start = std::f64::consts::FRAC_PI_2 + total_arc / 2.0;
                start - angle_step * badge_index
            } else {
                let start = -std::f64::consts::FRAC_PI_2 - total_arc / 2.0;
                start + angle_step * badge_index
            };
            
            let px = cx + radius * angle.cos();
            let py = cy - radius * angle.sin();
            
            let rot_deg = if upper {
                -(angle.to_degrees() - 90.0)
            } else {
                -(angle.to_degrees() + 90.0)
            };
            
            // Applique la transformation pour la pastille
            // Apply transform for badge
            let _ = GdiPlus::GdipTranslateWorldTransform(graphics, px as f32, py as f32, GdiPlus::MatrixOrder(0));
            let _ = GdiPlus::GdipRotateWorldTransform(graphics, rot_deg as f32, GdiPlus::MatrixOrder(0));
            
            // Dessine le cercle rouge
            // Draw red circle
            let badge_radius: f32 = 7.0;
            let mut red_brush: *mut GdiPlus::GpBrush = std::ptr::null_mut();
            let red_argb = 0xFFE63232u32; // Rouge / Red
            let _ = GdiPlus::GdipCreateSolidFill(red_argb, &mut red_brush as *mut _ as *mut *mut GdiPlus::GpSolidFill);
            
            if !red_brush.is_null() {
                let _ = GdiPlus::GdipFillEllipse(
                    graphics,
                    red_brush,
                    -badge_radius,
                    -badge_radius,
                    badge_radius * 2.0,
                    badge_radius * 2.0,
                );
                let _ = GdiPlus::GdipDeleteBrush(red_brush);
            }
            
            // Dessine le "C" en blanc
            // Draw "C" in white
            let mut white_brush: *mut GdiPlus::GpBrush = std::ptr::null_mut();
            let white_argb = 0xFFFFFFFFu32;
            let _ = GdiPlus::GdipCreateSolidFill(white_argb, &mut white_brush as *mut _ as *mut *mut GdiPlus::GpSolidFill);
            
            if !white_brush.is_null() {
                // Police plus petite pour le C
                // Smaller font for C
                let mut small_font: *mut GdiPlus::GpFont = std::ptr::null_mut();
                let _ = GdiPlus::GdipCreateFont(font_family, 9.0, 1, GdiPlus::Unit(2), &mut small_font); // Bold
                
                if !small_font.is_null() {
                    let c_str: Vec<u16> = "C".encode_utf16().chain(std::iter::once(0)).collect();
                    let mut c_bbox = GdiPlus::RectF { X: 0.0, Y: 0.0, Width: 0.0, Height: 0.0 };
                    let c_layout_rect = GdiPlus::RectF { X: 0.0, Y: 0.0, Width: 100.0, Height: 100.0 };
                    let _ = GdiPlus::GdipMeasureString(
                        graphics,
                        windows::core::PCWSTR(c_str.as_ptr()),
                        1,
                        small_font,
                        &c_layout_rect,
                        std::ptr::null_mut(),
                        &mut c_bbox,
                        std::ptr::null_mut(),
                        std::ptr::null_mut()
                    );
                    
                    let c_rect = GdiPlus::RectF {
                        X: -c_bbox.Width / 2.0,
                        Y: -c_bbox.Height / 2.0,
                        Width: c_bbox.Width,
                        Height: c_bbox.Height,
                    };
                    
                    let _ = GdiPlus::GdipDrawString(
                        graphics,
                        windows::core::PCWSTR(c_str.as_ptr()),
                        1,
                        small_font,
                        &c_rect,
                        std::ptr::null_mut(),
                        white_brush
                    );
                    
                    let _ = GdiPlus::GdipDeleteFont(small_font);
                }
                
                let _ = GdiPlus::GdipDeleteBrush(white_brush);
            }
            
            let _ = GdiPlus::GdipResetWorldTransform(graphics);
        }
        
        // Nettoyage
        // Cleanup
        GdiPlus::GdipDeleteFont(font);
        GdiPlus::GdipDeleteFontFamily(font_family);
        GdiPlus::GdipDeleteBrush(brush);
        GdiPlus::GdipDeleteGraphics(graphics);
    }
}

// =============================================================================
// DESSIN PRINCIPAL
// MAIN DRAWING
// =============================================================================

fn paint_window(_hwnd: HWND, hdc: HDC) {
    // Récupère l'état actuel / Get current state
    let (cursor_x, cursor_y, color, fg_color, bg_color, fg_mode, continue_mode, zoom, captured, 
         screen_width, screen_height) = {
        let state = match STATE.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        (
            state.cursor_x, state.cursor_y, state.color,
            state.fg_color, state.bg_color,
            state.fg_mode, state.continue_mode,
            state.zoom, state.captured,
            state.screen_width, state.screen_height,
        )
    };
    
    // Récupère les données de l'écran / Get screen data
    let screen_data = match SCREEN_DATA.lock() {
        Ok(d) => d.clone(),
        Err(_) => return,
    };
    
    if screen_data.is_empty() { return; }
    
    unsafe {
        // Crée un buffer double pour éviter le scintillement
        // Create a double buffer to avoid flickering
        let hdc_mem = CreateCompatibleDC(hdc);
        let hbitmap = CreateCompatibleBitmap(hdc, screen_width, screen_height);
        
        if hbitmap.is_invalid() {
            let _ = DeleteDC(hdc_mem);
            return;
        }
        
        SelectObject(hdc_mem, hbitmap);
        
        // Dessine le fond (capture d'écran)
        // Draw background (screen capture)
        if let Ok(bmp) = SCREEN_BITMAP.lock() {
            if let Some(h) = *bmp {
                let hdc_src = CreateCompatibleDC(hdc);
                SelectObject(hdc_src, HBITMAP(h as *mut _));
                let _ = BitBlt(hdc_mem, 0, 0, screen_width, screen_height, hdc_src, 0, 0, SRCCOPY);
                let _ = DeleteDC(hdc_src);
            }
        }
        
        // Paramètres de la loupe / Magnifier parameters
        let mag_size = (captured * zoom) as i32;
        let zoom_i = zoom as i32;
        let captured_i = captured as i32;
        let half_cap = captured_i / 2;
        let border_f = BORDER_WIDTH as f32;
        let cx_f = cursor_x as f32;
        let cy_f = cursor_y as f32;
        let inner_radius_f = mag_size as f32 / 2.0;
        let outer_radius_f = inner_radius_f + border_f;
        
        // Rayon intérieur des arcs réduit de 1px pour couvrir le bord du zoom
        // Inner radius of arcs reduced by 1px to cover the zoom edge
        let arc_inner_radius_f = inner_radius_f - 1.0;
        
        // =====================================================================
        // CALCUL DES COULEURS FG/BG
        // FG/BG COLOR CALCULATION
        // =====================================================================
        
        // Couleur pour l'arc FG (foreground)
        // Color for FG arc (foreground)
        // - Si mode FG actif: montre la couleur courante (sous le curseur)
        // - Sinon: montre la couleur FG sauvegardée (ou gris si pas encore capturée)
        // - If FG mode active: show current color (under cursor)
        // - Otherwise: show saved FG color (or gray if not captured yet)
        let (fg_r, fg_g, fg_b) = if fg_mode {
            color
        } else {
            fg_color.unwrap_or((128, 128, 128))
        };
        
        // Couleur pour l'arc BG (background)
        // Color for BG arc (background)
        // - Si mode BG actif: montre la couleur courante (sous le curseur)
        // - Sinon: montre la couleur BG sauvegardée (ou gris si pas encore capturée)
        // - If BG mode active: show current color (under cursor)
        // - Otherwise: show saved BG color (or gray if not captured yet)
        let (bg_r, bg_g, bg_b) = if !fg_mode {
            color
        } else {
            bg_color.unwrap_or((128, 128, 128))
        };
        
        // =====================================================================
        // CONTEXTE GDI+ PRINCIPAL
        // MAIN GDI+ CONTEXT
        // =====================================================================
        
        let mut graphics: *mut GdiPlus::GpGraphics = std::ptr::null_mut();
        let status = GdiPlus::GdipCreateFromHDC(hdc_mem, &mut graphics);
        
        if status == GdiPlus::Status(0) && !graphics.is_null() {
            // Active l'anti-aliasing
            // Enable anti-aliasing
            let _ = GdiPlus::GdipSetSmoothingMode(graphics, GdiPlus::SmoothingMode(4)); // AntiAlias
            
            // =================================================================
            // ÉTAPE 1: DESSINE LES PIXELS ZOOMÉS (avec clip circulaire)
            // STEP 1: DRAW ZOOMED PIXELS (with circular clip)
            // =================================================================
            
            // Crée un chemin circulaire pour le clip
            // Create a circular path for clipping
            let mut clip_path: *mut GdiPlus::GpPath = std::ptr::null_mut();
            let _ = GdiPlus::GdipCreatePath(GdiPlus::FillMode(0), &mut clip_path);
            
            if !clip_path.is_null() {
                // Cercle intérieur - même rayon que le bord intérieur des arcs
                // Inner circle - same radius as inner edge of arcs
                let _ = GdiPlus::GdipAddPathEllipse(
                    clip_path,
                    cx_f - inner_radius_f,
                    cy_f - inner_radius_f,
                    inner_radius_f * 2.0,
                    inner_radius_f * 2.0,
                );
                
                let _ = GdiPlus::GdipSetClipPath(graphics, clip_path, GdiPlus::CombineMode(0)); // Replace
                
                // Désactive l'anti-aliasing pour les pixels (évite les gaps)
                // Disable anti-aliasing for pixels (avoids gaps)
                let _ = GdiPlus::GdipSetSmoothingMode(graphics, GdiPlus::SmoothingMode(0)); // None
                let _ = GdiPlus::GdipSetPixelOffsetMode(graphics, GdiPlus::PixelOffsetMode(3)); // PixelOffsetModeHalf
                
                // Position de départ des pixels (entiers pour éviter les gaps)
                // Starting position of pixels (integers to avoid gaps)
                let start_x = (cx_f - inner_radius_f).floor() as i32;
                let start_y = (cy_f - inner_radius_f).floor() as i32;
                
                // Dessine chaque pixel zoomé
                // Draw each zoomed pixel
                for py in 0..captured_i {
                    for px in 0..captured_i {
                        let src_x = cursor_x - half_cap + px;
                        let src_y = cursor_y - half_cap + py;
                        
                        let (r, g, b) = if src_x >= 0 && src_x < screen_width && src_y >= 0 && src_y < screen_height {
                            let idx = ((src_y * screen_width + src_x) * 4) as usize;
                            if idx + 2 < screen_data.len() {
                                (screen_data[idx + 2], screen_data[idx + 1], screen_data[idx])
                            } else {
                                (128, 128, 128)
                            }
                        } else {
                            (64, 64, 64)
                        };
                        
                        // Position en entiers pour éviter les gaps entre pixels
                        // Integer position to avoid gaps between pixels
                        let dst_x = start_x + px * zoom_i;
                        let dst_y = start_y + py * zoom_i;
                        
                        // Crée une brosse pour ce pixel
                        // Create a brush for this pixel
                        let argb = 0xFF000000u32 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                        let mut pixel_brush: *mut GdiPlus::GpBrush = std::ptr::null_mut();
                        let _ = GdiPlus::GdipCreateSolidFill(argb, &mut pixel_brush as *mut _ as *mut *mut GdiPlus::GpSolidFill);
                        
                        if !pixel_brush.is_null() {
                            let _ = GdiPlus::GdipFillRectangleI(
                                graphics,
                                pixel_brush,
                                dst_x,
                                dst_y,
                                zoom_i,
                                zoom_i,
                            );
                            let _ = GdiPlus::GdipDeleteBrush(pixel_brush);
                        }
                    }
                }
                
                // Réactive l'anti-aliasing pour les arcs
                // Re-enable anti-aliasing for arcs
                let _ = GdiPlus::GdipSetSmoothingMode(graphics, GdiPlus::SmoothingMode(4)); // AntiAlias
                let _ = GdiPlus::GdipSetPixelOffsetMode(graphics, GdiPlus::PixelOffsetMode(0)); // Default
                
                // Réinitialise le clip
                // Reset clip
                let _ = GdiPlus::GdipResetClip(graphics);
                let _ = GdiPlus::GdipDeletePath(clip_path);
            }
            
            // =================================================================
            // ÉTAPE 2: DESSINE LES ARCS PAR-DESSUS (couvre les bords)
            // STEP 2: DRAW ARCS ON TOP (covers edges)
            // =================================================================
            
            // Détermine si chaque arc doit être visible
            // Determine if each arc should be visible
            // - Arc FG visible si: mode FG actif OU couleur FG déjà capturée
            // - Arc BG visible si: mode BG actif OU couleur BG déjà capturée
            // - FG arc visible if: FG mode active OR FG color already captured
            // - BG arc visible if: BG mode active OR BG color already captured
            let show_fg_arc = fg_mode || fg_color.is_some();
            let show_bg_arc = !fg_mode || bg_color.is_some();
            
            // Dessine l'arc supérieur (FG) avec anti-aliasing
            // Draw upper arc (FG) with anti-aliasing
            if show_fg_arc {
                let mut fg_brush_gdi: *mut GdiPlus::GpBrush = std::ptr::null_mut();
                let fg_argb = 0xFF000000u32 | ((fg_r as u32) << 16) | ((fg_g as u32) << 8) | (fg_b as u32);
                let _ = GdiPlus::GdipCreateSolidFill(fg_argb, &mut fg_brush_gdi as *mut _ as *mut *mut GdiPlus::GpSolidFill);
                
                if !fg_brush_gdi.is_null() {
                    // Crée un chemin pour l'arc supérieur (demi-anneau)
                    // Create a path for upper arc (half ring)
                    let mut path: *mut GdiPlus::GpPath = std::ptr::null_mut();
                    let _ = GdiPlus::GdipCreatePath(GdiPlus::FillMode(0), &mut path);
                    
                    if !path.is_null() {
                        // Arc extérieur (de 180° à 360°)
                        // Outer arc (from 180° to 360°)
                        let _ = GdiPlus::GdipAddPathArc(
                            path,
                            cx_f - outer_radius_f,
                            cy_f - outer_radius_f,
                            outer_radius_f * 2.0,
                            outer_radius_f * 2.0,
                            180.0,
                            180.0,
                        );
                        
                        // Arc intérieur (de 360° à 180°) - réduit de 1px
                        // Inner arc (from 360° to 180°) - reduced by 1px
                        let _ = GdiPlus::GdipAddPathArc(
                            path,
                            cx_f - arc_inner_radius_f,
                            cy_f - arc_inner_radius_f,
                            arc_inner_radius_f * 2.0,
                            arc_inner_radius_f * 2.0,
                            0.0,
                            -180.0,
                        );
                        
                        let _ = GdiPlus::GdipClosePathFigure(path);
                        let _ = GdiPlus::GdipFillPath(graphics, fg_brush_gdi, path);
                        let _ = GdiPlus::GdipDeletePath(path);
                    }
                    
                    let _ = GdiPlus::GdipDeleteBrush(fg_brush_gdi);
                }
            }
            
            // Dessine l'arc inférieur (BG) avec anti-aliasing
            // Draw lower arc (BG) with anti-aliasing
            if show_bg_arc {
                let mut bg_brush_gdi: *mut GdiPlus::GpBrush = std::ptr::null_mut();
                let bg_argb = 0xFF000000u32 | ((bg_r as u32) << 16) | ((bg_g as u32) << 8) | (bg_b as u32);
                let _ = GdiPlus::GdipCreateSolidFill(bg_argb, &mut bg_brush_gdi as *mut _ as *mut *mut GdiPlus::GpSolidFill);
                
                if !bg_brush_gdi.is_null() {
                    // Crée un chemin pour l'arc inférieur (demi-anneau)
                    // Create a path for lower arc (half ring)
                    let mut path: *mut GdiPlus::GpPath = std::ptr::null_mut();
                    let _ = GdiPlus::GdipCreatePath(GdiPlus::FillMode(0), &mut path);
                    
                    if !path.is_null() {
                        // Arc extérieur (de 0° à 180°)
                        // Outer arc (from 0° to 180°)
                        let _ = GdiPlus::GdipAddPathArc(
                            path,
                            cx_f - outer_radius_f,
                            cy_f - outer_radius_f,
                            outer_radius_f * 2.0,
                            outer_radius_f * 2.0,
                            0.0,
                            180.0,
                        );
                        
                        // Arc intérieur (de 180° à 0°) - réduit de 1px
                        // Inner arc (from 180° to 0°) - reduced by 1px
                        let _ = GdiPlus::GdipAddPathArc(
                            path,
                            cx_f - arc_inner_radius_f,
                            cy_f - arc_inner_radius_f,
                            arc_inner_radius_f * 2.0,
                            arc_inner_radius_f * 2.0,
                            180.0,
                            -180.0,
                        );
                        
                        let _ = GdiPlus::GdipClosePathFigure(path);
                        let _ = GdiPlus::GdipFillPath(graphics, bg_brush_gdi, path);
                        let _ = GdiPlus::GdipDeletePath(path);
                    }
                    
                    let _ = GdiPlus::GdipDeleteBrush(bg_brush_gdi);
                }
            }
            
            let _ = GdiPlus::GdipDeleteGraphics(graphics);
        }
        
        // =====================================================================
        // DESSIN DU RÉTICULE
        // DRAWING THE RETICLE
        // =====================================================================
        
        let ret_half = zoom_i / 2;
        let ret_x = cursor_x - ret_half;
        let ret_y = cursor_y - ret_half;
        let gray_pen = CreatePen(PS_SOLID, 1, COLORREF(0x606060));
        let old_pen = SelectObject(hdc_mem, gray_pen);
        let null_brush = GetStockObject(NULL_BRUSH);
        let old_brush = SelectObject(hdc_mem, null_brush);
        let _ = Rectangle(hdc_mem, ret_x, ret_y, ret_x + zoom_i, ret_y + zoom_i);
        let _ = SelectObject(hdc_mem, old_pen);
        let _ = SelectObject(hdc_mem, old_brush);
        let _ = DeleteObject(gray_pen);
        
        // Rayon de l'arc de texte (milieu de la bordure)
        // Text arc radius (middle of border)
        let text_radius = (arc_inner_radius_f + outer_radius_f) as f64 / 2.0;
        let char_spacing = 8.0_f64; // Espacement entre caractères / Character spacing
        
        // Détermine si chaque arc doit être visible (même logique que pour les arcs)
        // Determine if each arc should be visible (same logic as for arcs)
        let show_fg_arc = fg_mode || fg_color.is_some();
        let show_bg_arc = !fg_mode || bg_color.is_some();
        
        // =====================================================================
        // TEXTE FG EN ARC SUPÉRIEUR (COURBÉ)
        // FG TEXT IN UPPER ARC (CURVED)
        // =====================================================================
        
        if show_fg_arc {
            // Utilise format_labeled_hex_color du module common
            // Uses format_labeled_hex_color from common module
            let fg_hex = format_labeled_hex_color("Foreground", fg_r, fg_g, fg_b);
            // Utilise should_use_dark_text du module common
            // Uses should_use_dark_text from common module
            let fg_text_color = if should_use_dark_text(fg_r, fg_g, fg_b) { COLORREF(0) } else { COLORREF(0xFFFFFF) };
            
            // Affiche la pastille (C) si mode continue actif et mode FG
            // Show (C) badge if continue mode active and FG mode
            draw_curved_text(
                hdc_mem,
                &fg_hex,
                cx_f as f64,
                cy_f as f64,
                text_radius,
                char_spacing,
                true, // Arc supérieur / Upper arc
                fg_text_color,
                continue_mode && fg_mode, // Pastille continue / Continue badge
            );
        }
        
        // =====================================================================
        // TEXTE BG EN ARC INFÉRIEUR (COURBÉ)
        // BG TEXT IN LOWER ARC (CURVED)
        // =====================================================================
        
        if show_bg_arc {
            // Utilise format_labeled_hex_color du module common
            // Uses format_labeled_hex_color from common module
            let bg_hex = format_labeled_hex_color("Background", bg_r, bg_g, bg_b);
            // Utilise should_use_dark_text du module common
            // Uses should_use_dark_text from common module
            let bg_text_color = if should_use_dark_text(bg_r, bg_g, bg_b) { COLORREF(0) } else { COLORREF(0xFFFFFF) };
            
            // Affiche la pastille (C) si mode continue actif et mode BG
            // Show (C) badge if continue mode active and BG mode
            draw_curved_text(
                hdc_mem,
                &bg_hex,
                cx_f as f64,
                cy_f as f64,
                text_radius,
                char_spacing,
                false, // Arc inférieur / Lower arc
                bg_text_color,
                continue_mode && !fg_mode, // Pastille continue / Continue badge
            );
        }
        
        // Copie vers l'écran / Copy to screen
        let _ = BitBlt(hdc, 0, 0, screen_width, screen_height, hdc_mem, 0, 0, SRCCOPY);
        
        let _ = DeleteObject(hbitmap);
        let _ = DeleteDC(hdc_mem);
    }
}

// =============================================================================
// ÉVÉNEMENTS
// =============================================================================

fn handle_key(hwnd: HWND, vk: VIRTUAL_KEY) {
    let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) < 0 };
    
    match vk {
        VK_ESCAPE => {
            if let Ok(mut state) = STATE.lock() {
                state.quit = true;
            }
            unsafe { PostQuitMessage(0); }
        }
        VK_RETURN | VK_SPACE => select_color(),
        VK_C => {
            if let Ok(mut state) = STATE.lock() {
                state.continue_mode = !state.continue_mode;
            }
            unsafe { let _ = InvalidateRect(hwnd, None, FALSE); }
        }
        VK_I => {
            if let Ok(mut state) = STATE.lock() {
                if shift {
                    state.captured = (state.captured + CAPTURED_PIXELS_STEP).min(CAPTURED_PIXELS_MAX);
                } else {
                    state.zoom = (state.zoom + ZOOM_STEP).min(ZOOM_MAX);
                }
            }
            unsafe { let _ = InvalidateRect(hwnd, None, FALSE); }
        }
        VK_O => {
            if let Ok(mut state) = STATE.lock() {
                if shift {
                    state.captured = (state.captured - CAPTURED_PIXELS_STEP).max(CAPTURED_PIXELS_MIN);
                } else {
                    state.zoom = (state.zoom - ZOOM_STEP).max(ZOOM_MIN);
                }
            }
            unsafe { let _ = InvalidateRect(hwnd, None, FALSE); }
        }
        VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN => {
            let amt = if shift { SHIFT_MOVE_PIXELS as i32 } else { 1 };
            unsafe {
                let mut pt = POINT::default();
                let _ = GetCursorPos(&mut pt);
                match vk {
                    VK_LEFT => pt.x -= amt,
                    VK_RIGHT => pt.x += amt,
                    VK_UP => pt.y -= amt,
                    VK_DOWN => pt.y += amt,
                    _ => {}
                }
                let _ = SetCursorPos(pt.x, pt.y);
                update_cursor_pos(pt.x, pt.y);
                let _ = InvalidateRect(hwnd, None, FALSE);
            }
        }
        _ => {}
    }
}

fn handle_wheel(hwnd: HWND, delta: i16) {
    let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) < 0 };
    let up = delta > 0;
    
    if let Ok(mut state) = STATE.lock() {
        if shift {
            if up {
                state.captured = (state.captured + CAPTURED_PIXELS_STEP).min(CAPTURED_PIXELS_MAX);
            } else {
                state.captured = (state.captured - CAPTURED_PIXELS_STEP).max(CAPTURED_PIXELS_MIN);
            }
        } else {
            if up {
                state.zoom = (state.zoom + ZOOM_STEP).min(ZOOM_MAX);
            } else {
                state.zoom = (state.zoom - ZOOM_STEP).max(ZOOM_MIN);
            }
        }
    }
    unsafe { let _ = InvalidateRect(hwnd, None, FALSE); }
}

fn select_color() {
    let should_quit;
    
    if let Ok(mut state) = STATE.lock() {
        let color = state.color;
        
        if state.continue_mode {
            let has_other = if state.fg_mode {
                state.bg_color.is_some()
            } else {
                state.fg_color.is_some()
            };
            
            if state.fg_mode {
                state.fg_color = Some(color);
            } else {
                state.bg_color = Some(color);
            }
            
            if has_other {
                state.quit = true;
                should_quit = true;
            } else {
                // Passe à l'autre mode / Switch to other mode
                state.fg_mode = !state.fg_mode;
                should_quit = false;
            }
        } else {
            if state.fg_mode {
                state.fg_color = Some(color);
            } else {
                state.bg_color = Some(color);
            }
            state.quit = true;
            should_quit = true;
        }
    } else {
        return;
    }
    
    if should_quit {
        unsafe { PostQuitMessage(0); }
    } else {
        // Force le redessin pour montrer la couleur capturée
        // Force redraw to show captured color
        let hwnd_ptr = WINDOW_HWND.load(std::sync::atomic::Ordering::SeqCst);
        if hwnd_ptr != 0 {
            let hwnd = HWND(hwnd_ptr as *mut std::ffi::c_void);
            unsafe { 
                let _ = InvalidateRect(hwnd, None, FALSE); 
            }
        }
    }
}

// =============================================================================
// WINDOW PROCEDURE
// =============================================================================

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let _ = ShowCursor(false);
                let _ = SetTimer(hwnd, TIMER_ID, 16, None);
                LRESULT(0)
            }
            WM_DESTROY => {
                let _ = ShowCursor(true);
                let _ = KillTimer(hwnd, TIMER_ID);
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                paint_window(hwnd, hdc);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_TIMER => {
                let mut pt = POINT::default();
                let _ = GetCursorPos(&mut pt);
                update_cursor_pos(pt.x, pt.y);
                let _ = InvalidateRect(hwnd, None, FALSE);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let x = (lp.0 & 0xFFFF) as i16 as i32;
                let y = ((lp.0 >> 16) & 0xFFFF) as i16 as i32;
                update_cursor_pos(x, y);
                let _ = InvalidateRect(hwnd, None, FALSE);
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                select_color();
                LRESULT(0)
            }
            WM_RBUTTONDOWN => {
                if let Ok(mut state) = STATE.lock() {
                    state.quit = true;
                }
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_KEYDOWN => {
                handle_key(hwnd, VIRTUAL_KEY(wp.0 as u16));
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let delta = ((wp.0 >> 16) & 0xFFFF) as i16;
                handle_wheel(hwnd, delta);
                LRESULT(0)
            }
            WM_ERASEBKGND => {
                // Ne pas effacer le fond (évite le scintillement)
                LRESULT(1)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp)
        }
    }
}

// =============================================================================
// API PUBLIQUE
// =============================================================================

pub fn run(fg: bool) -> ColorPickerResult {
    if let Ok(mut state) = STATE.lock() {
        state.reset();
        state.fg_mode = fg;
    }
    
    // Initialise GDI+ pour l'anti-aliasing
    // Initialize GDI+ for anti-aliasing
    init_gdiplus();
    
    // Capture l'écran AVANT de créer la fenêtre
    // Capture screen BEFORE creating window
    capture_screen();
    
    unsafe {
        let hinst = GetModuleHandleW(None).unwrap();
        let class_wide: Vec<u16> = WINDOW_CLASS.encode_utf16().chain(std::iter::once(0)).collect();
        let class_name = PCWSTR(class_wide.as_ptr());
        
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst.into(),
            hCursor: HCURSOR::default(),
            lpszClassName: class_name,
            ..Default::default()
        };
        
        if RegisterClassExW(&wc) == 0 {
            cleanup_screen_bitmap();
            shutdown_gdiplus();
            return ColorPickerResult { foreground: None, background: None, continue_mode: false };
        }
        
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);
        
        // Fenêtre plein écran, toujours au-dessus
        // Fullscreen window, always on top
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            class_name,
            w!(""),
            WS_POPUP,
            0, 0, screen_width, screen_height,
            None, None, hinst, None,
        );
        
        if hwnd.is_err() {
            let _ = UnregisterClassW(class_name, hinst);
            cleanup_screen_bitmap();
            shutdown_gdiplus();
            return ColorPickerResult { foreground: None, background: None, continue_mode: false };
        }
        
        let hwnd = hwnd.unwrap();
        
        // Sauvegarde le handle de la fenêtre
        // Save window handle
        WINDOW_HWND.store(hwnd.0 as isize, std::sync::atomic::Ordering::SeqCst);
        
        // Position initiale / Initial position
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        update_cursor_pos(pt.x, pt.y);
        
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(hwnd);
        let _ = SetCapture(hwnd);
        
        // Boucle de messages / Message loop
        let mut msg = MSG::default();
        loop {
            let quit = STATE.lock().map(|s| s.quit).unwrap_or(false);
            if quit { break; }
            
            if GetMessageW(&mut msg, HWND::default(), 0, 0).0 <= 0 {
                break;
            }
            
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        
        let _ = ReleaseCapture();
        let _ = DestroyWindow(hwnd);
        let _ = UnregisterClassW(class_name, hinst);
    }
    
    cleanup_screen_bitmap();
    
    // Ferme GDI+ / Shutdown GDI+
    shutdown_gdiplus();
    
    if let Ok(state) = STATE.lock() {
        ColorPickerResult {
            foreground: state.fg_color,
            background: state.bg_color,
            continue_mode: state.continue_mode,
        }
    } else {
        ColorPickerResult { foreground: None, background: None, continue_mode: false }
    }
}