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
use core_graphics::image::CGImage;
use std::sync::Mutex;

// ============================================================================
// CONFIGURATION
// ============================================================================

/// Thickness of the colored border around the magnifier (in pixels)
const BORDER_WIDTH: f64 = 20.0;

/// Font size for the hex color text displayed on the border (in points)
const HEX_FONT_SIZE: f64 = 14.0;

/// Number of screen pixels captured by the magnifier
const CAPTURED_PIXELS: f64 = 8.0;

/// Zoom factor for the magnifier (magnifier size = CAPTURED_PIXELS * ZOOM_FACTOR)
const ZOOM_FACTOR: f64 = 20.0;

/// Number of pixels to move when pressing Shift + Arrow key
const SHIFT_MOVE_PIXELS: f64 = 50.0;

// ============================================================================

// Global state to store mouse position and color
static MOUSE_STATE: Mutex<Option<MouseColorInfo>> = Mutex::new(None);

struct MouseColorInfo {
    x: f64,
    y: f64,
    screen_x: f64,
    screen_y: f64,
    hex_color: String,
}

/// Captures a zoomed area around the cursor for the magnifier effect
fn capture_zoom_area(x: f64, y: f64, size: f64) -> Option<CGImage> {
    use core_graphics::geometry::{CGRect, CGPoint as CGPointStruct, CGSize};

    // Convert Cocoa coordinates (origin bottom-left) to CG coordinates (origin top-left)
    let main_display = CGDisplay::main();
    let screen_height = main_display.pixels_high() as f64;
    let cg_y = screen_height - y;

    // Use the main display to capture directly from screen
    let half_size = size / 2.0;
    let rect = CGRect::new(
        &CGPointStruct::new(x - half_size, cg_y - half_size),
        &CGSize::new(size, size)
    );

    main_display.image_for_rect(rect)
}

/// Captures the color of the pixel at the given screen coordinates
/// Returns (r, g, b) as f64 values in range 0.0-1.0
fn get_pixel_color(x: f64, y: f64) -> Option<(f64, f64, f64)> {
    use core_graphics::geometry::{CGRect, CGPoint as CGPointStruct, CGSize};

    // Convert Cocoa coordinates (origin bottom-left) to CG coordinates (origin top-left)
    let main_display = CGDisplay::main();
    let screen_height = main_display.pixels_high() as f64;
    let cg_y = screen_height - y;

    // Create a 1x1 rect around the target pixel
    let rect = CGRect::new(
        &CGPointStruct::new(x, cg_y),
        &CGSize::new(1.0, 1.0)
    );

    // Capture screenshot directly from display
    let image = main_display.image_for_rect(rect)?;

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
    // Display instructions including permission requirements
    println!("\n╔═══════════════════════════════════════════════════╗");
    println!("║         Sélecteur de couleur - Color Picker      ║");
    println!("╠═══════════════════════════════════════════════════╣");
    println!("║  • Déplacez la souris pour capturer la couleur   ║");
    println!("║  • Clic gauche ou ESC pour quitter               ║");
    println!("╠═══════════════════════════════════════════════════╣");
    println!("║  ⚠️  IMPORTANT - Permissions requises:            ║");
    println!("║                                                   ║");
    println!("║  Cette app nécessite la permission               ║");
    println!("║  \"Enregistrement d'écran\"                         ║");
    println!("║                                                   ║");
    println!("║  Si les couleurs ne s'affichent pas:             ║");
    println!("║  1. Ouvrez Préférences Système                   ║");
    println!("║  2. Sécurité et confidentialité                  ║");
    println!("║  3. Confidentialité > Enregistrement d'écran     ║");
    println!("║  4. Activez cette application                    ║");
    println!("║  5. Relancez l'application                       ║");
    println!("╚═══════════════════════════════════════════════════╝\n");

    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        // Initialize the application
        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicyRegular);

        // Register our custom view class to handle events
        let view_class = register_view_class();
        
        // Register our custom window class that can become key window
        let window_class = register_window_class();

        // Create a window for each screen to cover the entire desktop
        let screens = NSScreen::screens(nil);
        let count = screens.count();

        for i in 0..count {
            let screen = screens.objectAtIndex(i);
            // Use msg_send! for frame to avoid trait ambiguity (NSScreen vs NSView)
            let frame: NSRect = msg_send![screen, frame];

            // Create the window using our custom KeyableWindow class
            let window_alloc: id = msg_send![window_class, alloc];
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

            // Exclude this window from screen captures
            let _: () = msg_send![window, setSharingType: 0u64]; // NSWindowSharingNone

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

        // Hide the mouse cursor
        let _: () = msg_send![class!(NSCursor), hide];

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

fn register_window_class() -> &'static objc::runtime::Class {
    let superclass = class!(NSWindow);
    let mut decl = ClassDecl::new("KeyableWindow", superclass).unwrap();

    unsafe {
        // Allow borderless window to become key window (receive keyboard events)
        decl.add_method(sel!(canBecomeKeyWindow), can_become_key_window as extern "C" fn(&Object, Sel) -> bool);
    }

    decl.register()
}

