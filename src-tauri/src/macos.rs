//! macOS implementation of the Color Picker
//!
//! This module contains all macOS-specific code using Cocoa and Core Graphics.
//! It creates a fullscreen overlay window that captures the screen and displays
//! a magnified view of the pixels around the cursor.

// =============================================================================
// IMPORTS
// =============================================================================

// Cocoa framework bindings for macOS GUI
use cocoa::base::{id, nil, NO, YES};  // Basic Objective-C types
use cocoa::foundation::{
    NSAutoreleasePool,  // Memory management for Objective-C objects
    NSRect,             // Rectangle type (origin + size)
    NSArray,            // Objective-C array type
    NSPoint,            // Point type (x, y coordinates)
    NSString,           // Objective-C string type
};
use cocoa::appkit::{
    NSApp,                              // Global application instance
    NSApplication,                       // Application class
    NSBackingStoreBuffered,              // Double-buffered window rendering
    NSWindow,                            // Window class
    NSWindowStyleMask,                   // Window style options (borderless, etc.)
    NSRunningApplication,                // Info about running applications
    NSApplicationActivateIgnoringOtherApps, // Force activation flag
    NSScreen,                            // Screen/display information
};

// Objective-C runtime bindings for low-level messaging
use objc::{class, msg_send, sel, sel_impl};  // Macros for Objective-C calls
use objc::declare::ClassDecl;                 // For creating custom Objective-C classes
use objc::runtime::{Object, Sel};             // Runtime types for method dispatch

// Core Graphics for screen capture and pixel color extraction
use core_graphics::display::CGDisplay;  // Display/screen functions
use core_graphics::image::CGImage;      // Image type for screen captures

// Standard library imports
use std::sync::Mutex;  // Thread-safe mutex for shared state

// Import shared configuration constants from config module
use crate::config::*;

// =============================================================================
// GLOBAL STATE
// =============================================================================

/// Global mutex-protected state for mouse position and color information
/// This allows the draw function to access data set by mouse event handlers
/// Uses Mutex for thread-safe access from multiple event callbacks
static MOUSE_STATE: Mutex<Option<MouseColorInfo>> = Mutex::new(None);

/// Global mutex-protected state for current zoom level
/// Persists zoom changes from scroll wheel across redraws
static CURRENT_ZOOM: Mutex<f64> = Mutex::new(INITIAL_ZOOM_FACTOR);

/// Stores the final selected color when user clicks or presses Enter
/// None if user cancelled with ESC, Some((r,g,b)) if color was selected
static SELECTED_COLOR: Mutex<Option<(u8, u8, u8)>> = Mutex::new(None);

/// Flag to signal the picker should exit its event loop
static SHOULD_EXIT: Mutex<bool> = Mutex::new(false);

/// Store window addresses so we can close them when done
/// We store as usize (raw addresses) because id is not Send/Sync
static PICKER_WINDOWS: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// Structure containing all information about current mouse position and color
/// Updated on every mouse move event
struct MouseColorInfo {
    x: f64,           // X position in window coordinates (for drawing the magnifier)
    y: f64,           // Y position in window coordinates (for drawing the magnifier)
    screen_x: f64,    // X position in screen coordinates (for screen capture)
    screen_y: f64,    // Y position in screen coordinates (for screen capture)
    r: u8,            // Red component of pixel color (0-255)
    g: u8,            // Green component of pixel color (0-255)
    b: u8,            // Blue component of pixel color (0-255)
    hex_color: String, // Hex color string like "#FF5733"
    scale_factor: f64, // Screen scale factor (1.0 = standard, 2.0 = Retina)
}

// =============================================================================
// SCREEN CAPTURE FUNCTIONS
// =============================================================================

/// Captures a square area of pixels around the given screen coordinates
fn capture_zoom_area(x: f64, y: f64, size: f64) -> Option<CGImage> {
    use core_graphics::geometry::{CGRect, CGPoint as CGPointStruct, CGSize};

    let main_display = CGDisplay::main();
    let screen_height = main_display.pixels_high() as f64;
    let cg_y = screen_height - y;

    let center_x = x.round();
    let center_y = cg_y.round();
    let capture_size = size.round();
    let half_size = (capture_size / 2.0).floor();
    
    let rect = CGRect::new(
        &CGPointStruct::new(center_x - half_size, center_y - half_size),
        &CGSize::new(capture_size, capture_size)
    );

    main_display.image_for_rect(rect)
}

