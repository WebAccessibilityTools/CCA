//! CCA Color Picker
//!
//! This application creates a fullscreen overlay that captures the color
//! under the mouse cursor and displays a magnified view with the hex color value.
//!
//! Features:
//! - Circular magnifier following the cursor
//! - Colored border showing the current pixel color
//! - Hex color code displayed along the border arc
//! - Keyboard navigation (arrow keys, Shift+arrows for faster movement)
//! - Scroll wheel zoom control
//! - Click or ESC to exit and copy color
//! 
//! # Controls
//! - Mouse: Move to pick color
//! - Click: Exit and copy color
//! - ESC: Exit
//! - Arrow keys: Fine movement (1 pixel)
//! - Shift + Arrow keys: Fast movement (50 pixels)
//! - Scroll wheel: Zoom in/out


mod config;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

fn main() {
    #[cfg(target_os = "macos")]
    {
        macos::run();
    }

    #[cfg(target_os = "windows")]
    {
        windows::run();
    }

    #[cfg(target_os = "linux")]
    {
        linux::run();
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        eprintln!("Unsupported platform");
        std::process::exit(1);
    }
}