//! CCA Color Picker for macOS
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

// =============================================================================
// IMPORTS
// =============================================================================

// Cocoa framework bindings for macOS GUI
use cocoa::base::{id, nil, NO, YES};  // Basic Objective-C types (id=object pointer, nil=null)
use cocoa::foundation::{
    NSAutoreleasePool,  // Memory management pool for Objective-C objects
    NSRect,             // Rectangle structure (origin + size)
    NSArray,            // Objective-C array type
    NSPoint,            // Point structure (x, y)
    NSString,           // Objective-C string type
};
use cocoa::appkit::{
    NSApp,                              // Global application instance
    NSApplication,                       // Application class
    NSApplicationActivationPolicyRegular, // App appears in dock
    NSBackingStoreBuffered,              // Double-buffered window
    NSWindow,                            // Window class
    NSWindowStyleMask,                   // Window style options
    NSRunningApplication,                // Running application info
    NSApplicationActivateIgnoringOtherApps, // Activation option
    NSScreen,                            // Screen information
};

// Objective-C runtime bindings
use objc::{class, msg_send, sel, sel_impl};  // Macros for Objective-C messaging
use objc::declare::ClassDecl;                 // For creating custom Objective-C classes
use objc::runtime::{Object, Sel};             // Objective-C runtime types

// Core Graphics for screen capture and color picking
use core_graphics::display::CGDisplay;  // Display/screen functions
use core_graphics::image::CGImage;      // Image type for screen captures

// Standard library
use std::sync::Mutex;  // Thread-safe mutex for global state

// =============================================================================
// CONFIGURATION CONSTANTS
// =============================================================================

/// Thickness of the colored border around the magnifier (in pixels)
/// This border displays the current color being picked
const BORDER_WIDTH: f64 = 20.0;

/// Font size for the hex color text displayed on the border (in points)
/// The text shows the hex value like "#FF5733"
const HEX_FONT_SIZE: f64 = 14.0;

/// Number of screen pixels captured by the magnifier
/// Smaller value = more zoom, larger value = less zoom
/// This determines how many pixels are visible in the magnifier
const CAPTURED_PIXELS: f64 = 8.0;

/// Default zoom factor for the magnifier
/// magnifier_size = CAPTURED_PIXELS * ZOOM_FACTOR
/// Example: 8 pixels * 20 = 160px magnifier diameter
const INITIAL_ZOOM_FACTOR: f64 = 20.0;

/// Number of pixels to move when pressing Shift + Arrow key
/// Regular arrow key moves 1 pixel, Shift+arrow moves this many
const SHIFT_MOVE_PIXELS: f64 = 50.0;

/// Minimum zoom factor (can't zoom out beyond this)
const ZOOM_MIN: f64 = 15.0;

/// Maximum zoom factor (can't zoom in beyond this)
const ZOOM_MAX: f64 = 50.0;

/// Zoom increment per scroll wheel step
/// Each scroll tick changes zoom by this amount
const ZOOM_STEP: f64 = 2.0;

/// Fixed spacing between characters in the hex text (in pixels)
/// This ensures consistent text appearance regardless of zoom level
const CHAR_SPACING_PIXELS: f64 = 12.0;

// =============================================================================
// GLOBAL STATE
// =============================================================================

/// Global mutex-protected state for mouse position and color
/// Uses Mutex for thread-safe access from multiple event handlers
/// This allows the draw function to access position data set by mouse events
static MOUSE_STATE: Mutex<Option<MouseColorInfo>> = Mutex::new(None);

/// Global mutex-protected state for current zoom level
/// Allows zoom to be adjusted via scroll wheel and persisted across redraws
static CURRENT_ZOOM: Mutex<f64> = Mutex::new(INITIAL_ZOOM_FACTOR);

/// Information about the current mouse position and color
/// Stored globally so the draw function can access it
struct MouseColorInfo {
    x: f64,           // X position in window coordinates (for drawing)
    y: f64,           // Y position in window coordinates (for drawing)
    screen_x: f64,    // X position in screen coordinates (for capture)
    screen_y: f64,    // Y position in screen coordinates (for capture)
    hex_color: String, // Color as hex string like "#FF5733"
}

// =============================================================================
// SCREEN CAPTURE FUNCTIONS
// =============================================================================