/// Captures the color of a single pixel at the given screen coordinates
fn get_pixel_color(x: f64, y: f64) -> Option<(f64, f64, f64)> {
    use core_graphics::geometry::{CGRect, CGPoint as CGPointStruct, CGSize};

    let main_display = CGDisplay::main();
    let screen_height = main_display.pixels_high() as f64;
    let cg_y = screen_height - y;

    let center_x = x.round();
    let center_y = cg_y.round();

    let rect = CGRect::new(
        &CGPointStruct::new(center_x, center_y),
        &CGSize::new(1.0, 1.0)
    );

    let image = main_display.image_for_rect(rect)?;
    let data = image.data();
    let data_len = data.len() as usize;

    if data_len >= 4 {
        let b = data[0] as f64 / 255.0;
        let g = data[1] as f64 / 255.0;
        let r = data[2] as f64 / 255.0;
        Some((r, g, b))
    } else {
        None
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Signals the picker to exit and shows the cursor
/// This does NOT call app.stop() which would kill the entire Tauri app
fn stop_picker() {
    unsafe {
        // Show the cursor again
        let _: () = msg_send![class!(NSCursor), unhide];
    }
    
    // Signal the event loop to exit
    if let Ok(mut should_exit) = SHOULD_EXIT.lock() {
        *should_exit = true;
    }
}

// =============================================================================
// PUBLIC API
// =============================================================================

/// Runs the color picker on macOS
/// Returns the selected RGB color or None if cancelled
pub fn run() -> Option<(u8, u8, u8)> {
    // Reset all state from any previous run
    if let Ok(mut color) = SELECTED_COLOR.lock() {
        *color = None;
    }
    if let Ok(mut should_exit) = SHOULD_EXIT.lock() {
        *should_exit = false;
    }
    if let Ok(mut windows) = PICKER_WINDOWS.lock() {
        windows.clear();
    }
    if let Ok(mut zoom) = CURRENT_ZOOM.lock() {
        *zoom = INITIAL_ZOOM_FACTOR;
    }
    
    unsafe {
        // Create an autorelease pool for Objective-C memory management
        let _pool = NSAutoreleasePool::new(nil);

        // Get the shared application instance
        let app = NSApp();

        // Register our custom view and window classes
        let view_class = register_view_class();
        let window_class = register_window_class();

        // Get all available screens (for multi-monitor support)
        let screens = NSScreen::screens(nil);
        let count = screens.count();

        // Create an overlay window for each screen
        for i in 0..count {
            let screen = screens.objectAtIndex(i);
            let frame: NSRect = msg_send![screen, frame];

            // Allocate and initialize a new window using our custom class
            let window_alloc: id = msg_send![window_class, alloc];
            let window = window_alloc.initWithContentRect_styleMask_backing_defer_(
                frame,
                NSWindowStyleMask::NSBorderlessWindowMask,
                NSBackingStoreBuffered,
                NO
            );

            // Store window reference for cleanup (as raw address)
            if let Ok(mut windows) = PICKER_WINDOWS.lock() {
                windows.push(window as usize);
            }

            // Set window level very high so it appears above everything
            window.setLevel_(1000);

            // Create a fully transparent color for the background
            let cls_color = class!(NSColor);
            let clear_color: id = msg_send![cls_color, clearColor];

            // Configure window to be transparent and non-opaque
            window.setBackgroundColor_(clear_color);
            window.setOpaque_(NO);
            window.setHasShadow_(NO);
            window.setIgnoresMouseEvents_(NO);
            window.setAcceptsMouseMovedEvents_(YES);

            // Prevent window from being captured in screenshots/recordings
            let _: () = msg_send![window, setSharingType: 0u64];

            // Create our custom view and set it as the window's content
            let view: id = msg_send![view_class, alloc];
            let view: id = msg_send![view, initWithFrame:frame];

            window.setContentView_(view);
            window.makeKeyAndOrderFront_(nil);
            let _: () = msg_send![window, makeFirstResponder: view];
        }

        // Activate our application and bring it to the foreground
        let current_app = NSRunningApplication::currentApplication(nil);
        current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);

        // Hide the system cursor while the color picker is active
        let _: () = msg_send![class!(NSCursor), hide];

        // Run a custom event loop instead of app.run()
        // This allows us to exit without stopping the entire Tauri application
        let distant_future: id = msg_send![class!(NSDate), distantFuture];
        let default_mode = NSString::alloc(nil).init_str("kCFRunLoopDefaultMode");
        
        loop {
            // Check if we should exit
            if let Ok(should_exit) = SHOULD_EXIT.lock() {
                if *should_exit {
                    break;
                }
            }
            
            // Process one event with a short timeout
            let event: id = msg_send![app, 
                nextEventMatchingMask: u64::MAX
                untilDate: distant_future
                inMode: default_mode
                dequeue: YES
            ];
            
            if !event.is_null() {
                let _: () = msg_send![app, sendEvent: event];
            }
            
            // Check again after processing the event
            if let Ok(should_exit) = SHOULD_EXIT.lock() {
                if *should_exit {
                    break;
                }
            }
        }
        
        // Close all picker windows
        if let Ok(windows) = PICKER_WINDOWS.lock() {
            for window_addr in windows.iter() {
                let window = *window_addr as id;
                let _: () = msg_send![window, close];
            }
        }
        
        // Clear windows list
        if let Ok(mut windows) = PICKER_WINDOWS.lock() {
            windows.clear();
        }
    }
    
    // Return the selected color (if any)
    if let Ok(color) = SELECTED_COLOR.lock() {
        color.clone()
    } else {
        None
    }
}

// =============================================================================
// CUSTOM CLASS REGISTRATION
// =============================================================================

/// Registers a custom NSView subclass for handling drawing and events
/// Returns existing class if already registered
fn register_view_class() -> &'static objc::runtime::Class {
    // Check if class already exists
    if let Some(cls) = objc::runtime::Class::get("ColorPickerView") {
        return cls;
    }
    
    let superclass = class!(NSView);
    let mut decl = ClassDecl::new("ColorPickerView", superclass).unwrap();

    unsafe {
        decl.add_method(
            sel!(acceptsFirstResponder),
            accepts_first_responder as extern "C" fn(&Object, Sel) -> bool
        );
        decl.add_method(
            sel!(mouseDown:),
            mouse_down as extern "C" fn(&Object, Sel, id)
        );
        decl.add_method(
            sel!(mouseMoved:),
            mouse_moved as extern "C" fn(&Object, Sel, id)
        );
        decl.add_method(
            sel!(scrollWheel:),
            scroll_wheel as extern "C" fn(&Object, Sel, id)
        );
        decl.add_method(
            sel!(keyDown:),
            key_down as extern "C" fn(&Object, Sel, id)
        );
        decl.add_method(
            sel!(drawRect:),
            draw_rect as extern "C" fn(&Object, Sel, NSRect)
        );
    }

    decl.register()
}

