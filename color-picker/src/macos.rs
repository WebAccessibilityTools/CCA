//! macOS implementation of the Color Picker
//!
//! This module contains all macOS-specific code using Cocoa and Core Graphics.
//! It creates a fullscreen overlay window that captures the screen and displays
//! a magnified view of the pixels around the cursor.

// Suppress deprecation warnings for legacy cocoa/objc crate usage during migration
#![allow(deprecated)]

// =============================================================================
// IMPORTS
// =============================================================================

// Objective-C runtime bindings for low-level messaging (legacy)
use objc::{class, msg_send, sel, sel_impl};
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel, BOOL};

// objc2 imports for modern Objective-C bindings
use objc2::rc::Retained;
use objc2::ClassType;
use objc2_foundation::{MainThreadMarker, NSAffineTransform, NSCopying, NSPoint, NSRect, NSSize, NSString};
use objc2_app_kit::{
    NSAffineTransformNSAppKitAdditions,
    NSApplication, 
    NSApplicationActivationPolicy,
    NSBezierPath,
    NSColor,
    NSCursor,
    NSEvent,
    NSEventModifierFlags,
    NSGraphicsContext,
    NSStringDrawing,
    NSView, 
    NSWindow as NSWindow2,
    NSWindowStyleMask,
};

// Core Graphics for screen capture and pixel color extraction
use core_graphics::display::CGDisplay;
use core_graphics::image::CGImage;

// Standard library imports
use std::ptr::null_mut;
use std::sync::Mutex;

// Import shared configuration constants from config module
use crate::config::*;

// Type alias for Objective-C object pointer (replaces deprecated cocoa::base::id)
type Id = *mut Object;

// Objective-C boolean constants (replaces deprecated cocoa::base::YES/NO)
const YES: BOOL = true as BOOL;
const NO: BOOL = false as BOOL;

// NSRect compatible with objc::Encode for use with add_method
// This is needed because objc2's NSRect doesn't implement objc::Encode
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct NSRectEncode {
    pub origin: NSPointEncode,
    pub size: NSSizeEncode,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct NSPointEncode {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct NSSizeEncode {
    pub width: f64,
    pub height: f64,
}

unsafe impl objc::Encode for NSRectEncode {
    fn encode() -> objc::Encoding {
        let encoding = format!(
            "{{CGRect={}{}}}",
            NSPointEncode::encode().as_str(),
            NSSizeEncode::encode().as_str()
        );
        unsafe { objc::Encoding::from_str(&encoding) }
    }
}

unsafe impl objc::Encode for NSPointEncode {
    fn encode() -> objc::Encoding {
        unsafe { objc::Encoding::from_str("{CGPoint=dd}") }
    }
}

unsafe impl objc::Encode for NSSizeEncode {
    fn encode() -> objc::Encoding {
        unsafe { objc::Encoding::from_str("{CGSize=dd}") }
    }
}

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
        let screens: Id = msg_send![class!(NSScreen), screens];
        let count: usize = msg_send![screens, count];
        
        // Create an overlay window for each screen
        for i in 0..count {
            let screen: Id = msg_send![screens, objectAtIndex: i];
            let frame: NSRect = msg_send![screen, frame];
            
            let window_alloc: Id = msg_send![window_class, alloc];
            let window: Id = msg_send![window_alloc,
                initWithContentRect: frame
                styleMask: NSWindowStyleMask::Borderless
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
            let view: Id = msg_send![view_class, alloc];
            let view: Id = msg_send![view, initWithFrame: frame];
            
            // Set view as content and show window
            let _: () = msg_send![window, setContentView: view];
            let _: () = msg_send![window, makeKeyAndOrderFront: null_mut::<Object>()];
            let _: () = msg_send![window, makeFirstResponder: view];
        }
    }
    
    // Activate the application - use legacy msg_send to avoid deprecated warning
    unsafe {
        let running_app: Id = msg_send![class!(NSRunningApplication), currentApplication];
        let _: () = msg_send![running_app, activateWithOptions: 0u64];
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
    // Get NSView class via objc2 and convert to objc runtime class
    let superclass_ptr = NSView::class() as *const objc2::runtime::AnyClass as *const objc::runtime::Class;
    let superclass = unsafe { &*superclass_ptr };
    let mut decl = ClassDecl::new("ColorPickerView", superclass).unwrap();

    unsafe {
        decl.add_method(sel!(acceptsFirstResponder), accepts_first_responder as extern "C" fn(&Object, Sel) -> bool);
        decl.add_method(sel!(mouseDown:), mouse_down as extern "C" fn(&Object, Sel, Id));
        decl.add_method(sel!(mouseMoved:), mouse_moved as extern "C" fn(&Object, Sel, Id));
        decl.add_method(sel!(scrollWheel:), scroll_wheel as extern "C" fn(&Object, Sel, Id));
        decl.add_method(sel!(keyDown:), key_down as extern "C" fn(&Object, Sel, Id));
        decl.add_method(sel!(drawRect:), draw_rect as extern "C" fn(&Object, Sel, NSRectEncode));
    }

    decl.register()
}

fn register_window_class() -> &'static objc::runtime::Class {
    // Get NSWindow class via objc2 and convert to objc runtime class
    let superclass_ptr = NSWindow2::class() as *const objc2::runtime::AnyClass as *const objc::runtime::Class;
    let superclass = unsafe { &*superclass_ptr };
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
    // We need MainThreadMarker - this function is always called from the main thread
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        unsafe {
            app.stop(None);
        }
        
        // Create and post a dummy event to ensure the run loop exits
        // Convert app to raw pointer for legacy msg_send!
        let app_ptr: Id = &*app as *const NSApplication as Id;
        unsafe {
            let dummy_event: Id = msg_send![class!(NSEvent), 
                otherEventWithType:15u64  // NSEventTypeApplicationDefined
                location:NSPoint::new(0.0, 0.0)
                modifierFlags:0u64
                timestamp:0.0f64
                windowNumber:0i64
                context:null_mut::<Object>()
                subtype:0i16
                data1:0i64
                data2:0i64
            ];
            let _: () = msg_send![app_ptr, postEvent:dummy_event atStart:YES];
        }
    }
}

