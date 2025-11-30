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
pub fn run() {
    eprintln!("Windows support is not yet implemented.");
    std::process::exit(1);
}