/// Registers a custom NSWindow subclass that can become key window
/// Returns existing class if already registered
fn register_window_class() -> &'static objc::runtime::Class {
    // Check if class already exists
    if let Some(cls) = objc::runtime::Class::get("KeyableWindow") {
        return cls;
    }
    
    let superclass = class!(NSWindow);
    let mut decl = ClassDecl::new("KeyableWindow", superclass).unwrap();

    unsafe {
        decl.add_method(
            sel!(canBecomeKeyWindow),
            can_become_key_window as extern "C" fn(&Object, Sel) -> bool
        );
    }

    decl.register()
}

// =============================================================================
// OBJECTIVE-C METHOD IMPLEMENTATIONS
// =============================================================================

extern "C" fn can_become_key_window(_this: &Object, _cmd: Sel) -> bool {
    true
}

extern "C" fn accepts_first_responder(_this: &Object, _cmd: Sel) -> bool {
    true
}

/// Mouse click - save color and exit picker
extern "C" fn mouse_down(_this: &Object, _cmd: Sel, _event: id) {
    // Save the current color before exiting
    if let Ok(state) = MOUSE_STATE.lock() {
        if let Some(ref info) = *state {
            if let Ok(mut selected) = SELECTED_COLOR.lock() {
                *selected = Some((info.r, info.g, info.b));
            }
        }
    }
    
    stop_picker();
}