/// Captures a square area of pixels around the given screen coordinates
///
/// # Arguments
/// * `x` - X coordinate in Cocoa screen space (origin bottom-left)
/// * `y` - Y coordinate in Cocoa screen space (origin bottom-left)
/// * `size` - Width and height of the capture area in pixels
///
/// # Returns
/// * `Some(CGImage)` - The captured image if successful
/// * `None` - If capture failed (e.g., no screen recording permission)
///
/// # Note
/// This function converts from Cocoa coordinates (origin bottom-left)
/// to Core Graphics coordinates (origin top-left)
fn capture_zoom_area(x: f64, y: f64, size: f64) -> Option<CGImage> {
    // Import Core Graphics geometry types
    use core_graphics::geometry::{CGRect, CGPoint as CGPointStruct, CGSize};

    // Get the main display for screen capture
    let main_display = CGDisplay::main();

    // Get screen height for coordinate system conversion
    // Cocoa uses bottom-left origin, Core Graphics uses top-left origin
    let screen_height = main_display.pixels_high() as f64;

    // Convert Y coordinate from Cocoa (bottom-left) to CG (top-left)
    // In Cocoa: y=0 is at bottom, in CG: y=0 is at top
    let cg_y = screen_height - y;

    // Calculate the capture rectangle centered on the cursor position
    let half_size = size / 2.0;
    let rect = CGRect::new(
        &CGPointStruct::new(x - half_size, cg_y - half_size),  // Top-left corner
        &CGSize::new(size, size)                                 // Width and height
    );

    // Capture the screen region and return the image
    // Returns None if screen recording permission is not granted
    main_display.image_for_rect(rect)
}

/// Captures the color of a single pixel at the given screen coordinates
///
/// # Arguments
/// * `x` - X coordinate in Cocoa screen space
/// * `y` - Y coordinate in Cocoa screen space
///
/// # Returns
/// * `Some((r, g, b))` - RGB values as f64 in range 0.0-1.0
/// * `None` - If capture failed
///
/// # Note
/// macOS uses BGRA format for pixel data, so we need to reorder the components
fn get_pixel_color(x: f64, y: f64) -> Option<(f64, f64, f64)> {
    // Import Core Graphics geometry types
    use core_graphics::geometry::{CGRect, CGPoint as CGPointStruct, CGSize};

    // Get the main display
    let main_display = CGDisplay::main();

    // Get screen height for coordinate conversion
    let screen_height = main_display.pixels_high() as f64;

    // Convert Y from Cocoa to CG coordinate system
    let cg_y = screen_height - y;

    // Create a 1x1 pixel rectangle at the target position
    let rect = CGRect::new(
        &CGPointStruct::new(x, cg_y),
        &CGSize::new(1.0, 1.0)
    );

    // Capture just this single pixel
    let image = main_display.image_for_rect(rect)?;

    // Get the raw pixel data from the image
    let data = image.data();
    let data_len = data.len() as usize;

    // Check we have at least 4 bytes (BGRA format)
    if data_len >= 4 {
        // macOS uses BGRA format, so:
        // data[0] = Blue, data[1] = Green, data[2] = Red, data[3] = Alpha
        let b = data[0] as f64 / 255.0;  // Blue component normalized to 0.0-1.0
        let g = data[1] as f64 / 255.0;  // Green component normalized to 0.0-1.0
        let r = data[2] as f64 / 255.0;  // Red component normalized to 0.0-1.0

        Some((r, g, b))
    } else {
        None  // Not enough data
    }
}

// =============================================================================
// MAIN ENTRY POINT
// =============================================================================

