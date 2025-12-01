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
    NSApplicationActivationPolicyRegular, // App appears in dock
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
///
/// # Arguments
/// * `x` - X coordinate in Cocoa screen space (origin at bottom-left)
/// * `y` - Y coordinate in Cocoa screen space (origin at bottom-left)
/// * `size` - Width and height of the capture area in points
///
/// # Returns
/// * `Some(CGImage)` - The captured image if successful
/// * `None` - If capture failed (e.g., no screen recording permission)
fn capture_zoom_area(x: f64, y: f64, size: f64) -> Option<CGImage> {
    // Import Core Graphics geometry types for creating capture rectangle
    use core_graphics::geometry::{CGRect, CGPoint as CGPointStruct, CGSize};

    // Get the main display for screen capture operations
    let main_display = CGDisplay::main();
    
    // Get screen height in pixels for coordinate conversion
    // Cocoa uses bottom-left origin, Core Graphics uses top-left origin
    let screen_height = main_display.pixels_high() as f64;
    
    // Convert Y coordinate from Cocoa (bottom-left) to CG (top-left)
    let cg_y = screen_height - y;

    // Round coordinates to align with pixel boundaries for sharp capture
    let center_x = x.round();
    let center_y = cg_y.round();
    
    // Round capture size and calculate half for centering
    let capture_size = size.round();
    let half_size = (capture_size / 2.0).floor();
    
    // Create the capture rectangle centered on the cursor position
    let rect = CGRect::new(
        &CGPointStruct::new(center_x - half_size, center_y - half_size),  // Top-left corner
        &CGSize::new(capture_size, capture_size)                           // Width and height
    );

    // Capture and return the screen region as a CGImage
    main_display.image_for_rect(rect)
}

/// Captures the color of a single pixel at the given screen coordinates
///
/// # Arguments
/// * `x` - X coordinate in Cocoa screen space
/// * `y` - Y coordinate in Cocoa screen space
///
/// # Returns
/// * `Some((r, g, b))` - RGB color values normalized to 0.0-1.0 range
/// * `None` - If capture failed
fn get_pixel_color(x: f64, y: f64) -> Option<(f64, f64, f64)> {
    // Import Core Graphics geometry types
    use core_graphics::geometry::{CGRect, CGPoint as CGPointStruct, CGSize};

    // Get the main display
    let main_display = CGDisplay::main();
    
    // Get screen height for coordinate conversion
    let screen_height = main_display.pixels_high() as f64;
    
    // Convert from Cocoa to CG coordinate system
    let cg_y = screen_height - y;

    // Round to align with pixel boundaries (same as capture_zoom_area)
    let center_x = x.round();
    let center_y = cg_y.round();

    // Create a 1x1 pixel capture rectangle
    let rect = CGRect::new(
        &CGPointStruct::new(center_x, center_y),
        &CGSize::new(1.0, 1.0)
    );

    // Capture the single pixel
    let image = main_display.image_for_rect(rect)?;
    
    // Get the raw pixel data from the image
    let data = image.data();
    let data_len = data.len() as usize;

    // Extract RGB values if we have enough data
    // Note: Core Graphics returns BGRA format on macOS
    if data_len >= 4 {
        let b = data[0] as f64 / 255.0;  // Blue channel
        let g = data[1] as f64 / 255.0;  // Green channel
        let r = data[2] as f64 / 255.0;  // Red channel
        // data[3] would be Alpha, but we don't need it
        Some((r, g, b))
    } else {
        None
    }
}

// =============================================================================
// PUBLIC API
// =============================================================================