/// Mouse movement - update color and redraw
extern "C" fn mouse_moved(_this: &Object, _cmd: Sel, event: id) {
    unsafe {
        let location: NSPoint = msg_send![event, locationInWindow];
        let window: id = msg_send![_this, window];
        let screen_location: NSPoint = msg_send![window, convertPointToScreen: location];

        if let Some((r, g, b)) = get_pixel_color(screen_location.x as f64, screen_location.y as f64) {
            let r_int = (r * 255.0) as u8;
            let g_int = (g * 255.0) as u8;
            let b_int = (b * 255.0) as u8;

            let hex_color = format!("#{:02X}{:02X}{:02X}", r_int, g_int, b_int);

            if let Ok(mut state) = MOUSE_STATE.lock() {
                let screen: id = msg_send![window, screen];
                let scale_factor: f64 = msg_send![screen, backingScaleFactor];

                *state = Some(MouseColorInfo {
                    x: location.x,
                    y: location.y,
                    screen_x: screen_location.x,
                    screen_y: screen_location.y,
                    r: r_int,
                    g: g_int,
                    b: b_int,
                    hex_color: hex_color.clone(),
                    scale_factor,
                });
            }

            let _: () = msg_send![_this, setNeedsDisplay: YES];
        }
    }
}

/// Scroll wheel - adjust zoom level
extern "C" fn scroll_wheel(_this: &Object, _cmd: Sel, event: id) {
    unsafe {
        let delta_y: f64 = msg_send![event, deltaY];

        if delta_y != 0.0 {
            if let Ok(mut zoom) = CURRENT_ZOOM.lock() {
                let new_zoom = *zoom + delta_y * ZOOM_STEP;
                *zoom = new_zoom.clamp(ZOOM_MIN, ZOOM_MAX);
            }

            let _: () = msg_send![_this, setNeedsDisplay: YES];
        }
    }
}

/// Keyboard events - ESC to cancel, Enter to confirm, arrows to move
extern "C" fn key_down(_this: &Object, _cmd: Sel, event: id) {
    unsafe {
        let key_code: u16 = msg_send![event, keyCode];
        let modifier_flags: u64 = msg_send![event, modifierFlags];

        let shift_pressed = (modifier_flags & (1 << 17)) != 0;
        let move_amount = if shift_pressed { SHIFT_MOVE_PIXELS } else { 1.0 };

        // ESC key (key code 53) - cancel without saving
        if key_code == 53 {
            stop_picker();
            return;
        }
        
        // Enter/Return key (key code 36) - save color and exit
        if key_code == 36 {
            if let Ok(state) = MOUSE_STATE.lock() {
                if let Some(ref info) = *state {
                    if let Ok(mut selected) = SELECTED_COLOR.lock() {
                        *selected = Some((info.r, info.g, info.b));
                    }
                }
            }
            stop_picker();
            return;
        }

        // Arrow keys - move cursor
        let (dx, dy): (f64, f64) = match key_code {
            123 => (-move_amount, 0.0),  // Left
            124 => (move_amount, 0.0),   // Right
            125 => (0.0, -move_amount),  // Down
            126 => (0.0, move_amount),   // Up
            _ => (0.0, 0.0),
        };

        if dx != 0.0 || dy != 0.0 {
            let cg_event = core_graphics::event::CGEvent::new(
                core_graphics::event_source::CGEventSource::new(
                    core_graphics::event_source::CGEventSourceStateID::HIDSystemState
                ).unwrap()
            ).unwrap();

            let current_pos = cg_event.location();
            let new_x = current_pos.x + dx;
            let new_y = current_pos.y - dy;

            let new_pos = core_graphics::geometry::CGPoint::new(new_x, new_y);

            let move_event = core_graphics::event::CGEvent::new_mouse_event(
                core_graphics::event_source::CGEventSource::new(
                    core_graphics::event_source::CGEventSourceStateID::HIDSystemState
                ).unwrap(),
                core_graphics::event::CGEventType::MouseMoved,
                new_pos,
                core_graphics::event::CGMouseButton::Left,
            ).unwrap();

            move_event.post(core_graphics::event::CGEventTapLocation::HID);

            let main_display = CGDisplay::main();
            let screen_height = main_display.pixels_high() as f64;
            let cocoa_y = screen_height - new_y;

            if let Some((r, g, b)) = get_pixel_color(new_x, cocoa_y) {
                let r_int = (r * 255.0) as u8;
                let g_int = (g * 255.0) as u8;
                let b_int = (b * 255.0) as u8;

                let hex_color = format!("#{:02X}{:02X}{:02X}", r_int, g_int, b_int);

                if let Ok(mut state) = MOUSE_STATE.lock() {
                    let window: id = msg_send![_this, window];
                    let screen_point = NSPoint::new(new_x, cocoa_y);
                    let window_point: NSPoint = msg_send![window, convertPointFromScreen: screen_point];

                    let screen: id = msg_send![window, screen];
                    let scale_factor: f64 = msg_send![screen, backingScaleFactor];

                    *state = Some(MouseColorInfo {
                        x: window_point.x,
                        y: window_point.y,
                        screen_x: new_x,
                        screen_y: cocoa_y,
                        r: r_int,
                        g: g_int,
                        b: b_int,
                        hex_color: hex_color.clone(),
                        scale_factor,
                    });
                }

                let _: () = msg_send![_this, setNeedsDisplay: YES];
            }
        }
    }
}