fn main() {
    // =========================================================================
    // IMPORTANT - Required Permissions:
    //
    // This app requires the "Screen Recording" permission to capture colors.
    //
    // If the colors do not appear:
    // 1. Open System Preferences
    // 2. Security & Privacy
    // 3. Privacy > Screen Recording
    // 4. Enable this application
    // 5. Restart the application
    // =========================================================================

    // All Cocoa/AppKit code must be in an unsafe block
    // because we're calling Objective-C methods via FFI
    unsafe {
        // Create an autorelease pool for Objective-C memory management
        // Objects created in this block are automatically released when the pool is drained
        let _pool = NSAutoreleasePool::new(nil);

        // Get the shared NSApplication instance (singleton)
        // This is the main application object that manages the event loop
        let app = NSApp();

        // Set activation policy to Regular so app appears in Dock
        // and can become the frontmost application
        app.setActivationPolicy_(NSApplicationActivationPolicyRegular);

        // Register our custom NSView subclass to handle events
        // This view will handle mouse, keyboard, and drawing
        let view_class = register_view_class();

        // Register our custom NSWindow subclass that can become key window
        // Borderless windows normally can't receive keyboard events, so we override this
        let window_class = register_window_class();

        // Get all connected screens/displays
        // We create a window on each screen for multi-monitor support
        let screens = NSScreen::screens(nil);
        let count = screens.count();  // Number of screens

        // Create a fullscreen transparent window on each screen
        // This ensures the color picker works across all monitors
        for i in 0..count {
            // Get the screen at index i
            let screen = screens.objectAtIndex(i);

            // Get the screen's frame (position and size)
            // Using msg_send! to avoid trait ambiguity between NSScreen and NSView
            let frame: NSRect = msg_send![screen, frame];

            // Allocate a new window using our custom KeyableWindow class
            let window_alloc: id = msg_send![window_class, alloc];

            // Initialize the window with:
            // - frame: covers the entire screen
            // - style: borderless (no title bar, buttons, etc.)
            // - backing: buffered for smooth drawing
            // - defer: NO means create the window immediately
            let window = window_alloc.initWithContentRect_styleMask_backing_defer_(
                frame,
                NSWindowStyleMask::NSBorderlessWindowMask,
                NSBackingStoreBuffered,
                NO
            );

            // Set window level to screen saver level (1000)
            // This puts our window above almost everything else
            window.setLevel_(1000);

            // Get a clear (transparent) color for the window background
            let cls_color = class!(NSColor);
            let clear_color: id = msg_send![cls_color, clearColor];

            // Configure window for transparency
            window.setBackgroundColor_(clear_color);  // Transparent background
            window.setOpaque_(NO);                     // Window is not opaque
            window.setHasShadow_(NO);                  // No drop shadow
            window.setIgnoresMouseEvents_(NO);         // Capture mouse events
            window.setAcceptsMouseMovedEvents_(YES);   // Track mouse movement

            // Exclude this window from screen captures/recordings
            // This prevents the magnifier from capturing itself!
            // NSWindowSharingNone = 0
            let _: () = msg_send![window, setSharingType: 0u64];

            // Create an instance of our custom view
            let view: id = msg_send![view_class, alloc];
            let view: id = msg_send![view, initWithFrame:frame];

            // Set the view as the window's content view
            window.setContentView_(view);

            // Make window visible and bring to front
            window.makeKeyAndOrderFront_(nil);

            // Make the view the first responder to receive keyboard events
            let _: () = msg_send![window, makeFirstResponder: view];
        }

        // Activate this application and bring to foreground
        // ignoring other applications
        let current_app = NSRunningApplication::currentApplication(nil);
        current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);

        // Hide the system mouse cursor
        // Our magnifier serves as a custom cursor
        let _: () = msg_send![class!(NSCursor), hide];

        // Start the application event loop
        // This blocks until the application terminates
        app.run();
    }
}

// =============================================================================
// CUSTOM VIEW CLASS REGISTRATION
// =============================================================================

/// Registers a custom NSView subclass called "ColorPickerView"
/// This view handles all user input (mouse, keyboard) and drawing
///
/// # Returns
/// A reference to the registered Objective-C class
fn register_view_class() -> &'static objc::runtime::Class {
    // Get the NSView class to use as our superclass
    let superclass = class!(NSView);

    // Create a new class declaration with name "ColorPickerView"
    let mut decl = ClassDecl::new("ColorPickerView", superclass).unwrap();

    unsafe {
        // Override acceptsFirstResponder to return YES
        // This allows the view to receive keyboard events
        decl.add_method(
            sel!(acceptsFirstResponder),
            accepts_first_responder as extern "C" fn(&Object, Sel) -> bool
        );

        // Handle mouse click events - exits the application
        decl.add_method(
            sel!(mouseDown:),
            mouse_down as extern "C" fn(&Object, Sel, id)
        );

        // Handle mouse movement - captures color under cursor
        decl.add_method(
            sel!(mouseMoved:),
            mouse_moved as extern "C" fn(&Object, Sel, id)
        );

        // Handle scroll wheel - adjusts zoom level
        decl.add_method(
            sel!(scrollWheel:),
            scroll_wheel as extern "C" fn(&Object, Sel, id)
        );

        // Handle keyboard input - ESC to exit, arrows to move cursor
        decl.add_method(
            sel!(keyDown:),
            key_down as extern "C" fn(&Object, Sel, id)
        );

        // Override drawRect: to draw the magnifier and color info
        decl.add_method(
            sel!(drawRect:),
            draw_rect as extern "C" fn(&Object, Sel, NSRect)
        );
    }

    // Register the class with the Objective-C runtime and return it
    decl.register()
}