/// Runs the color picker application on macOS
///
/// This function creates a fullscreen transparent overlay window,
/// captures the screen, and allows the user to pick a color by
/// clicking or pressing Enter. Press ESC to cancel.
///
/// # Returns
/// * `Some((r, g, b))` - The selected RGB color (0-255 range) if user confirmed
/// * `None` - If user pressed ESC to cancel
pub fn run() -> Option<(u8, u8, u8)> {
    // Reset the selected color from any previous run
    if let Ok(mut color) = SELECTED_COLOR.lock() {
        *color = None;
    }
    
    // All Cocoa/Objective-C calls must be in an unsafe block
    unsafe {
        // Create an autorelease pool for Objective-C memory management
        // This ensures temporary objects are properly released
        let _pool = NSAutoreleasePool::new(nil);

        // Get the shared application instance
        let app = NSApp();
        
        // Set activation policy to Regular so the app appears in the dock
        // and can receive keyboard events
        app.setActivationPolicy_(NSApplicationActivationPolicyRegular);

        // Register our custom view and window classes
        let view_class = register_view_class();
        let window_class = register_window_class();

        // Get all available screens (for multi-monitor support)
        let screens = NSScreen::screens(nil);
        let count = screens.count();

        // Create an overlay window for each screen
        for i in 0..count {
            // Get the screen at index i
            let screen = screens.objectAtIndex(i);
            
            // Get the screen's frame (position and size)
            let frame: NSRect = msg_send![screen, frame];

            // Allocate and initialize a new window using our custom class
            let window_alloc: id = msg_send![window_class, alloc];
            let window = window_alloc.initWithContentRect_styleMask_backing_defer_(
                frame,                                    // Cover the entire screen
                NSWindowStyleMask::NSBorderlessWindowMask, // No title bar or borders
                NSBackingStoreBuffered,                   // Double-buffered rendering
                NO                                        // Don't defer window creation
            );

            // Set window level very high so it appears above everything
            window.setLevel_(1000);

            // Get NSColor class for creating colors
            let cls_color = class!(NSColor);
            
            // Create a fully transparent color for the background
            let clear_color: id = msg_send![cls_color, clearColor];

            // Configure window to be transparent and non-opaque
            window.setBackgroundColor_(clear_color);
            window.setOpaque_(NO);           // Window is not opaque
            window.setHasShadow_(NO);        // No window shadow
            window.setIgnoresMouseEvents_(NO); // We want to receive mouse events
            window.setAcceptsMouseMovedEvents_(YES); // Track mouse movement

            // Prevent window from being captured in screenshots/recordings
            let _: () = msg_send![window, setSharingType: 0u64];

            // Create our custom view and set it as the window's content
            let view: id = msg_send![view_class, alloc];
            let view: id = msg_send![view, initWithFrame:frame];

            // Set the view as the window's content view
            window.setContentView_(view);
            
            // Show the window and bring it to front
            window.makeKeyAndOrderFront_(nil);

            // Make the view the first responder so it receives keyboard events
            let _: () = msg_send![window, makeFirstResponder: view];
        }

        // Activate our application and bring it to the foreground
        let current_app = NSRunningApplication::currentApplication(nil);
        current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);

        // Hide the system cursor while the color picker is active
        let _: () = msg_send![class!(NSCursor), hide];

        // Run the application event loop
        // This blocks until stop: is called
        app.run();
    }
    
    // After the event loop exits, return the selected color (if any)
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
///
/// This creates a new Objective-C class "ColorPickerView" that inherits
/// from NSView and overrides several methods for custom behavior.
fn register_view_class() -> &'static objc::runtime::Class {
    // Get the NSView class to use as our superclass
    let superclass = class!(NSView);
    
    // Create a new class declaration
    let mut decl = ClassDecl::new("ColorPickerView", superclass).unwrap();

    // Add method implementations to our custom class
    unsafe {
        // acceptsFirstResponder - return YES to receive keyboard events
        decl.add_method(
            sel!(acceptsFirstResponder),
            accepts_first_responder as extern "C" fn(&Object, Sel) -> bool
        );
        
        // mouseDown: - handle mouse click events
        decl.add_method(
            sel!(mouseDown:),
            mouse_down as extern "C" fn(&Object, Sel, id)
        );
        
        // mouseMoved: - handle mouse movement events
        decl.add_method(
            sel!(mouseMoved:),
            mouse_moved as extern "C" fn(&Object, Sel, id)
        );
        
        // scrollWheel: - handle scroll wheel events for zoom
        decl.add_method(
            sel!(scrollWheel:),
            scroll_wheel as extern "C" fn(&Object, Sel, id)
        );
        
        // keyDown: - handle keyboard events
        decl.add_method(
            sel!(keyDown:),
            key_down as extern "C" fn(&Object, Sel, id)
        );
        
        // drawRect: - custom drawing code
        decl.add_method(
            sel!(drawRect:),
            draw_rect as extern "C" fn(&Object, Sel, NSRect)
        );
    }

    // Register and return the new class
    decl.register()
}

