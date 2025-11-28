use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSAutoreleasePool, NSRect, NSArray, NSPoint, NSString};
use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationPolicyRegular, NSBackingStoreBuffered,
    NSWindow, NSWindowStyleMask,
    NSRunningApplication, NSApplicationActivateIgnoringOtherApps, NSScreen
};
use objc::{class, msg_send, sel, sel_impl};
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel};
use core_graphics::display::CGDisplay;
use core_graphics::window::{kCGWindowListOptionAll, kCGNullWindowID, kCGWindowImageDefault};
use std::sync::Mutex;

// Global state to store mouse position and color
static MOUSE_STATE: Mutex<Option<MouseColorInfo>> = Mutex::new(None);

#[derive(Clone)]
struct MouseColorInfo {
    x: f64,
    y: f64,
    hex_color: String,
}

/// Captures the color of the pixel at the given screen coordinates
/// Returns (r, g, b) as f64 values in range 0.0-1.0
fn get_pixel_color(x: f64, y: f64) -> Option<(f64, f64, f64)> {
    use core_graphics::geometry::{CGRect, CGPoint as CGPointStruct, CGSize};

    // Create a 1x1 rect around the target pixel
    let rect = CGRect::new(
        &CGPointStruct::new(x, y),
        &CGSize::new(1.0, 1.0)
    );

    // Capture screenshot of that rect
    let image = CGDisplay::screenshot(
        rect,
        kCGWindowListOptionAll,
        kCGNullWindowID,
        kCGWindowImageDefault,
    )?;

    // Get the pixel data
    let data = image.data();
    let data_len = data.len() as usize;

    // We should have at least 4 bytes (BGRA format)
    if data_len >= 4 {
        // Most Mac displays use BGRA format
        let b = data[0] as f64 / 255.0;
        let g = data[1] as f64 / 255.0;
        let r = data[2] as f64 / 255.0;

        Some((r, g, b))
    } else {
        None
    }
}

fn main() {
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        // Initialize the application
        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicyRegular);

        // Register our custom view class to handle events
        let view_class = register_view_class();

        // Create a window for each screen to cover the entire desktop
        let screens = NSScreen::screens(nil);
        let count = screens.count();

        for i in 0..count {
            let screen = screens.objectAtIndex(i);
            // Use msg_send! for frame to avoid trait ambiguity (NSScreen vs NSView)
            let frame: NSRect = msg_send![screen, frame];

            // Create the window
            // Use msg_send! for alloc to avoid trait ambiguity with NSWindow
            let window_alloc: id = msg_send![class!(NSWindow), alloc];
            let window = window_alloc.initWithContentRect_styleMask_backing_defer_(
                frame,
                NSWindowStyleMask::NSBorderlessWindowMask,
                NSBackingStoreBuffered,
                NO
            );

            // Configure the window level to be above almost everything (ScreenSaver level)
            // CGWindowLevelKey::kCGScreenSaverWindowLevel is usually 1000 or higher.
            // In Cocoa, NSScreenSaverWindowLevel is 1000.
            window.setLevel_(1000); 

            // Make it transparent but interactive
                        // Use msg_send! for NSColor to avoid trait ambiguity
            let cls_color = class!(NSColor);
            let clear_color: id = msg_send![cls_color, clearColor];

            window.setBackgroundColor_(clear_color);
            window.setOpaque_(NO);
            window.setHasShadow_(NO);
            window.setIgnoresMouseEvents_(NO); // We want to capture mouse events
            window.setAcceptsMouseMovedEvents_(YES); // Enable mouse moved events

            // Create and set the custom view
            let view: id = msg_send![view_class, alloc];
            let view: id = msg_send![view, initWithFrame:frame];
            window.setContentView_(view);

            // Make window key and visible
            window.makeKeyAndOrderFront_(nil);

            // Make the window and view first responder to receive key events
            let _: () = msg_send![window, makeFirstResponder: view];
        }

        // Activate the app to ensure it captures input immediately
        let current_app = NSRunningApplication::currentApplication(nil);
        current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);

        app.run();
    }
}

fn register_view_class() -> &'static objc::runtime::Class {
    let superclass = class!(NSView);
    let mut decl = ClassDecl::new("MouseBlockerView", superclass).unwrap();

    unsafe {
        // Allow the view to become the first responder (to capture key events)
        decl.add_method(sel!(acceptsFirstResponder), accepts_first_responder as extern "C" fn(&Object, Sel) -> bool);

        // Handle mouse down - Exit on click
        decl.add_method(sel!(mouseDown:), mouse_down as extern "C" fn(&Object, Sel, id));

        // Handle mouse moved - Capture color
        decl.add_method(sel!(mouseMoved:), mouse_moved as extern "C" fn(&Object, Sel, id));

        // Handle key down - Exit on ESC
        decl.add_method(sel!(keyDown:), key_down as extern "C" fn(&Object, Sel, id));

        // Optional: Draw a semi-transparent overlay if needed (currently clear)
        decl.add_method(sel!(drawRect:), draw_rect as extern "C" fn(&Object, Sel, NSRect));
    }

    decl.register()
}