/// Registers a custom NSWindow subclass called "KeyableWindow"
/// This window can become key window even though it's borderless
///
/// # Returns
/// A reference to the registered Objective-C class
///
/// # Note
/// Borderless windows normally return NO from canBecomeKeyWindow,
/// which prevents them from receiving keyboard events. We override
/// this to enable keyboard input.
fn register_window_class() -> &'static objc::runtime::Class {
    // Get NSWindow as our superclass
    let superclass = class!(NSWindow);

    // Create new class declaration
    let mut decl = ClassDecl::new("KeyableWindow", superclass).unwrap();

    unsafe {
        // Override canBecomeKeyWindow to return YES
        // Borderless windows normally return NO, preventing keyboard input
        decl.add_method(
            sel!(canBecomeKeyWindow),
            can_become_key_window as extern "C" fn(&Object, Sel) -> bool
        );
    }

    // Register and return the class
    decl.register()
}

// =============================================================================
// OBJECTIVE-C METHOD IMPLEMENTATIONS
// =============================================================================

/// Returns YES to indicate this window can become the key window
/// (receive keyboard events)
///
/// This overrides the default behavior of borderless windows which
/// normally cannot become key windows.
extern "C" fn can_become_key_window(_this: &Object, _cmd: Sel) -> bool {
    YES  // YES is defined as true in cocoa::base
}

/// Returns YES to indicate this view can become first responder
/// (receive keyboard events)
///
/// A view must be first responder to receive keyboard events.
extern "C" fn accepts_first_responder(_this: &Object, _cmd: Sel) -> bool {
    YES
}

/// Handles mouse click - exits the application
///
/// # Arguments
/// * `_this` - The view object (self)
/// * `_cmd` - The selector being called
/// * `_event` - The mouse event (unused)
///
/// When the user clicks, we unhide the cursor and terminate the app.
extern "C" fn mouse_down(_this: &Object, _cmd: Sel, _event: id) {
    unsafe {
        // Show the cursor again before exiting
        let _: () = msg_send![class!(NSCursor), unhide];

        // Get the application instance
        let app = NSApp();

        // Terminate the application
        let _: () = msg_send![app, terminate:nil];
    }
}

/// Handles mouse movement - captures color and updates display
///
/// # Arguments
/// * `_this` - The view object (self)
/// * `_cmd` - The selector being called
/// * `event` - The mouse event containing position info
///
/// This is called continuously as the mouse moves. It captures the color
/// under the cursor, updates the global state, and triggers a redraw.
extern "C" fn mouse_moved(_this: &Object, _cmd: Sel, event: id) {
    unsafe {
        // Get mouse location in window coordinates
        // This is relative to the window's content view
        let location: NSPoint = msg_send![event, locationInWindow];

        // Get the window containing this view
        let window: id = msg_send![_this, window];

        // Convert window coordinates to screen coordinates
        // Screen coordinates are needed for the capture functions
        let screen_location: NSPoint = msg_send![window, convertPointToScreen: location];

        // Get the color at the current mouse position
        if let Some((r, g, b)) = get_pixel_color(screen_location.x as f64, screen_location.y as f64) {
            // Convert normalized values (0.0-1.0) to integer (0-255)
            let r_int = (r * 255.0) as u8;
            let g_int = (g * 255.0) as u8;
            let b_int = (b * 255.0) as u8;

            // Format as hex color string like "#FF5733"
            let hex_color = format!("#{:02X}{:02X}{:02X}", r_int, g_int, b_int);

            // Update the global state with new position and color
            if let Ok(mut state) = MOUSE_STATE.lock() {
                *state = Some(MouseColorInfo {
                    x: location.x,
                    y: location.y,
                    screen_x: screen_location.x,
                    screen_y: screen_location.y,
                    hex_color: hex_color.clone(),
                });
            }

            // Print color info to terminal with ANSI codes to update in place
            // \r = carriage return (go to start of line)
            // \x1B[K = clear from cursor to end of line
            print!("\r\x1B[K");
            print!("RGB: ({:3}, {:3}, {:3})  |  HEX: {}  ", r_int, g_int, b_int, hex_color);

            // Flush stdout to ensure immediate display
            use std::io::{self, Write};
            io::stdout().flush().unwrap();

            // Request the view to redraw itself
            // This triggers drawRect: to be called
            let _: () = msg_send![_this, setNeedsDisplay: YES];
        }
    }
}