extern "C" fn can_become_key_window(_this: &Object, _cmd: Sel) -> bool {
    YES
}

extern "C" fn accepts_first_responder(_this: &Object, _cmd: Sel) -> bool {
    YES
}

extern "C" fn mouse_down(_this: &Object, _cmd: Sel, _event: id) {
    // Exit on left click
    unsafe {
        // Show the cursor again before exiting
        let _: () = msg_send![class!(NSCursor), unhide];
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
                    screen_x: screen_location.x,
                    screen_y: screen_location.y,
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
        let modifier_flags: u64 = msg_send![event, modifierFlags];
        
        // Check if Shift is pressed (NSEventModifierFlagShift = 1 << 17)
        let shift_pressed = (modifier_flags & (1 << 17)) != 0;
        let move_amount = if shift_pressed { SHIFT_MOVE_PIXELS } else { 1.0 };
        
        // 53 is the key code for Escape on macOS
        if key_code == 53 {
            // Show the cursor again before exiting
            let _: () = msg_send![class!(NSCursor), unhide];
            let app = NSApp();
            let _: () = msg_send![app, terminate:nil];
        }
        
        // Arrow keys: 123=Left, 124=Right, 125=Down, 126=Up
        let (dx, dy): (f64, f64) = match key_code {
            123 => (-move_amount, 0.0),  // Left
            124 => (move_amount, 0.0),   // Right
            125 => (0.0, -move_amount),  // Down (screen coords: down = negative y)
            126 => (0.0, move_amount),   // Up
            _ => (0.0, 0.0),
        };
        
        if dx != 0.0 || dy != 0.0 {
            // Get current mouse position
            let cg_event = core_graphics::event::CGEvent::new(core_graphics::event_source::CGEventSource::new(core_graphics::event_source::CGEventSourceStateID::HIDSystemState).unwrap()).unwrap();
            let current_pos = cg_event.location();
            
            // Calculate new position
            let new_x = current_pos.x + dx;
            let new_y = current_pos.y - dy; // CG coordinates: y increases downward
            
            // Move the mouse cursor
            let new_pos = core_graphics::geometry::CGPoint::new(new_x, new_y);
            let move_event = core_graphics::event::CGEvent::new_mouse_event(
                core_graphics::event_source::CGEventSource::new(core_graphics::event_source::CGEventSourceStateID::HIDSystemState).unwrap(),
                core_graphics::event::CGEventType::MouseMoved,
                new_pos,
                core_graphics::event::CGMouseButton::Left,
            ).unwrap();
            move_event.post(core_graphics::event::CGEventTapLocation::HID);
            
            // Update the color at new position
            // Convert CG coordinates to Cocoa coordinates for color picking
            let main_display = CGDisplay::main();
            let screen_height = main_display.pixels_high() as f64;
            let cocoa_y = screen_height - new_y;
            
            if let Some((r, g, b)) = get_pixel_color(new_x, cocoa_y) {
                let r_int = (r * 255.0) as u8;
                let g_int = (g * 255.0) as u8;
                let b_int = (b * 255.0) as u8;
                let hex_color = format!("#{:02X}{:02X}{:02X}", r_int, g_int, b_int);
                
                // Update global state
                if let Ok(mut state) = MOUSE_STATE.lock() {
                    // Get window for coordinate conversion
                    let window: id = msg_send![_this, window];
                    let screen_point = NSPoint::new(new_x, cocoa_y);
                    let window_point: NSPoint = msg_send![window, convertPointFromScreen: screen_point];
                    
                    *state = Some(MouseColorInfo {
                        x: window_point.x,
                        y: window_point.y,
                        screen_x: new_x,
                        screen_y: cocoa_y,
                        hex_color: hex_color.clone(),
                    });
                }
                
                // Display in terminal
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

extern "C" fn draw_rect(_this: &Object, _cmd: Sel, _rect: NSRect) {
    unsafe {
        // Draw a very faint black overlay to indicate the shield is active (optional)
        // 5% opacity black
        let cls = class!(NSColor);
        let color: id = msg_send![cls, colorWithCalibratedWhite:0.0 alpha:0.05];

        let _: () = msg_send![color, set];
        let bounds: NSRect = msg_send![_this, bounds];
        cocoa::appkit::NSRectFill(bounds);

        // Draw the magnifier/zoom effect and hex color value near the mouse cursor
        if let Ok(state) = MOUSE_STATE.lock() {
            if let Some(ref info) = *state {
                // Capture and draw the magnifier
                if let Some(cg_image) = capture_zoom_area(info.screen_x, info.screen_y, CAPTURED_PIXELS) {
                    // Convert CGImage to NSImage
                    let ns_image_cls = class!(NSImage);
                    let ns_image: id = msg_send![ns_image_cls, alloc];

                    // Get CGImageRef from CGImage
                    let cg_image_ptr = {
                        use core_graphics::sys::CGImageRef;
                        let ptr_addr = &cg_image as *const CGImage as *const *const core_graphics::sys::CGImage;
                        *ptr_addr
                    };

                    let size = cocoa::foundation::NSSize::new(
                        cg_image.width() as f64,
                        cg_image.height() as f64
                    );
                    let ns_image: id = msg_send![ns_image, initWithCGImage:cg_image_ptr size:size];

                    // Define magnifier size
                    let mag_size = CAPTURED_PIXELS * ZOOM_FACTOR;
                    let mag_x = info.x - mag_size / 2.0;
                    let mag_y = info.y - mag_size / 2.0;

                    let mag_rect = NSRect::new(
                        NSPoint::new(mag_x, mag_y),
                        cocoa::foundation::NSSize::new(mag_size, mag_size)
                    );

                    let path_cls = class!(NSBezierPath);

                    // Create circular clipping path for magnifier
                    let circular_clip: id = msg_send![path_cls, bezierPathWithOvalInRect: mag_rect];
                    
                    // Save graphics state before clipping
                    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
                    
                    // Disable image interpolation to get sharp pixels without antialiasing.
                    // By default, macOS applies bilinear/bicubic interpolation when scaling images,
                    // which creates smooth gradients between pixels. For a color picker, we need
                    // each grid square to show the exact color of a single screen pixel, not a
                    // blended average of neighboring pixels.
                    let graphics_context: id = msg_send![class!(NSGraphicsContext), currentContext];
                    let _: () = msg_send![graphics_context, setImageInterpolation: 1u64]; // NSImageInterpolationNone = 1
                    
                    let _: () = msg_send![circular_clip, addClip];

                    // Draw the magnified image (clipped to circle)
                    let from_rect = NSRect::new(
                        NSPoint::new(0.0, 0.0),
                        size
                    );
                    let _: () = msg_send![ns_image, drawInRect:mag_rect
                                          fromRect:from_rect
                                          operation:2u64
                                          fraction:1.0];

                    // Restore graphics state (remove circular clipping)
                    let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];

                    // Draw white circle outline for reticle at center
                    let pixel_size = mag_size / CAPTURED_PIXELS;
                    let center_x = mag_x + mag_size / 2.0;
                    let center_y = mag_y + mag_size / 2.0;
                    let reticle_radius = pixel_size * 0.8;
                    let diameter = reticle_radius * 2.0;
                    let circle_rect = NSRect::new(
                        NSPoint::new(center_x - reticle_radius, center_y - reticle_radius),
                        cocoa::foundation::NSSize::new(diameter, diameter)
                    );
                    let white_color: id = msg_send![cls, whiteColor];
                    let _: () = msg_send![white_color, setStroke];
                    let reticle_path: id = msg_send![path_cls, bezierPathWithOvalInRect: circle_rect];
                    let _: () = msg_send![reticle_path, setLineWidth: 2.0];
                    let _: () = msg_send![reticle_path, stroke];

                    // Draw circular border around magnifier with current pixel color
                    // Parse hex color to get RGB values
                    let hex = &info.hex_color[1..]; // Skip the '#'
                    let r_val = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f64 / 255.0;
                    let g_val = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f64 / 255.0;
                    let b_val = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f64 / 255.0;
                    
                    // Draw thick colored border
                    let border_color: id = msg_send![cls, colorWithCalibratedRed:r_val green:g_val blue:b_val alpha:1.0];
                    let _: () = msg_send![border_color, setStroke];
                    let border_path: id = msg_send![path_cls, bezierPathWithOvalInRect: mag_rect];
                    let _: () = msg_send![border_path, setLineWidth: BORDER_WIDTH];
                    let _: () = msg_send![border_path, stroke];

                    // Draw hex color text along the circular border
                    let font_cls = class!(NSFont);
                    // Use heavy weight font for thicker text
                    let font_manager: id = msg_send![class!(NSFontManager), sharedFontManager];
                    let base_font: id = msg_send![font_cls, systemFontOfSize: HEX_FONT_SIZE];
                    let family_name: id = msg_send![base_font, familyName];
                    // Weight 15 is the maximum (ultra black), 9 is bold, 12+ is heavy/black
                    let font: id = msg_send![font_manager, fontWithFamily: family_name
                                             traits: 0u64
                                             weight: 15i64
                                             size: HEX_FONT_SIZE];
                    
                    // Calculate contrasting text color (black or white based on luminance)
                    let luminance = 0.299 * r_val + 0.587 * g_val + 0.114 * b_val;
                    let text_color: id = if luminance > 0.5 {
                        msg_send![cls, blackColor]
                    } else {
                        msg_send![cls, whiteColor]
                    };

                    // Create attributes dictionary for the text
                    let dict_cls = class!(NSDictionary);
                    let ns_string_cls = class!(NSString);
                    let font_attr_name: id = msg_send![ns_string_cls, stringWithUTF8String: "NSFont".as_ptr()];
                    let color_attr_name: id = msg_send![ns_string_cls, stringWithUTF8String: "NSForegroundColor".as_ptr()];
                    let keys: Vec<id> = vec![font_attr_name, color_attr_name];
                    let values: Vec<id> = vec![font, text_color];
                    let attributes: id = msg_send![dict_cls, dictionaryWithObjects:values.as_ptr() forKeys:keys.as_ptr() count:2usize];

                    // Draw each character of the hex color along the arc
                    let hex_text = &info.hex_color;
                    let char_count = hex_text.len() as f64;
                    // The border is drawn on mag_rect (radius = mag_size/2) with stroke width border_width.
                    // The stroke extends half inside and half outside, so the center of the border
                    // is exactly at mag_size/2 from the center.
                    let radius = mag_size / 2.0;
                    
                    // Arc spans at top of circle (90 degrees), text readable normally
                    // Tighter angle span for letters closer together
                    let arc_span_degrees: f64 = 70.0;
                    let start_angle: f64 = ((90.0_f64 + arc_span_degrees / 2.0) as f64).to_radians();
                    let end_angle: f64 = ((90.0_f64 - arc_span_degrees / 2.0) as f64).to_radians();
                    let angle_span = end_angle - start_angle;
                    let angle_step = angle_span / (char_count + 1.0);

                    // Save graphics state
                    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];

                    for (i, c) in hex_text.chars().enumerate() {
                        let angle = start_angle + angle_step * (i as f64 + 1.0);
                        
                        // Calculate position on the arc
                        let char_x = center_x + radius * angle.cos();
                        let char_y = center_y + radius * angle.sin();

                        // Create NSString for the character
                        let char_str = c.to_string();
                        let ns_char = NSString::alloc(nil);
                        let ns_char = NSString::init_str(ns_char, &char_str);

                        // Get character size for centering
                        let char_size: cocoa::foundation::NSSize = msg_send![ns_char, sizeWithAttributes: attributes];

                        // Save state for rotation
                        let transform_cls = class!(NSAffineTransform);
                        let transform: id = msg_send![transform_cls, transform];
                        
                        // Translate to character position
                        let _: () = msg_send![transform, translateXBy:char_x yBy:char_y];
                        
                        // Rotate to follow the arc - text readable from outside
                        // At top of circle (90°), we want text upright, so rotate by angle - 90°
                        let rotation_angle = angle - std::f64::consts::PI / 2.0;
                        let _: () = msg_send![transform, rotateByRadians:rotation_angle];
                        
                        // Apply transform
                        let _: () = msg_send![transform, concat];

                        // Draw character centered at origin (which is now at the arc position)
                        let draw_point = NSPoint::new(-char_size.width / 2.0, -char_size.height / 2.0);
                        let _: () = msg_send![ns_char, drawAtPoint:draw_point withAttributes:attributes];

                        // Reset transform for next character
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