extern "C" fn mouse_down(_this: &Object, _cmd: Sel, _event: Id) {
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

extern "C" fn mouse_moved(_this: &Object, _cmd: Sel, event: Id) {
    // Convert legacy id to objc2 reference
    let event_ref: &NSEvent = unsafe { &*(event as *const NSEvent) };
    
    // Get mouse location in window coordinates using objc2
    let location: NSPoint = unsafe { event_ref.locationInWindow() };
    
    // Get the window from the view using objc2
    let view_ref: &NSView = unsafe { &*(_this as *const Object as *const NSView) };
    let window_opt: Option<Retained<NSWindow2>> = view_ref.window();
    
    if let Some(window) = window_opt {
        // Convert window coordinates to screen coordinates
        let screen_location: NSPoint = unsafe { window.convertPointToScreen(location) };
        
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

            // Request redraw using objc2
            let view_ref: &NSView = unsafe { &*(_this as *const Object as *const NSView) };
            unsafe { view_ref.setNeedsDisplay(true) };
        }
    }
}

extern "C" fn scroll_wheel(_this: &Object, _cmd: Sel, event: Id) {
    // Convert legacy id to objc2 reference
    let event_ref: &NSEvent = unsafe { &*(event as *const NSEvent) };
    
    // Get the vertical scroll delta using objc2
    let delta_y: f64 = unsafe { event_ref.deltaY() };

    if delta_y != 0.0 {
        if let Ok(mut zoom) = CURRENT_ZOOM.lock() {
            let new_zoom = *zoom + delta_y * ZOOM_STEP;
            *zoom = new_zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        }

        // Request redraw using objc2
        let view_ref: &NSView = unsafe { &*(_this as *const Object as *const NSView) };
        unsafe { view_ref.setNeedsDisplay(true) };
    }
}

extern "C" fn key_down(_this: &Object, _cmd: Sel, event: Id) {
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
                // Get window and screen info using objc2
                let view_ref: &NSView = unsafe { &*(_this as *const Object as *const NSView) };
                if let Some(window) = view_ref.window() {
                    let screen_point = NSPoint::new(new_x, cocoa_y);
                    let window_point: NSPoint = unsafe { window.convertPointFromScreen(screen_point) };

                    let scale_factor: f64 = if let Some(screen) = window.screen() {
                        screen.backingScaleFactor()
                    } else {
                        1.0
                    };

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

            // Request redraw using objc2
            let view_ref: &NSView = unsafe { &*(_this as *const Object as *const NSView) };
            unsafe { view_ref.setNeedsDisplay(true) };
        }
    }
}

// =============================================================================
// DRAWING
// =============================================================================