/// Handles scroll wheel - adjusts zoom level
///
/// # Arguments
/// * `_this` - The view object
/// * `_cmd` - The selector
/// * `event` - The scroll event
///
/// Scroll up = zoom in (larger magnifier)
/// Scroll down = zoom out (smaller magnifier)
extern "C" fn scroll_wheel(_this: &Object, _cmd: Sel, event: id) {
    unsafe {
        // Get vertical scroll delta
        // Positive = scroll up, Negative = scroll down
        let delta_y: f64 = msg_send![event, deltaY];

        if delta_y != 0.0 {
            // Update zoom level within bounds
            if let Ok(mut zoom) = CURRENT_ZOOM.lock() {
                // Scroll up = zoom in (larger), scroll down = zoom out (smaller)
                let new_zoom = *zoom + delta_y * ZOOM_STEP;

                // Clamp to min/max bounds
                *zoom = new_zoom.clamp(ZOOM_MIN, ZOOM_MAX);
            }

            // Request redraw to show new zoom level
            let _: () = msg_send![_this, setNeedsDisplay: YES];
        }
    }
}

/// Handles keyboard input - ESC to exit, arrows to move cursor
///
/// # Arguments
/// * `_this` - The view object
/// * `_cmd` - The selector
/// * `event` - The keyboard event
///
/// Key codes:
/// - 53 = ESC (exit application)
/// - 123 = Left arrow
/// - 124 = Right arrow
/// - 125 = Down arrow
/// - 126 = Up arrow
///
/// Hold Shift for faster movement (SHIFT_MOVE_PIXELS instead of 1)
extern "C" fn key_down(_this: &Object, _cmd: Sel, event: id) {
    unsafe {
        // Get the key code (hardware-independent key identifier)
        let key_code: u16 = msg_send![event, keyCode];

        // Get modifier flags (Shift, Ctrl, etc.)
        let modifier_flags: u64 = msg_send![event, modifierFlags];

        // Check if Shift key is pressed (bit 17 in modifier flags)
        let shift_pressed = (modifier_flags & (1 << 17)) != 0;

        // Move 1 pixel normally, or SHIFT_MOVE_PIXELS with Shift held
        let move_amount = if shift_pressed { SHIFT_MOVE_PIXELS } else { 1.0 };

        // Key code 53 = Escape key
        if key_code == 53 {
            // Show cursor and exit
            let _: () = msg_send![class!(NSCursor), unhide];
            let app = NSApp();
            let _: () = msg_send![app, terminate:nil];
            return;
        }

        // Arrow keys:
        // 123 = Left, 124 = Right, 125 = Down, 126 = Up
        let (dx, dy): (f64, f64) = match key_code {
            123 => (-move_amount, 0.0),   // Left arrow - move left
            124 => (move_amount, 0.0),    // Right arrow - move right
            125 => (0.0, -move_amount),   // Down arrow - move down
            126 => (0.0, move_amount),    // Up arrow - move up
            _ => (0.0, 0.0),              // Other keys - no movement
        };

        // Only process if there's actual movement
        if dx != 0.0 || dy != 0.0 {
            // Create a Core Graphics event to get current mouse position
            // CGEvent gives us the actual system cursor position
            let cg_event = core_graphics::event::CGEvent::new(
                core_graphics::event_source::CGEventSource::new(
                    core_graphics::event_source::CGEventSourceStateID::HIDSystemState
                ).unwrap()
            ).unwrap();

            // Get current cursor position in CG coordinates
            let current_pos = cg_event.location();

            // Calculate new position
            // Note: CG Y axis is inverted vs Cocoa (top-left vs bottom-left origin)
            let new_x = current_pos.x + dx;
            let new_y = current_pos.y - dy;  // Subtract because CG Y increases downward

            // Create new position point
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

            // Post the event to the system event queue
            // This makes the system cursor actually move
            move_event.post(core_graphics::event::CGEventTapLocation::HID);

            // Convert CG coordinates back to Cocoa for color picking
            let main_display = CGDisplay::main();
            let screen_height = main_display.pixels_high() as f64;
            let cocoa_y = screen_height - new_y;  // Convert back to Cocoa Y

            // Get color at new position
            if let Some((r, g, b)) = get_pixel_color(new_x, cocoa_y) {
                // Convert to integer RGB
                let r_int = (r * 255.0) as u8;
                let g_int = (g * 255.0) as u8;
                let b_int = (b * 255.0) as u8;

                // Format hex string
                let hex_color = format!("#{:02X}{:02X}{:02X}", r_int, g_int, b_int);

                // Update global state
                if let Ok(mut state) = MOUSE_STATE.lock() {
                    // Get window for coordinate conversion
                    let window: id = msg_send![_this, window];
                    let screen_point = NSPoint::new(new_x, cocoa_y);

                    // Convert screen coordinates to window coordinates
                    let window_point: NSPoint = msg_send![window, convertPointFromScreen: screen_point];

                    *state = Some(MouseColorInfo {
                        x: window_point.x,
                        y: window_point.y,
                        screen_x: new_x,
                        screen_y: cocoa_y,
                        hex_color: hex_color.clone(),
                    });
                }

                // Update terminal display
                print!("\r\x1B[K");
                print!("RGB: ({:3}, {:3}, {:3})  |  HEX: {}  ", r_int, g_int, b_int, hex_color);
                use std::io::{self, Write};
                io::stdout().flush().unwrap();

                // Request view redraw
                let _: () = msg_send![_this, setNeedsDisplay: YES];
            }
        }
    }
}