// =============================================================================
// DRAWING
// =============================================================================

extern "C" fn draw_rect(_this: &Object, _cmd: Sel, _rect: NSRect) {
    unsafe {
        let cls = class!(NSColor);

        // Draw faint overlay
        let color: id = msg_send![cls, colorWithCalibratedWhite:0.0 alpha:0.05];
        let _: () = msg_send![color, set];
        let bounds: NSRect = msg_send![_this, bounds];
        cocoa::appkit::NSRectFill(bounds);

        // Draw magnifier if we have mouse state
        if let Ok(state) = MOUSE_STATE.lock() {
            if let Some(ref info) = *state {
                let current_zoom = match CURRENT_ZOOM.lock() {
                    Ok(z) => *z,
                    Err(_) => INITIAL_ZOOM_FACTOR,
                };

                let mag_size = CAPTURED_PIXELS * current_zoom;
                let capture_size = CAPTURED_PIXELS / info.scale_factor;

                if let Some(cg_image) = capture_zoom_area(info.screen_x, info.screen_y, capture_size) {
                    let img_width = cg_image.width() as f64;
                    let img_height = cg_image.height() as f64;
                    let target_pixels = CAPTURED_PIXELS;
                    
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
                    
                    let use_width = if img_width > target_pixels { target_pixels } else { img_width };
                    let use_height = if img_height > target_pixels { target_pixels } else { img_height };
                    
                    let ns_image_cls = class!(NSImage);
                    let ns_image: id = msg_send![ns_image_cls, alloc];

                    let cg_image_ptr = {
                        let ptr_addr = &cg_image as *const CGImage as *const *const core_graphics::sys::CGImage;
                        *ptr_addr
                    };

                    let full_size = cocoa::foundation::NSSize::new(img_width, img_height);
                    let ns_image: id = msg_send![ns_image, initWithCGImage:cg_image_ptr size:full_size];
                    let cropped_size = cocoa::foundation::NSSize::new(use_width, use_height);

                    let mag_x = info.x - mag_size / 2.0;
                    let mag_y = info.y - mag_size / 2.0;

                    let mag_rect = NSRect::new(
                        NSPoint::new(mag_x, mag_y),
                        cocoa::foundation::NSSize::new(mag_size, mag_size)
                    );

                    let path_cls = class!(NSBezierPath);
                    let circular_clip: id = msg_send![path_cls, bezierPathWithOvalInRect: mag_rect];

                    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];

                    let graphics_context: id = msg_send![class!(NSGraphicsContext), currentContext];
                    let _: () = msg_send![graphics_context, setImageInterpolation: 1u64];

                    let _: () = msg_send![circular_clip, addClip];

                    let from_rect = NSRect::new(
                        NSPoint::new(crop_x, crop_y),
                        cropped_size
                    );

                    let _: () = msg_send![ns_image, drawInRect:mag_rect
                                          fromRect:from_rect
                                          operation:2u64
                                          fraction:1.0];

                    let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];

                    // Draw reticle
                    let actual_pixels = use_width;
                    let pixel_size = mag_size / actual_pixels;
                    
                    let center_x = mag_x + mag_size / 2.0;
                    let center_y = mag_y + mag_size / 2.0;
                    
                    let offset = if (actual_pixels as i32) % 2 == 0 {
                        pixel_size / 2.0
                    } else {
                        0.0
                    };
                    let reticle_center_x = center_x + offset;
                    let reticle_center_y = center_y + offset;

                    let half_pixel = pixel_size / 2.0;
                    let square_rect = NSRect::new(
                        NSPoint::new(reticle_center_x - half_pixel, reticle_center_y - half_pixel),
                        cocoa::foundation::NSSize::new(pixel_size, pixel_size)
                    );

                    let gray_color: id = msg_send![cls, colorWithCalibratedRed: 0.5f64 green: 0.5f64 blue: 0.5f64 alpha: 1.0f64];
                    let _: () = msg_send![gray_color, setStroke];

                    let reticle_path: id = msg_send![path_cls, bezierPathWithRect: square_rect];
                    let _: () = msg_send![reticle_path, setLineWidth: 1.0];
                    let _: () = msg_send![reticle_path, stroke];

                    // Draw border
                    let hex = &info.hex_color[1..];
                    let r_val = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f64 / 255.0;
                    let g_val = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f64 / 255.0;
                    let b_val = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f64 / 255.0;

                    let border_rect = NSRect::new(
                        NSPoint::new(mag_x - BORDER_WIDTH / 2.0, mag_y - BORDER_WIDTH / 2.0),
                        cocoa::foundation::NSSize::new(mag_size + BORDER_WIDTH, mag_size + BORDER_WIDTH)
                    );

                    let border_color: id = msg_send![cls, colorWithCalibratedRed:r_val green:g_val blue:b_val alpha:1.0];
                    let _: () = msg_send![border_color, setStroke];

                    let border_path: id = msg_send![path_cls, bezierPathWithOvalInRect: border_rect];
                    let _: () = msg_send![border_path, setLineWidth: BORDER_WIDTH];
                    let _: () = msg_send![border_path, stroke];

                    // Draw hex text
                    let font_cls = class!(NSFont);
                    let font: id = msg_send![font_cls, systemFontOfSize: HEX_FONT_SIZE weight: 0.62f64];

                    let luminance = 0.299 * r_val + 0.587 * g_val + 0.114 * b_val;

                    let text_color: id = if luminance > 0.5 {
                        msg_send![cls, colorWithCalibratedRed: 0.0f64 green: 0.0f64 blue: 0.0f64 alpha: 1.0f64]
                    } else {
                        msg_send![cls, colorWithCalibratedRed: 1.0f64 green: 1.0f64 blue: 1.0f64 alpha: 1.0f64]
                    };

                    let hex_text = &info.hex_color;
                    let char_count = hex_text.len() as f64;
                    let radius = mag_size / 2.0 + BORDER_WIDTH / 2.0;

                    let angle_step = CHAR_SPACING_PIXELS / radius;
                    let total_arc = angle_step * (char_count - 1.0);
                    let start_angle: f64 = std::f64::consts::PI / 2.0 + total_arc / 2.0;

                    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];

                    for (i, c) in hex_text.chars().enumerate() {
                        let angle = start_angle - angle_step * (i as f64);

                        let char_x = center_x + radius * angle.cos();
                        let char_y = center_y + radius * angle.sin();

                        let char_str = c.to_string();
                        let ns_char = NSString::alloc(nil);
                        let ns_char = NSString::init_str(ns_char, &char_str);

                        let dict_cls = class!(NSDictionary);
                        let font_attr_key = NSString::alloc(nil);
                        let font_attr_key = NSString::init_str(font_attr_key, "NSFont");
                        let color_attr_key = NSString::alloc(nil);
                        let color_attr_key = NSString::init_str(color_attr_key, "NSColor");

                        let keys: [id; 2] = [font_attr_key, color_attr_key];
                        let values: [id; 2] = [font, text_color];

                        let attributes: id = msg_send![dict_cls, dictionaryWithObjects: values.as_ptr() forKeys: keys.as_ptr() count: 2usize];

                        let char_size: cocoa::foundation::NSSize = msg_send![ns_char, sizeWithAttributes: attributes];

                        let transform_cls = class!(NSAffineTransform);
                        let transform: id = msg_send![transform_cls, transform];

                        let _: () = msg_send![transform, translateXBy:char_x yBy:char_y];

                        let rotation_angle = angle - std::f64::consts::PI / 2.0;
                        let _: () = msg_send![transform, rotateByRadians:rotation_angle];

                        let _: () = msg_send![transform, concat];

                        let draw_point = NSPoint::new(-char_size.width / 2.0, -char_size.height / 2.0);
                        let _: () = msg_send![ns_char, drawAtPoint:draw_point withAttributes:attributes];

                        let inverse: id = msg_send![transform, copy];
                        let _: () = msg_send![inverse, invert];
                        let _: () = msg_send![inverse, concat];
                    }

                    let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
                }
            }
        }
    }
}