/// Registers a custom NSWindow subclass that can become key window
///
/// By default, borderless windows cannot become the key window.
/// This subclass overrides canBecomeKeyWindow to allow keyboard focus.
fn register_window_class() -> &'static objc::runtime::Class {
    // Get NSWindow as our superclass
    let superclass = class!(NSWindow);
    
    // Create a new class declaration
    let mut decl = ClassDecl::new("KeyableWindow", superclass).unwrap();

    // Override canBecomeKeyWindow to return YES
    unsafe {
        decl.add_method(
            sel!(canBecomeKeyWindow),
            can_become_key_window as extern "C" fn(&Object, Sel) -> bool
        );
    }

    // Register and return the new class
    decl.register()
}

// =============================================================================
// OBJECTIVE-C METHOD IMPLEMENTATIONS
// =============================================================================

/// Implementation of canBecomeKeyWindow - always returns YES
/// This allows our borderless window to receive keyboard focus
extern "C" fn can_become_key_window(_this: &Object, _cmd: Sel) -> bool {
    YES
}

/// Implementation of acceptsFirstResponder - always returns YES
/// This allows our view to receive keyboard events directly
extern "C" fn accepts_first_responder(_this: &Object, _cmd: Sel) -> bool {
    YES
}

/// Implementation of mouseDown: - handles mouse click events
/// When clicked, saves the current color and exits the application
extern "C" fn mouse_down(_this: &Object, _cmd: Sel, _event: id) {
    unsafe {
        // Save the current color before exiting
        // Lock the mouse state mutex to read the current color
        if let Ok(state) = MOUSE_STATE.lock() {
            if let Some(ref info) = *state {
                // Lock the selected color mutex and save the RGB values
                if let Ok(mut selected) = SELECTED_COLOR.lock() {
                    *selected = Some((info.r, info.g, info.b));
                }
            }
        }
        
        // Show the cursor again before exiting
        let _: () = msg_send![class!(NSCursor), unhide];
        
        // Get the application instance
        let app = NSApp();
        
        // Stop the application run loop (app.run() will return)
        let _: () = msg_send![app, stop:nil];
        
        // Post a dummy event to wake up the run loop so it can exit
        // This is necessary because stop: only takes effect on the next event
        let dummy_event: id = msg_send![class!(NSEvent), 
            otherEventWithType:15u64  // NSEventTypeApplicationDefined
            location:NSPoint::new(0.0, 0.0)
            modifierFlags:0u64
            timestamp:0.0f64
            windowNumber:0i64
            context:nil
            subtype:0i16
            data1:0i64
            data2:0i64
        ];
        let _: () = msg_send![app, postEvent:dummy_event atStart:YES];
    }
}