// =============================================================================
// DRAWING IMPLEMENTATION
// =============================================================================

/// Draws the magnifier, border, and hex color text
/// Called automatically when the view needs to be redrawn
///
/// # Arguments
/// * `_this` - The view object
/// * `_cmd` - The selector
/// * `_rect` - The rectangle that needs to be redrawn (we redraw everything)
///
/// Drawing order:
/// 1. Faint overlay on entire screen
/// 2. Magnified pixel grid (circular, clipped)
/// 3. Center reticle (white circle marking target pixel)
/// 4. Colored border (shows current color)
/// 5. Hex text along the border arc
extern "C" fn draw_rect(_this: &Object, _cmd: Sel, _rect: NSRect) {
    unsafe {
        // Get NSColor class for color operations
        let cls = class!(NSColor);

        // =====================================================================
        // DRAW FAINT OVERLAY
        // =====================================================================

        // Create a very faint black overlay (5% opacity)
        // This slightly dims the screen to show the picker is active
        let color: id = msg_send![cls, colorWithCalibratedWhite:0.0 alpha:0.05];

        // Set as the current fill color
        let _: () = msg_send![color, set];

        // Get the view's bounds (full size)
        let bounds: NSRect = msg_send![_this, bounds];

        // Fill the entire view with the overlay color
        cocoa::appkit::NSRectFill(bounds);

        // =====================================================================
        // DRAW MAGNIFIER (only if we have mouse state)
        // =====================================================================

        if let Ok(state) = MOUSE_STATE.lock() {
            if let Some(ref info) = *state {
                // Get current zoom level from global state
                let current_zoom = match CURRENT_ZOOM.lock() {
                    Ok(z) => *z,
                    Err(_) => INITIAL_ZOOM_FACTOR,  // Fallback to default
                };

                // Calculate magnifier size based on zoom
                // mag_size = number of captured pixels * zoom factor
                let mag_size = CAPTURED_PIXELS * current_zoom;

                // Capture the screen area around the cursor
                if let Some(cg_image) = capture_zoom_area(info.screen_x, info.screen_y, CAPTURED_PIXELS) {

                    // =============================================================
                    // CREATE NSIMAGE FROM CGIMAGE
                    // =============================================================

                    // Allocate NSImage
                    let ns_image_cls = class!(NSImage);
                    let ns_image: id = msg_send![ns_image_cls, alloc];

                    // Get the raw CGImageRef pointer from the CGImage wrapper
                    // This is necessary because the Rust CGImage wrapper doesn't
                    // directly expose the pointer in a way msg_send! can use
                    let cg_image_ptr = {
                        let ptr_addr = &cg_image as *const CGImage as *const *const core_graphics::sys::CGImage;
                        *ptr_addr
                    };

                    // Create NSSize for the image dimensions
                    let size = cocoa::foundation::NSSize::new(
                        cg_image.width() as f64,
                        cg_image.height() as f64
                    );

                    // Initialize NSImage with the CGImage
                    let ns_image: id = msg_send![ns_image, initWithCGImage:cg_image_ptr size:size];

                    // =============================================================
                    // CALCULATE MAGNIFIER POSITION AND SIZE
                    // =============================================================

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

                    // =============================================================
                    // DRAW MAGNIFIED IMAGE WITH CIRCULAR CLIPPING
                    // =============================================================

                    // Create circular clipping path
                    let circular_clip: id = msg_send![path_cls, bezierPathWithOvalInRect: mag_rect];

                    // Save current graphics state (so we can restore after clipping)
                    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];

                    // Disable image interpolation to get sharp pixels
                    // By default, macOS applies bilinear/bicubic interpolation when scaling,
                    // which creates smooth gradients between pixels. For a color picker,
                    // we need each pixel to show its exact color without blending.
                    // NSImageInterpolationNone = 1
                    let graphics_context: id = msg_send![class!(NSGraphicsContext), currentContext];
                    let _: () = msg_send![graphics_context, setImageInterpolation: 1u64];

                    // Apply circular clipping - everything drawn after this is clipped to the circle
                    let _: () = msg_send![circular_clip, addClip];

                    // Draw the magnified image
                    let from_rect = NSRect::new(
                        NSPoint::new(0.0, 0.0),
                        size  // Source rectangle (entire image)
                    );

                    // Draw image scaled to mag_rect (magnified)
                    // operation: 2 = NSCompositingOperationSourceOver (normal alpha blending)
                    // fraction: 1.0 = fully opaque
                    let _: () = msg_send![ns_image, drawInRect:mag_rect
                                          fromRect:from_rect
                                          operation:2u64
                                          fraction:1.0];

                    // Restore graphics state (removes clipping)
                    let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];

                    // =============================================================
                    // DRAW CENTER RETICLE (TARGET INDICATOR)
                    // =============================================================

                    // Calculate size of one magnified pixel
                    let pixel_size = mag_size / CAPTURED_PIXELS;

                    // Calculate center of magnifier
                    let center_x = mag_x + mag_size / 2.0;
                    let center_y = mag_y + mag_size / 2.0;

                    // Reticle is slightly smaller than one pixel (80%)
                    let reticle_radius = pixel_size * 0.8;
                    let diameter = reticle_radius * 2.0;

                    // Create rectangle for reticle circle
                    let circle_rect = NSRect::new(
                        NSPoint::new(center_x - reticle_radius, center_y - reticle_radius),
                        cocoa::foundation::NSSize::new(diameter, diameter)
                    );

                    // Set stroke color to white for visibility
                    let white_color: id = msg_send![cls, whiteColor];
                    let _: () = msg_send![white_color, setStroke];

                    // Create and draw the reticle circle
                    let reticle_path: id = msg_send![path_cls, bezierPathWithOvalInRect: circle_rect];
                    let _: () = msg_send![reticle_path, setLineWidth: 2.0];
                    let _: () = msg_send![reticle_path, stroke];

                    // =============================================================
                    // DRAW COLORED BORDER
                    // =============================================================

                    // Parse hex color to get RGB values for the border
                    let hex = &info.hex_color[1..]; // Skip the '#'
                    let r_val = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f64 / 255.0;
                    let g_val = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f64 / 255.0;
                    let b_val = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f64 / 255.0;

                    // Create rectangle for border (larger than magnifier by BORDER_WIDTH)
                    let border_rect = NSRect::new(
                        NSPoint::new(mag_x - BORDER_WIDTH / 2.0, mag_y - BORDER_WIDTH / 2.0),
                        cocoa::foundation::NSSize::new(mag_size + BORDER_WIDTH, mag_size + BORDER_WIDTH)
                    );

                    // Create color from RGB values
                    let border_color: id = msg_send![cls, colorWithCalibratedRed:r_val green:g_val blue:b_val alpha:1.0];
                    let _: () = msg_send![border_color, setStroke];

                    // Draw the border circle
                    let border_path: id = msg_send![path_cls, bezierPathWithOvalInRect: border_rect];
                    let _: () = msg_send![border_path, setLineWidth: BORDER_WIDTH];
                    let _: () = msg_send![border_path, stroke];

                    // =============================================================
                    // DRAW HEX COLOR TEXT ALONG THE BORDER ARC
                    // =============================================================

                    // Use a bold system font for visibility
                    let font_cls = class!(NSFont);

                    // Use systemFontOfSize:weight: with bold weight (0.62)
                    // Available weights: 0.0 (ultralight) to 1.0 (black)
                    let font: id = msg_send![font_cls, systemFontOfSize: HEX_FONT_SIZE weight: 0.62f64];

                    // Calculate contrasting text color based on border color luminance
                    // Uses standard luminance formula: 0.299*R + 0.587*G + 0.114*B
                    // This ensures text is always readable against the border
                    let luminance = 0.299 * r_val + 0.587 * g_val + 0.114 * b_val;

                    // Create the appropriate text color based on luminance
                    let text_color: id = if luminance > 0.5 {
                        // Dark text on light background
                        msg_send![cls, colorWithCalibratedRed: 0.0f64 green: 0.0f64 blue: 0.0f64 alpha: 1.0f64]
                    } else {
                        // Light text on dark background
                        msg_send![cls, colorWithCalibratedRed: 1.0f64 green: 1.0f64 blue: 1.0f64 alpha: 1.0f64]
                    };

                    // =============================================================
                    // POSITION AND DRAW EACH CHARACTER ALONG THE ARC
                    // =============================================================

                    // Get the hex text to draw
                    let hex_text = &info.hex_color;
                    let char_count = hex_text.len() as f64;

                    // Radius for text placement (center of border)
                    let radius = mag_size / 2.0 + BORDER_WIDTH / 2.0;

                    // Calculate fixed character spacing in pixels
                    // This ensures consistent spacing regardless of zoom level
                    // Convert pixel spacing to angle (arc length = radius * angle)
                    // angle = arc_length / radius
                    let angle_step = CHAR_SPACING_PIXELS / radius;

                    // Total arc span for all characters
                    let total_arc = angle_step * (char_count - 1.0);

                    // Start angle: center the text at top of circle (90 degrees = PI/2)
                    let start_angle: f64 = std::f64::consts::PI / 2.0 + total_arc / 2.0;

                    // Save graphics state before transforms
                    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];

                    // Draw each character individually, rotated to follow the arc
                    for (i, c) in hex_text.chars().enumerate() {
                        // Calculate angle for this character
                        // Start from start_angle and subtract (go clockwise from left to right)
                        let angle = start_angle - angle_step * (i as f64);

                        // Calculate position on the arc using polar coordinates
                        let char_x = center_x + radius * angle.cos();
                        let char_y = center_y + radius * angle.sin();

                        // Create NSString for the single character
                        let char_str = c.to_string();
                        let ns_char = NSString::alloc(nil);
                        let ns_char = NSString::init_str(ns_char, &char_str);

                        // Create fresh attributes dictionary for each character
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

                        // Create affine transform for rotation
                        let transform_cls = class!(NSAffineTransform);
                        let transform: id = msg_send![transform_cls, transform];

                        // Translate to character position on the arc
                        let _: () = msg_send![transform, translateXBy:char_x yBy:char_y];

                        // Rotate to follow the arc (perpendicular to radius)
                        // Subtract PI/2 to make text face outward from center
                        let rotation_angle = angle - std::f64::consts::PI / 2.0;
                        let _: () = msg_send![transform, rotateByRadians:rotation_angle];

                        // Apply the transform to the graphics context
                        let _: () = msg_send![transform, concat];

                        // Draw character centered at origin (which is now at arc position)
                        let draw_point = NSPoint::new(-char_size.width / 2.0, -char_size.height / 2.0);
                        let _: () = msg_send![ns_char, drawAtPoint:draw_point withAttributes:attributes];

                        // Reset transform for next character by applying inverse
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