extern "C" fn accepts_first_responder(_this: &Object, _cmd: Sel) -> bool {
    YES
}

extern "C" fn mouse_down(_this: &Object, _cmd: Sel, _event: id) {
    // Exit on left click
    unsafe {
        let app = NSApp();
        let _: () = msg_send![app, terminate:nil];
    }
}

extern "C" fn mouse_moved(_this: &Object, _cmd: Sel, event: id) {
    unsafe {
        // Get mouse location in window coordinates (for drawing)
        let location: NSPoint = msg_send![event, locationInWindow];

        // Get mouse location in screen coordinates (for color picking)
        let window: id = msg_send![_this, window];
        let screen_location: NSPoint = msg_send![window, convertPointToScreen: location];

        // Get the color at this pixel
        if let Some((r, g, b)) = get_pixel_color(screen_location.x as f64, screen_location.y as f64) {
            // Convert to 0-255 range
            let r_int = (r * 255.0) as u8;
            let g_int = (g * 255.0) as u8;
            let b_int = (b * 255.0) as u8;

            // Create hex string
            let hex_color = format!("#{:02X}{:02X}{:02X}", r_int, g_int, b_int);

            // Update global state
            if let Ok(mut state) = MOUSE_STATE.lock() {
                *state = Some(MouseColorInfo {
                    x: location.x,
                    y: location.y,
                    hex_color: hex_color.clone(),
                });
            }

            // Display in terminal with ANSI escape codes to overwrite the previous line
            print!("\r\x1B[K"); // Clear line
            print!("RGB: ({:3}, {:3}, {:3})  |  HEX: {}  ",
                   r_int, g_int, b_int, hex_color);
            use std::io::{self, Write};
            io::stdout().flush().unwrap();

            // Request view redraw
            let _: () = msg_send![_this, setNeedsDisplay: YES];
        }
    }
}

extern "C" fn key_down(_this: &Object, _cmd: Sel, event: id) {
    unsafe {
        let key_code: u16 = msg_send![event, keyCode];
        // 53 is the key code for Escape on macOS
        if key_code == 53 {
            let app = NSApp();
            let _: () = msg_send![app, terminate:nil];
        }
    }
}

extern "C" fn draw_rect(_this: &Object, _cmd: Sel, _rect: NSRect) {
    unsafe {
        // Draw a very faint black overlay to indicate the shield is active (optional)
        // 5% opacity black
        let cls = class!(NSColor);
        let color: id = msg_send![cls, colorWithCalibratedWhite:0.0 alpha:0.05];

        let _: () = msg_send![color, set];
        let bounds: NSRect = msg_send![_this, bounds];
        cocoa::appkit::NSRectFill(bounds);

        // Draw the hex color value near the mouse cursor
        if let Ok(state) = MOUSE_STATE.lock() {
            if let Some(ref info) = *state {
                // Set up font
                let font_cls = class!(NSFont);
                let font: id = msg_send![font_cls, boldSystemFontOfSize: 18.0];

                // Create NSString from the hex color
                let ns_str = NSString::alloc(nil);
                let ns_str = NSString::init_str(ns_str, &info.hex_color);

                // Calculate text size for background
                let white_color: id = msg_send![cls, whiteColor];

                // Create attributes dictionary
                let dict_cls = class!(NSDictionary);
                let ns_string_cls = class!(NSString);

                // Get the proper attribute key names from NSAttributedString
                let font_attr_name: id = msg_send![ns_string_cls, stringWithUTF8String: "NSFont".as_ptr()];
                let color_attr_name: id = msg_send![ns_string_cls, stringWithUTF8String: "NSForegroundColor".as_ptr()];

                let keys: Vec<id> = vec![font_attr_name, color_attr_name];
                let values: Vec<id> = vec![font, white_color];

                let attributes: id = msg_send![dict_cls, dictionaryWithObjects:values.as_ptr() forKeys:keys.as_ptr() count:2usize];

                // Calculate text position (to the right of the cursor)
                let text_x = info.x + 20.0;
                let text_y = info.y - 8.0;
                let text_point = NSPoint::new(text_x, text_y);

                // Draw background rectangle for better visibility
                let text_size: cocoa::foundation::NSSize = msg_send![ns_str, sizeWithAttributes: attributes];
                let padding = 8.0;
                let bg_rect = NSRect::new(
                    NSPoint::new(text_x - padding, text_y - padding / 2.0),
                    cocoa::foundation::NSSize::new(text_size.width + padding * 2.0, text_size.height + padding)
                );

                // Draw black background with full opacity
                let bg_color: id = msg_send![cls, whiteColor];
                let _: () = msg_send![bg_color, setFill];
                cocoa::appkit::NSRectFill(bg_rect);

                // Draw the text
                let _: () = msg_send![ns_str, drawAtPoint:text_point withAttributes:attributes];
            }
        }
    }
}