/// Implementation of mouseMoved: - handles mouse movement events
/// Updates the current color and requests a redraw of the magnifier
extern "C" fn mouse_moved(_this: &Object, _cmd: Sel, event: id) {
    unsafe {
        // Get mouse location in window coordinates
        let location: NSPoint = msg_send![event, locationInWindow];
        
        // Get the window containing this view
        let window: id = msg_send![_this, window];
        
        // Convert window coordinates to screen coordinates
        let screen_location: NSPoint = msg_send![window, convertPointToScreen: location];

        // Get the color of the pixel at the cursor position
        if let Some((r, g, b)) = get_pixel_color(screen_location.x as f64, screen_location.y as f64) {
            // Convert from 0.0-1.0 range to 0-255 integer range
            let r_int = (r * 255.0) as u8;
            let g_int = (g * 255.0) as u8;
            let b_int = (b * 255.0) as u8;

            // Format as hex color string
            let hex_color = format!("#{:02X}{:02X}{:02X}", r_int, g_int, b_int);

            // Update the global mouse state with new position and color
            if let Ok(mut state) = MOUSE_STATE.lock() {
                // Get the screen's backing scale factor (1.0 or 2.0 for Retina)
                let screen: id = msg_send![window, screen];
                let scale_factor: f64 = msg_send![screen, backingScaleFactor];

                // Store all the information in the global state
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

            // Request a redraw of the view to update the magnifier display
            let _: () = msg_send![_this, setNeedsDisplay: YES];
        }
    }
}

/// Implementation of scrollWheel: - handles scroll wheel events
/// Adjusts the zoom level of the magnifier
extern "C" fn scroll_wheel(_this: &Object, _cmd: Sel, event: id) {
    unsafe {
        // Get the vertical scroll delta
        let delta_y: f64 = msg_send![event, deltaY];

        // Only process if there's actual scroll movement
        if delta_y != 0.0 {
            // Lock the zoom mutex and update the zoom level
            if let Ok(mut zoom) = CURRENT_ZOOM.lock() {
                // Calculate new zoom with the scroll delta
                let new_zoom = *zoom + delta_y * ZOOM_STEP;
                // Clamp to valid range
                *zoom = new_zoom.clamp(ZOOM_MIN, ZOOM_MAX);
            }

            // Request a redraw to show the new zoom level
            let _: () = msg_send![_this, setNeedsDisplay: YES];
        }
    }
}

/// Implementation of keyDown: - handles keyboard events
/// Arrow keys move the cursor, ESC cancels, Enter/Return confirms selection
extern "C" fn key_down(_this: &Object, _cmd: Sel, event: id) {
    unsafe {
        // Get the key code of the pressed key
        let key_code: u16 = msg_send![event, keyCode];
        
        // Get modifier flags to check for Shift key
        let modifier_flags: u64 = msg_send![event, modifierFlags];

        // Check if Shift key is pressed (bit 17)
        let shift_pressed = (modifier_flags & (1 << 17)) != 0;
        
        // Determine move amount: 1 pixel normally, SHIFT_MOVE_PIXELS with Shift
        let move_amount = if shift_pressed { SHIFT_MOVE_PIXELS } else { 1.0 };

        // ESC key (key code 53) - cancel without saving
        if key_code == 53 {
            // Don't save any color - leave SELECTED_COLOR as None
            
            // Show the cursor again
            let _: () = msg_send![class!(NSCursor), unhide];
            
            // Stop the application
            let app = NSApp();
            let _: () = msg_send![app, stop:nil];
            
            // Post dummy event to wake up run loop
            let dummy_event: id = msg_send![class!(NSEvent), 
                otherEventWithType:15u64
                location:NSPoint::new(0.0, 0.0)
                modifierFlags:0u64
                timestamp:0.0f64
                windowNumber:0i64
                context:nil
                subtype:0i16
                data1:0i64
                data2:0i64
            ];
            let _: () = msg_send![app, postEvent:dummy_event atStart:YES];
            return;
        }
        
        // Enter/Return key (key code 36) - save color and exit
        if key_code == 36 {
            // Save the current color before exiting
            if let Ok(state) = MOUSE_STATE.lock() {
                if let Some(ref info) = *state {
                    if let Ok(mut selected) = SELECTED_COLOR.lock() {
                        *selected = Some((info.r, info.g, info.b));
                    }
                }
            }
            
            // Show the cursor again
            let _: () = msg_send![class!(NSCursor), unhide];
            
            // Stop the application
            let app = NSApp();
            let _: () = msg_send![app, stop:nil];
            
            // Post dummy event to wake up run loop
            let dummy_event: id = msg_send![class!(NSEvent), 
                otherEventWithType:15u64
                location:NSPoint::new(0.0, 0.0)
                modifierFlags:0u64
                timestamp:0.0f64
                windowNumber:0i64
                context:nil
                subtype:0i16
                data1:0i64
                data2:0i64
            ];
            let _: () = msg_send![app, postEvent:dummy_event atStart:YES];
            return;
        }

        // Arrow keys - determine movement direction
        // Key codes: 123=Left, 124=Right, 125=Down, 126=Up
        let (dx, dy): (f64, f64) = match key_code {
            123 => (-move_amount, 0.0),  // Left arrow
            124 => (move_amount, 0.0),   // Right arrow
            125 => (0.0, -move_amount),  // Down arrow
            126 => (0.0, move_amount),   // Up arrow
            _ => (0.0, 0.0),             // Other keys - no movement
        };

        // If there's movement to perform
        if dx != 0.0 || dy != 0.0 {
            // Create a CGEvent to get the current mouse position
            let cg_event = core_graphics::event::CGEvent::new(
                core_graphics::event_source::CGEventSource::new(
                    core_graphics::event_source::CGEventSourceStateID::HIDSystemState
                ).unwrap()
            ).unwrap();

            // Get current cursor position
            let current_pos = cg_event.location();

            // Calculate new position with the movement delta
            let new_x = current_pos.x + dx;
            let new_y = current_pos.y - dy;  // Inverted because CG uses top-left origin

            // Create the new position point
            let new_pos = core_graphics::geometry::CGPoint::new(new_x, new_y);

            // Create and post a mouse move event to actually move the cursor
            let move_event = core_graphics::event::CGEvent::new_mouse_event(
                core_graphics::event_source::CGEventSource::new(
                    core_graphics::event_source::CGEventSourceStateID::HIDSystemState
                ).unwrap(),
                core_graphics::event::CGEventType::MouseMoved,
                new_pos,
                core_graphics::event::CGMouseButton::Left,
            ).unwrap();

            // Post the event to the system
            move_event.post(core_graphics::event::CGEventTapLocation::HID);

            // Convert CG coordinates back to Cocoa coordinates for color capture
            let main_display = CGDisplay::main();
            let screen_height = main_display.pixels_high() as f64;
            let cocoa_y = screen_height - new_y;

            // Get the color at the new cursor position
            if let Some((r, g, b)) = get_pixel_color(new_x, cocoa_y) {
                // Convert to 0-255 range
                let r_int = (r * 255.0) as u8;
                let g_int = (g * 255.0) as u8;
                let b_int = (b * 255.0) as u8;

                // Format as hex string
                let hex_color = format!("#{:02X}{:02X}{:02X}", r_int, g_int, b_int);

                // Update the global state
                if let Ok(mut state) = MOUSE_STATE.lock() {
                    // Get window and convert screen point to window point
                    let window: id = msg_send![_this, window];
                    let screen_point = NSPoint::new(new_x, cocoa_y);
                    let window_point: NSPoint = msg_send![window, convertPointFromScreen: screen_point];

                    // Get scale factor
                    let screen: id = msg_send![window, screen];
                    let scale_factor: f64 = msg_send![screen, backingScaleFactor];

                    // Update state with new position and color
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

                // Request redraw
                let _: () = msg_send![_this, setNeedsDisplay: YES];
            }
        }
    }
}

// =============================================================================
// DRAWING
// =============================================================================

/// Implementation of drawRect: - handles all custom drawing
/// Draws the magnifier, reticle, border, and hex text
extern "C" fn draw_rect(_this: &Object, _cmd: Sel, _rect: NSRect) {
    unsafe {
        // Get NSColor class for color operations
        let cls = class!(NSColor);

        // =====================================================================
        // DRAW FAINT OVERLAY
        // =====================================================================
        
        // Create a very faint black overlay (5% opacity)
        // This slightly dims the screen to indicate the picker is active
        let color: id = msg_send![cls, colorWithCalibratedWhite:0.0 alpha:0.05];
        
        // Set as the current fill color
        let _: () = msg_send![color, set];
        
        // Get the view's bounds (full size)
        let bounds: NSRect = msg_send![_this, bounds];
        
        // Fill the entire view with the overlay
        cocoa::appkit::NSRectFill(bounds);

        // =====================================================================
        // DRAW MAGNIFIER (only if we have mouse state)
        // =====================================================================
        
        // Lock the mouse state to read current position and color
        if let Ok(state) = MOUSE_STATE.lock() {
            if let Some(ref info) = *state {
                // Get current zoom level
                let current_zoom = match CURRENT_ZOOM.lock() {
                    Ok(z) => *z,
                    Err(_) => INITIAL_ZOOM_FACTOR,  // Fallback to default
                };

                // Calculate magnifier display size
                // mag_size = number of pixels captured * zoom factor
                let mag_size = CAPTURED_PIXELS * current_zoom;
                
                // On Retina displays, divide by scale_factor to capture
                // the same number of physical pixels as on standard displays
                let capture_size = CAPTURED_PIXELS / info.scale_factor;

                // Capture the screen area around the cursor
                if let Some(cg_image) = capture_zoom_area(info.screen_x, info.screen_y, capture_size) {
                    // ==========================================================
                    // CREATE NSIMAGE FROM CGIMAGE
                    // ==========================================================
                    
                    // Allocate a new NSImage
                    let ns_image_cls = class!(NSImage);
                    let ns_image: id = msg_send![ns_image_cls, alloc];

                    // Get raw CGImageRef pointer for Objective-C messaging
                    let cg_image_ptr = {
                        let ptr_addr = &cg_image as *const CGImage as *const *const core_graphics::sys::CGImage;
                        *ptr_addr
                    };

                    // Create NSSize with the captured image dimensions
                    let size = cocoa::foundation::NSSize::new(
                        cg_image.width() as f64,
                        cg_image.height() as f64
                    );

                    // Initialize NSImage with the CGImage
                    let ns_image: id = msg_send![ns_image, initWithCGImage:cg_image_ptr size:size];

                    // ==========================================================
                    // CALCULATE MAGNIFIER POSITION
                    // ==========================================================
                    
                    // Position magnifier centered on cursor
                    let mag_x = info.x - mag_size / 2.0;
                    let mag_y = info.y - mag_size / 2.0;

                    // Create rectangle for the magnifier
                    let mag_rect = NSRect::new(
                        NSPoint::new(mag_x, mag_y),
                        cocoa::foundation::NSSize::new(mag_size, mag_size)
                    );

                    // Get NSBezierPath class for drawing shapes
                    let path_cls = class!(NSBezierPath);

                    // ==========================================================
                    // DRAW MAGNIFIED IMAGE WITH CIRCULAR CLIPPING
                    // ==========================================================
                    
                    // Create circular clipping path
                    let circular_clip: id = msg_send![path_cls, bezierPathWithOvalInRect: mag_rect];

                    // Save current graphics state before clipping
                    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];

                    // Disable image interpolation for sharp pixel edges
                    // Without this, macOS would blur the pixels when scaling
                    let graphics_context: id = msg_send![class!(NSGraphicsContext), currentContext];
                    let _: () = msg_send![graphics_context, setImageInterpolation: 1u64]; // NSImageInterpolationNone

                    // Apply circular clipping - drawing is now restricted to the circle
                    let _: () = msg_send![circular_clip, addClip];

                    // Source rectangle - use entire captured image
                    let from_rect = NSRect::new(
                        NSPoint::new(0.0, 0.0),
                        size
                    );

                    // Draw the magnified image
                    // operation: 2 = NSCompositingOperationSourceOver (normal alpha blending)
                    // fraction: 1.0 = fully opaque
                    let _: () = msg_send![ns_image, drawInRect:mag_rect
                                          fromRect:from_rect
                                          operation:2u64
                                          fraction:1.0];

                    // Restore graphics state (removes clipping)
                    let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];

                    // ==========================================================
                    // DRAW CENTER RETICLE (SQUARE TARGET)
                    // ==========================================================
                    
                    // Get actual number of pixels captured for sizing calculations
                    let actual_pixels = cg_image.width() as f64;
                    
                    // Calculate size of one magnified pixel
                    let pixel_size = mag_size / actual_pixels;
                    
                    // Calculate center of magnifier
                    let center_x = mag_x + mag_size / 2.0;
                    let center_y = mag_y + mag_size / 2.0;
                    
                    // If even number of pixels, offset by half to center on a pixel
                    // (odd numbers naturally center on the middle pixel)
                    let offset = if (actual_pixels as i32) % 2 == 0 {
                        pixel_size / 2.0
                    } else {
                        0.0
                    };
                    let reticle_center_x = center_x + offset;
                    let reticle_center_y = center_y + offset;

                    // Create square reticle rectangle (same size as one pixel)
                    let half_pixel = pixel_size / 2.0;
                    let square_rect = NSRect::new(
                        NSPoint::new(reticle_center_x - half_pixel, reticle_center_y - half_pixel),
                        cocoa::foundation::NSSize::new(pixel_size, pixel_size)
                    );

                    // Set stroke color to gray (#808080)
                    let gray_color: id = msg_send![cls, colorWithCalibratedRed: 0.5f64 green: 0.5f64 blue: 0.5f64 alpha: 1.0f64];
                    let _: () = msg_send![gray_color, setStroke];

                    // Create and stroke the square reticle path
                    let reticle_path: id = msg_send![path_cls, bezierPathWithRect: square_rect];
                    let _: () = msg_send![reticle_path, setLineWidth: 1.0];  // Thin border
                    let _: () = msg_send![reticle_path, stroke];

                    // ==========================================================
                    // DRAW COLORED BORDER
                    // ==========================================================
                    
                    // Parse hex color string to get RGB values for the border
                    let hex = &info.hex_color[1..];  // Skip the '#'
                    let r_val = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f64 / 255.0;
                    let g_val = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f64 / 255.0;
                    let b_val = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f64 / 255.0;

                    // Create border rectangle (slightly larger than magnifier)
                    let border_rect = NSRect::new(
                        NSPoint::new(mag_x - BORDER_WIDTH / 2.0, mag_y - BORDER_WIDTH / 2.0),
                        cocoa::foundation::NSSize::new(mag_size + BORDER_WIDTH, mag_size + BORDER_WIDTH)
                    );

                    // Create color matching the picked pixel
                    let border_color: id = msg_send![cls, colorWithCalibratedRed:r_val green:g_val blue:b_val alpha:1.0];
                    let _: () = msg_send![border_color, setStroke];

                    // Create and stroke the circular border path
                    let border_path: id = msg_send![path_cls, bezierPathWithOvalInRect: border_rect];
                    let _: () = msg_send![border_path, setLineWidth: BORDER_WIDTH];
                    let _: () = msg_send![border_path, stroke];

                    // ==========================================================
                    // DRAW HEX TEXT ALONG BORDER ARC
                    // ==========================================================
                    
                    // Get system font for text rendering
                    let font_cls = class!(NSFont);
                    let font: id = msg_send![font_cls, systemFontOfSize: HEX_FONT_SIZE weight: 0.62f64]; // Semi-bold

                    // Calculate luminance to determine text color (black or white)
                    // Using standard luminance formula
                    let luminance = 0.299 * r_val + 0.587 * g_val + 0.114 * b_val;

                    // Choose text color for best contrast against the border
                    let text_color: id = if luminance > 0.5 {
                        // Light background - use black text
                        msg_send![cls, colorWithCalibratedRed: 0.0f64 green: 0.0f64 blue: 0.0f64 alpha: 1.0f64]
                    } else {
                        // Dark background - use white text
                        msg_send![cls, colorWithCalibratedRed: 1.0f64 green: 1.0f64 blue: 1.0f64 alpha: 1.0f64]
                    };

                    // Get the hex text to display
                    let hex_text = &info.hex_color;
                    let char_count = hex_text.len() as f64;
                    
                    // Calculate radius for text placement (middle of border)
                    let radius = mag_size / 2.0 + BORDER_WIDTH / 2.0;

                    // Calculate angle between characters based on fixed pixel spacing
                    let angle_step = CHAR_SPACING_PIXELS / radius;
                    
                    // Calculate total arc length and starting angle
                    let total_arc = angle_step * (char_count - 1.0);
                    let start_angle: f64 = std::f64::consts::PI / 2.0 + total_arc / 2.0;  // Start at top

                    // Save graphics state for text transformations
                    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];

                    // Draw each character individually along the arc
                    for (i, c) in hex_text.chars().enumerate() {
                        // Calculate angle for this character
                        let angle = start_angle - angle_step * (i as f64);

                        // Calculate position on the arc
                        let char_x = center_x + radius * angle.cos();
                        let char_y = center_y + radius * angle.sin();

                        // Convert character to NSString
                        let char_str = c.to_string();
                        let ns_char = NSString::alloc(nil);
                        let ns_char = NSString::init_str(ns_char, &char_str);

                        // Create text attributes dictionary
                        let dict_cls = class!(NSDictionary);
                        let font_attr_key = NSString::alloc(nil);
                        let font_attr_key = NSString::init_str(font_attr_key, "NSFont");
                        let color_attr_key = NSString::alloc(nil);
                        let color_attr_key = NSString::init_str(color_attr_key, "NSColor");

                        let keys: [id; 2] = [font_attr_key, color_attr_key];
                        let values: [id; 2] = [font, text_color];

                        let attributes: id = msg_send![dict_cls, dictionaryWithObjects: values.as_ptr() forKeys: keys.as_ptr() count: 2usize];

                        // Get character size for centering
                        let char_size: cocoa::foundation::NSSize = msg_send![ns_char, sizeWithAttributes: attributes];

                        // Create transform for positioning and rotating the character
                        let transform_cls = class!(NSAffineTransform);
                        let transform: id = msg_send![transform_cls, transform];

                        // Translate to character position
                        let _: () = msg_send![transform, translateXBy:char_x yBy:char_y];

                        // Rotate to be tangent to the arc
                        let rotation_angle = angle - std::f64::consts::PI / 2.0;
                        let _: () = msg_send![transform, rotateByRadians:rotation_angle];

                        // Apply the transform
                        let _: () = msg_send![transform, concat];

                        // Draw the character centered at the transformed origin
                        let draw_point = NSPoint::new(-char_size.width / 2.0, -char_size.height / 2.0);
                        let _: () = msg_send![ns_char, drawAtPoint:draw_point withAttributes:attributes];

                        // Invert the transform to prepare for next character
                        let inverse: id = msg_send![transform, copy];
                        let _: () = msg_send![inverse, invert];
                        let _: () = msg_send![inverse, concat];
                    }

                    // Restore graphics state
                    let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
                }
            }
        }
    }
}