extern "C" fn draw_rect(_this: &Object, _cmd: Sel, _rect: NSRectEncode) {
    // Draw faint overlay
    let overlay_color = unsafe { 
        NSColor::colorWithCalibratedWhite_alpha(0.0, 0.05) 
    };
    unsafe { overlay_color.set() };
    
    // Get bounds using objc2 and fill with overlay color
    let view_ref: &NSView = unsafe { &*(_this as *const Object as *const NSView) };
    let bounds: NSRect = view_ref.bounds();
    let bounds_path = unsafe { NSBezierPath::bezierPathWithRect(bounds) };
    unsafe { bounds_path.fill() };

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
                    let ns_image: Id = msg_send![ns_image_cls, alloc];

                    let cg_image_ptr = {
                        let ptr_addr = &cg_image as *const CGImage as *const *const core_graphics::sys::CGImage;
                        *ptr_addr
                    };

                    let full_size = NSSize::new(img_width, img_height);
                    let ns_image: Id = msg_send![ns_image, initWithCGImage:cg_image_ptr size:full_size];
                    let cropped_size = NSSize::new(use_width, use_height);

                    let mag_x = info.x - mag_size / 2.0;
                    let mag_y = info.y - mag_size / 2.0;

                    let mag_rect = NSRect::new(
                        NSPoint::new(mag_x, mag_y),
                        NSSize::new(mag_size, mag_size)
                    );

                    // Create circular clip using objc2
                    let mag_rect2 = NSRect::new(
                        NSPoint::new(mag_x, mag_y),
                        NSSize::new(mag_size, mag_size)
                    );
                    let circular_clip = NSBezierPath::bezierPathWithOvalInRect(mag_rect2);

                    // Save graphics state using objc2
                    NSGraphicsContext::saveGraphicsState_class();

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

                    // Restore graphics state using objc2
                    NSGraphicsContext::restoreGraphicsState_class();

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
                    let square_rect2 = NSRect::new(
                        NSPoint::new(reticle_center_x - half_pixel, reticle_center_y - half_pixel),
                        NSSize::new(pixel_size, pixel_size)
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

                    let border_rect2 = NSRect::new(
                        NSPoint::new(mag_x - BORDER_WIDTH / 2.0, mag_y - BORDER_WIDTH / 2.0),
                        NSSize::new(mag_size + BORDER_WIDTH, mag_size + BORDER_WIDTH)
                    );

                    let border_color = NSColor::colorWithCalibratedRed_green_blue_alpha(r_val, g_val, b_val, 1.0);
                    border_color.setStroke();

                    let border_path = NSBezierPath::bezierPathWithOvalInRect(border_rect2);
                    border_path.setLineWidth(BORDER_WIDTH);
                    border_path.stroke();

                    // Draw hex text - use legacy for font with weight (objc2 doesn't have systemFontOfSize:weight:)
                    let font_cls = class!(NSFont);
                    let font: Id = msg_send![font_cls, systemFontOfSize: HEX_FONT_SIZE weight: 0.62f64];

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

                    NSGraphicsContext::saveGraphicsState_class();

                    for (i, c) in hex_text.chars().enumerate() {
                        let angle = start_angle - angle_step * (i as f64);

                        let char_x = center_x + radius * angle.cos();
                        let char_y = center_y + radius * angle.sin();

                        // Create NSString for the character using objc2
                        let char_str = c.to_string();
                        let ns_char = NSString::from_str(&char_str);
                        
                        // Create attribute keys using objc2
                        let font_attr_key = NSString::from_str("NSFont");
                        let color_attr_key = NSString::from_str("NSColor");

                        // Create attributes dictionary using legacy msg_send (objc2 NSDictionary::from_vec requires Retained values)
                        let dict_cls = class!(NSDictionary);
                        let font_key_ptr: Id = &*font_attr_key as *const NSString as Id;
                        let color_key_ptr: Id = &*color_attr_key as *const NSString as Id;
                        let text_color_ptr: Id = &*text_color as *const NSColor as Id;
                        
                        let keys: [Id; 2] = [font_key_ptr, color_key_ptr];
                        let values: [Id; 2] = [font, text_color_ptr];

                        let attributes: Id = msg_send![dict_cls, dictionaryWithObjects: values.as_ptr() forKeys: keys.as_ptr() count: 2usize];
                        
                        // Convert to objc2 NSDictionary reference for use with NSStringDrawing
                        use objc2_foundation::NSDictionary;
                        use objc2::runtime::AnyObject;
                        let attrs_ref: &NSDictionary<NSString, AnyObject> = unsafe { &*(attributes as *const NSDictionary<_, _>) };

                        // Get character size using objc2 NSStringDrawing
                        let char_size: NSSize = unsafe { ns_char.sizeWithAttributes(Some(attrs_ref)) };

                        // Use objc2 for NSAffineTransform
                        let transform = NSAffineTransform::transform();
                        transform.translateXBy_yBy(char_x, char_y);

                        let rotation_angle = angle - std::f64::consts::PI / 2.0;
                        transform.rotateByRadians(rotation_angle);

                        transform.concat();

                        // Draw the character using objc2 NSStringDrawing trait
                        let draw_point = NSPoint::new(-char_size.width, -char_size.height);
                        unsafe { ns_char.drawAtPoint_withAttributes(draw_point, Some(attrs_ref)) };

                        // Invert transform
                        let inverse = transform.copy();
                        inverse.invert();
                        inverse.concat();
                    }

                    NSGraphicsContext::restoreGraphicsState_class();
                }
            }
        }
    }
}