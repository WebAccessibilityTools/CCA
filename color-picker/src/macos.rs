//! macOS implementation of the Color Picker
//!
//! This module contains all macOS-specific code using Cocoa and Core Graphics.
//! It creates a fullscreen overlay window that captures the screen and displays
//! a magnified view of the pixels around the cursor.


// =============================================================================
// IMPORTS
// =============================================================================

// Cocoa framework bindings for macOS GUI (legacy, being migrated to objc2)
use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSRect, NSPoint, NSString};
use cocoa::appkit::NSWindowStyleMask;

// Objective-C runtime bindings for low-level messaging (legacy)
use objc::{class, msg_send, sel, sel_impl};
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel};

// objc2 imports for modern Objective-C bindings
use objc2::rc::Retained;
use objc2_foundation::{MainThreadMarker, NSAffineTransform, NSCopying, NSPoint as NSPoint2, NSRect as NSRect2, NSSize as NSSize2};
use objc2_app_kit::{
    NSAffineTransformNSAppKitAdditions,
    NSApplication, 
    NSApplicationActivationOptions,
    NSApplicationActivationPolicy,
    NSBezierPath,
    NSColor,
    NSCursor,
    NSEvent,
    NSEventModifierFlags,
    NSGraphicsContext,
    NSRunningApplication,
    NSView, 
    NSWindow as NSWindow2,
};

// Core Graphics for screen capture and pixel color extraction
use core_graphics::display::CGDisplay;
use core_graphics::image::CGImage;

// Standard library imports
use std::sync::Mutex;

// Import shared configuration constants from config module
use crate::config::*;

// =============================================================================
// GLOBAL STATE
// =============================================================================

/// Global mutex-protected state for mouse position and color information
static MOUSE_STATE: Mutex<Option<MouseColorInfo>> = Mutex::new(None);

/// Global mutex-protected state for current zoom level
static CURRENT_ZOOM: Mutex<f64> = Mutex::new(INITIAL_ZOOM_FACTOR);

/// Stores the final selected color when user clicks or presses Enter
static SELECTED_COLOR: Mutex<Option<(u8, u8, u8)>> = Mutex::new(None);

