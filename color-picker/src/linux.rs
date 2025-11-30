//! Linux implementation of the Color Picker
//!
//! This module will contain all Linux-specific code.
//! 
//! TODO: Implement using:
//! - X11: x11rb or xcb crate for X11 protocol
//! - Wayland: wayland-client crate (note: screen capture is restricted on Wayland)
//! - GTK: gtk-rs for cross-desktop support
//! - XGetImage for screen capture on X11

/// Runs the color picker application on Linux
pub fn run() {
    eprintln!("Linux support is not yet implemented.");
    std::process::exit(1);
}
