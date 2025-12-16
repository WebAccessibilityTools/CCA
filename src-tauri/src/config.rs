//! Configuration constants shared across all platforms
//!
//! These values control the appearance and behavior of the color picker.

/// Thickness of the colored border around the magnifier (in pixels)
/// This border displays the current color being picked
pub const BORDER_WIDTH: f64 = 20.0;

/// Font size for the hex color text displayed on the border (in points)
/// The text shows the hex value like "#FF5733"
pub const HEX_FONT_SIZE: f64 = 14.0;

/// Number of screen pixels captured by the magnifier
/// Must be ODD to have a single center pixel for the reticle
/// Smaller value = more zoom, larger value = less zoom
/// This determines how many pixels are visible in the magnifier
pub const CAPTURED_PIXELS: f64 = 11.0;

/// Default zoom factor for the magnifier
/// magnifier_size = CAPTURED_PIXELS * ZOOM_FACTOR
/// Example: 11 pixels * 20 = 220px magnifier diameter
pub const INITIAL_ZOOM_FACTOR: f64 = 20.0;

/// Number of pixels to move when pressing Shift + Arrow key
/// Regular arrow key moves 1 pixel, Shift+arrow moves this many
pub const SHIFT_MOVE_PIXELS: f64 = 50.0;

/// Minimum zoom factor (can't zoom out beyond this)
pub const ZOOM_MIN: f64 = 15.0;

/// Maximum zoom factor (can't zoom in beyond this)
pub const ZOOM_MAX: f64 = 50.0;

/// Zoom increment per scroll wheel step
/// Each scroll tick changes zoom by this amount
pub const ZOOM_STEP: f64 = 2.0;

/// Fixed spacing between characters in the hex text (in pixels)
/// This ensures consistent text appearance regardless of zoom level
pub const CHAR_SPACING_PIXELS: f64 = 12.0;

/// Default foreground color RGB value (black)
/// Valeur RGB par défaut pour la couleur de premier plan (noir)
pub const DEFAULT_FOREGROUND_RGB: (u8, u8, u8) = (0, 0, 0);

/// Default background color RGB value (white)
/// Valeur RGB par défaut pour la couleur d'arrière-plan (blanc)
pub const DEFAULT_BACKGROUND_RGB: (u8, u8, u8) = (255, 255, 255);