/// Structure containing all information about current mouse position and color
struct MouseColorInfo {
    x: f64,
    y: f64,
    screen_x: f64,
    screen_y: f64,
    r: u8,
    g: u8,
    b: u8,
    hex_color: String,
    scale_factor: f64,
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
// PUBLIC API
// =============================================================================

/// Runs the color picker application on macOS
pub fn run() -> Option<(u8, u8, u8)> {
    if let Ok(mut color) = SELECTED_COLOR.lock() {
        *color = None;
    }
    
    // Get main thread marker - required for UI operations
    let mtm = MainThreadMarker::new().expect("Must be called from main thread");
    
    // Get the shared application instance
    let app = NSApplication::sharedApplication(mtm);
    
    // Set activation policy to Regular so the app appears in the dock
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    
    // Register our custom view and window classes (still using legacy for class registration)
    let view_class = register_view_class();
    let window_class = register_window_class();
    
    // Get all screens using legacy API (objc2 NSArray has issues)
    unsafe {
        let screens: id = msg_send![class!(NSScreen), screens];
        let count: usize = msg_send![screens, count];
        
        // Create an overlay window for each screen
        for i in 0..count {
            let screen: id = msg_send![screens, objectAtIndex: i];
            let frame: NSRect = msg_send![screen, frame];
            
            let window_alloc: id = msg_send![window_class, alloc];
            let window: id = msg_send![window_alloc,
                initWithContentRect: frame
                styleMask: NSWindowStyleMask::NSBorderlessWindowMask
                backing: 2u64  // NSBackingStoreBuffered = 2
                defer: NO
            ];
            
            // Set window level very high
            let _: () = msg_send![window, setLevel: 1000i64];
            
            // Get clear color using objc2
            let clear_color = NSColor::clearColor();
            let _: () = msg_send![window, setBackgroundColor: &*clear_color];
            
            // Configure window
            let _: () = msg_send![window, setOpaque: NO];
            let _: () = msg_send![window, setHasShadow: NO];
            let _: () = msg_send![window, setIgnoresMouseEvents: NO];
            let _: () = msg_send![window, setAcceptsMouseMovedEvents: YES];
            let _: () = msg_send![window, setSharingType: 0u64]; // NSWindowSharingNone
            
            // Create view
            let view: id = msg_send![view_class, alloc];
            let view: id = msg_send![view, initWithFrame: frame];
            
            // Set view as content and show window
            let _: () = msg_send![window, setContentView: view];
            let _: () = msg_send![window, makeKeyAndOrderFront: nil];
            let _: () = msg_send![window, makeFirstResponder: view];
        }
    }
    
    // Activate the application using objc2
    let current_app = unsafe { NSRunningApplication::currentApplication() };
    unsafe {
        current_app.activateWithOptions(NSApplicationActivationOptions::NSApplicationActivateIgnoringOtherApps);
    }
    
    // Hide cursor using objc2
    unsafe {
        NSCursor::hide();
    }
    
    // Run the application event loop
    unsafe {
        app.run();
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

fn register_view_class() -> &'static objc::runtime::Class {
    let superclass = class!(NSView);
    let mut decl = ClassDecl::new("ColorPickerView", superclass).unwrap();

    unsafe {
        decl.add_method(sel!(acceptsFirstResponder), accepts_first_responder as extern "C" fn(&Object, Sel) -> bool);
        decl.add_method(sel!(mouseDown:), mouse_down as extern "C" fn(&Object, Sel, id));
        decl.add_method(sel!(mouseMoved:), mouse_moved as extern "C" fn(&Object, Sel, id));
        decl.add_method(sel!(scrollWheel:), scroll_wheel as extern "C" fn(&Object, Sel, id));
        decl.add_method(sel!(keyDown:), key_down as extern "C" fn(&Object, Sel, id));
        decl.add_method(sel!(drawRect:), draw_rect as extern "C" fn(&Object, Sel, NSRect));
    }

    decl.register()
}

fn register_window_class() -> &'static objc::runtime::Class {
    let superclass = class!(NSWindow);
    let mut decl = ClassDecl::new("KeyableWindow", superclass).unwrap();

    unsafe {
        decl.add_method(sel!(canBecomeKeyWindow), can_become_key_window as extern "C" fn(&Object, Sel) -> bool);
    }

    decl.register()
}

// =============================================================================
// OBJECTIVE-C METHOD IMPLEMENTATIONS
// =============================================================================

extern "C" fn can_become_key_window(_this: &Object, _cmd: Sel) -> bool { true }

extern "C" fn accepts_first_responder(_this: &Object, _cmd: Sel) -> bool { true }

/// Helper function to stop the application and show cursor
/// This is used by mouse_down, key_down (ESC and Enter)
fn stop_application() {
    unsafe {
        NSCursor::unhide();
    }
    
    // Get the shared application and stop it
    // We need to post a dummy event to break out of the run loop
    unsafe {
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, stop:nil];
        
        // Create and post a dummy event to ensure the run loop exits
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

extern "C" fn mouse_down(_this: &Object, _cmd: Sel, _event: id) {
    // Save the current color as the selected color
    if let Ok(state) = MOUSE_STATE.lock() {
        if let Some(ref info) = *state {
            if let Ok(mut selected) = SELECTED_COLOR.lock() {
                *selected = Some((info.r, info.g, info.b));
            }
        }
    }
    
    stop_application();
}

extern "C" fn mouse_moved(_this: &Object, _cmd: Sel, event: id) {
    // Convert legacy id to objc2 reference
    let event_ref: &NSEvent = unsafe { &*(event as *const NSEvent) };
    
    // Get mouse location in window coordinates using objc2
    let location: NSPoint2 = unsafe { event_ref.locationInWindow() };
    
    // Get the window from the view using objc2
    let view_ref: &NSView = unsafe { &*(_this as *const Object as *const NSView) };
    let window_opt: Option<Retained<NSWindow2>> = view_ref.window();
    
    if let Some(window) = window_opt {
        // Convert window coordinates to screen coordinates
        let screen_location: NSPoint2 = unsafe { window.convertPointToScreen(location) };
        
        // Get the color of the pixel at the cursor position
        if let Some((r, g, b)) = get_pixel_color(screen_location.x, screen_location.y) {
            let r_int = (r * 255.0) as u8;
            let g_int = (g * 255.0) as u8;
            let b_int = (b * 255.0) as u8;

            let hex_color = format!("#{:02X}{:02X}{:02X}", r_int, g_int, b_int);

            if let Ok(mut state) = MOUSE_STATE.lock() {
                // Get scale factor using objc2
                let scale_factor: f64 = if let Some(screen) = window.screen() {
                    screen.backingScaleFactor()
                } else {
                    1.0
                };
                
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

            // Request redraw using legacy API (view is still legacy)
            unsafe {
                let _: () = msg_send![_this, setNeedsDisplay: YES];
            }
        }
    }
}

extern "C" fn scroll_wheel(_this: &Object, _cmd: Sel, event: id) {
    // Convert legacy id to objc2 reference
    let event_ref: &NSEvent = unsafe { &*(event as *const NSEvent) };
    
    // Get the vertical scroll delta using objc2
    let delta_y: f64 = unsafe { event_ref.deltaY() };

    if delta_y != 0.0 {
        if let Ok(mut zoom) = CURRENT_ZOOM.lock() {
            let new_zoom = *zoom + delta_y * ZOOM_STEP;
            *zoom = new_zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        }

        // Request redraw using legacy API
        unsafe {
            let _: () = msg_send![_this, setNeedsDisplay: YES];
        }
    }
}

extern "C" fn key_down(_this: &Object, _cmd: Sel, event: id) {
    // Convert legacy id to objc2 reference
    let event_ref: &NSEvent = unsafe { &*(event as *const NSEvent) };
    
    // Get key code and modifier flags using objc2
    let key_code: u16 = unsafe { event_ref.keyCode() };
    let modifier_flags: NSEventModifierFlags = unsafe { event_ref.modifierFlags() };
    
    // Check if Shift key is pressed
    let shift_pressed = modifier_flags.contains(NSEventModifierFlags::NSEventModifierFlagShift);
    let move_amount = if shift_pressed { SHIFT_MOVE_PIXELS } else { 1.0 };

    // ESC - cancel and exit
    if key_code == 53 {
        stop_application();
        return;
    }
    
    // Enter - select color and exit
    if key_code == 36 {
        if let Ok(state) = MOUSE_STATE.lock() {
            if let Some(ref info) = *state {
                if let Ok(mut selected) = SELECTED_COLOR.lock() {
                    *selected = Some((info.r, info.g, info.b));
                }
            }
        }
        stop_application();
        return;
    }

    // Arrow keys
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
                // Get window and screen info using legacy API (still needed for view)
                unsafe {
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
            }

            // Request redraw
            unsafe {
                let _: () = msg_send![_this, setNeedsDisplay: YES];
            }
        }
    }
}

// =============================================================================
// DRAWING
// =============================================================================

extern "C" fn draw_rect(_this: &Object, _cmd: Sel, _rect: NSRect) {
    // Draw faint overlay
    let overlay_color = unsafe { 
        NSColor::colorWithCalibratedWhite_alpha(0.0, 0.05) 
    };
    unsafe { overlay_color.set() };
    
    let bounds: NSRect = unsafe { msg_send![_this, bounds] };
    unsafe { cocoa::appkit::NSRectFill(bounds) };

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
                
                // Create NSImage from CGImage (still needs legacy for CGImage conversion)
                unsafe {
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

                    // Create circular clip using objc2
                    let mag_rect2 = NSRect2::new(
                        NSPoint2::new(mag_x, mag_y),
                        NSSize2::new(mag_size, mag_size)
                    );
                    let circular_clip = NSBezierPath::bezierPathWithOvalInRect(mag_rect2);

                    // Save graphics state (use legacy - objc2 method is instance method)
                    let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];

                    if let Some(graphics_context) = NSGraphicsContext::currentContext() {
                        graphics_context.setImageInterpolation(objc2_app_kit::NSImageInterpolation::None);
                    }

                    circular_clip.addClip();

                    let from_rect = NSRect::new(
                        NSPoint::new(crop_x, crop_y),
                        cropped_size
                    );

                    let _: () = msg_send![ns_image, drawInRect:mag_rect
                                          fromRect:from_rect
                                          operation:2u64
                                          fraction:1.0];

                    // Restore graphics state
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
                    let square_rect2 = NSRect2::new(
                        NSPoint2::new(reticle_center_x - half_pixel, reticle_center_y - half_pixel),
                        NSSize2::new(pixel_size, pixel_size)
                    );

                    // Gray reticle color using objc2
                    let gray_color = NSColor::colorWithCalibratedRed_green_blue_alpha(0.5, 0.5, 0.5, 1.0);
                    gray_color.setStroke();

                    let reticle_path = NSBezierPath::bezierPathWithRect(square_rect2);
                    reticle_path.setLineWidth(1.0);
                    reticle_path.stroke();

                    // Draw border
                    let hex = &info.hex_color[1..];
                    let r_val = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f64 / 255.0;
                    let g_val = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f64 / 255.0;
                    let b_val = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f64 / 255.0;

                    let border_rect2 = NSRect2::new(
                        NSPoint2::new(mag_x - BORDER_WIDTH / 2.0, mag_y - BORDER_WIDTH / 2.0),
                        NSSize2::new(mag_size + BORDER_WIDTH, mag_size + BORDER_WIDTH)
                    );

                    let border_color = NSColor::colorWithCalibratedRed_green_blue_alpha(r_val, g_val, b_val, 1.0);
                    border_color.setStroke();

                    let border_path = NSBezierPath::bezierPathWithOvalInRect(border_rect2);
                    border_path.setLineWidth(BORDER_WIDTH);
                    border_path.stroke();

                    // Draw hex text - use legacy for font with weight (objc2 doesn't have systemFontOfSize:weight:)
                    let font_cls = class!(NSFont);
                    let font: id = msg_send![font_cls, systemFontOfSize: HEX_FONT_SIZE weight: 0.62f64];

                    let luminance = 0.299 * r_val + 0.587 * g_val + 0.114 * b_val;

                    let text_color = if luminance > 0.5 {
                        NSColor::colorWithCalibratedRed_green_blue_alpha(0.0, 0.0, 0.0, 1.0)
                    } else {
                        NSColor::colorWithCalibratedRed_green_blue_alpha(1.0, 1.0, 1.0, 1.0)
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

                        // Create NSString for the character
                        let char_str = c.to_string();
                        
                        // Create attributes dictionary (still using legacy for complex dict creation)
                        let ns_char_legacy = NSString::alloc(nil);
                        let ns_char_legacy = NSString::init_str(ns_char_legacy, &char_str);

                        let dict_cls = class!(NSDictionary);
                        let font_attr_key = NSString::alloc(nil);
                        let font_attr_key = NSString::init_str(font_attr_key, "NSFont");
                        let color_attr_key = NSString::alloc(nil);
                        let color_attr_key = NSString::init_str(color_attr_key, "NSColor");

                        let text_color_ptr: *const NSColor = &*text_color;
                        let keys: [id; 2] = [font_attr_key, color_attr_key];
                        let values: [id; 2] = [font, text_color_ptr as id];

                        let attributes: id = msg_send![dict_cls, dictionaryWithObjects: values.as_ptr() forKeys: keys.as_ptr() count: 2usize];

                        let char_size: cocoa::foundation::NSSize = msg_send![ns_char_legacy, sizeWithAttributes: attributes];

                        // Use objc2 for NSAffineTransform
                        let transform = NSAffineTransform::transform();
                        transform.translateXBy_yBy(char_x, char_y);

                        let rotation_angle = angle - std::f64::consts::PI / 2.0;
                        transform.rotateByRadians(rotation_angle);

                        transform.concat();

                        let draw_point = NSPoint::new(-char_size.width / 2.0, -char_size.height / 2.0);
                        let _: () = msg_send![ns_char_legacy, drawAtPoint:draw_point withAttributes:attributes];

                        // Invert transform
                        let inverse = transform.copy();
                        inverse.invert();
                        inverse.concat();
                    }

                    let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
                }
            }
        }
    }
}