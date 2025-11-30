//! Windows implementation of the Color Picker
//!
//! This module will contain all Windows-specific code using Win32 API.
//! 
//! TODO: Implement using:
//! - windows-rs crate for Win32 API bindings
//! - GetDC / BitBlt for screen capture
//! - CreateWindowEx for overlay window
//! - SetLayeredWindowAttributes for transparency

/// Runs the color picker application on Windows
/// 
/// # Returns
/// * `Some((r, g, b))` - The selected RGB color if user clicked or pressed Enter
/// * `None` - If user pressed ESC to cancel
pub fn run() -> Option<(u8, u8, u8)> {
    eprintln!("Windows support is not yet implemented.");
    None
}
