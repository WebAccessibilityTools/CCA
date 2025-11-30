//! macOS implementation of the Color Picker
//!
//! This module contains all macOS-specific code using Cocoa and Core Graphics.

// Cocoa framework bindings for macOS GUI
use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{
    NSAutoreleasePool,
    NSRect,
    NSArray,
    NSPoint,
    NSString,
};
use cocoa::appkit::{
    NSApp,
    NSApplication,
    NSApplicationActivationPolicyRegular,
    NSBackingStoreBuffered,
    NSWindow,
    NSWindowStyleMask,
    NSRunningApplication,
    NSApplicationActivateIgnoringOtherApps,
    NSScreen,
};

// Objective-C runtime bindings
use objc::{class, msg_send, sel, sel_impl};
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel};

// Core Graphics for screen capture and color picking
use core_graphics::display::CGDisplay;
use core_graphics::image::CGImage;

// Standard library
use std::sync::Mutex;

// Import shared configuration
use crate::config::*;

// =============================================================================
// GLOBAL STATE
// =============================================================================

/// Global mutex-protected state for mouse position and color
static MOUSE_STATE: Mutex<Option<MouseColorInfo>> = Mutex::new(None);

/// Global mutex-protected state for current zoom level
static CURRENT_ZOOM: Mutex<f64> = Mutex::new(INITIAL_ZOOM_FACTOR);

/// Information about the current mouse position and color
struct MouseColorInfo {
    x: f64,
    y: f64,
    screen_x: f64,
    screen_y: f64,
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

    let half_size = size / 2.0;
    let rect = CGRect::new(
        &CGPointStruct::new(x - half_size, cg_y - half_size),
        &CGSize::new(size, size)
    );

    main_display.image_for_rect(rect)
}

/// Captures the color of a single pixel at the given screen coordinates
fn get_pixel_color(x: f64, y: f64) -> Option<(f64, f64, f64)> {
    use core_graphics::geometry::{CGRect, CGPoint as CGPointStruct, CGSize};

    let main_display = CGDisplay::main();
    let screen_height = main_display.pixels_high() as f64;
    let cg_y = screen_height - y;

    let rect = CGRect::new(
        &CGPointStruct::new(x, cg_y),
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
pub fn run() {
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicyRegular);

        let view_class = register_view_class();
        let window_class = register_window_class();

        let screens = NSScreen::screens(nil);
        let count = screens.count();

        for i in 0..count {
            let screen = screens.objectAtIndex(i);
            let frame: NSRect = msg_send![screen, frame];

            let window_alloc: id = msg_send![window_class, alloc];
            let window = window_alloc.initWithContentRect_styleMask_backing_defer_(
                frame,
                NSWindowStyleMask::NSBorderlessWindowMask,
                NSBackingStoreBuffered,
                NO
            );

            window.setLevel_(1000);

            let cls_color = class!(NSColor);
            let clear_color: id = msg_send![cls_color, clearColor];

            window.setBackgroundColor_(clear_color);
            window.setOpaque_(NO);
            window.setHasShadow_(NO);
            window.setIgnoresMouseEvents_(NO);
            window.setAcceptsMouseMovedEvents_(YES);

            let _: () = msg_send![window, setSharingType: 0u64];

            let view: id = msg_send![view_class, alloc];
            let view: id = msg_send![view, initWithFrame:frame];

            window.setContentView_(view);
            window.makeKeyAndOrderFront_(nil);

            let _: () = msg_send![window, makeFirstResponder: view];
        }

        let current_app = NSRunningApplication::currentApplication(nil);
        current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);

        let _: () = msg_send![class!(NSCursor), hide];

        app.run();
    }
}

// =============================================================================
// CUSTOM CLASS REGISTRATION
// =============================================================================

fn register_view_class() -> &'static objc::runtime::Class {
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

fn register_window_class() -> &'static objc::runtime::Class {
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
    YES
}

extern "C" fn accepts_first_responder(_this: &Object, _cmd: Sel) -> bool {
    YES
}

extern "C" fn mouse_down(_this: &Object, _cmd: Sel, _event: id) {
    unsafe {
        let _: () = msg_send![class!(NSCursor), unhide];
        let app = NSApp();
        let _: () = msg_send![app, terminate:nil];
    }
}

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
                    hex_color: hex_color.clone(),
                    scale_factor,
                });
            }

            print!("\r\x1B[K");
            print!("RGB: ({:3}, {:3}, {:3})  |  HEX: {}  ", r_int, g_int, b_int, hex_color);

            use std::io::{self, Write};
            io::stdout().flush().unwrap();

            let _: () = msg_send![_this, setNeedsDisplay: YES];
        }
    }
}

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

extern "C" fn key_down(_this: &Object, _cmd: Sel, event: id) {
    unsafe {
        let key_code: u16 = msg_send![event, keyCode];
        let modifier_flags: u64 = msg_send![event, modifierFlags];

        let shift_pressed = (modifier_flags & (1 << 17)) != 0;
        let move_amount = if shift_pressed { SHIFT_MOVE_PIXELS } else { 1.0 };

        if key_code == 53 {
            let _: () = msg_send![class!(NSCursor), unhide];
            let app = NSApp();
            let _: () = msg_send![app, terminate:nil];
            return;
        }

        let (dx, dy): (f64, f64) = match key_code {
            123 => (-move_amount, 0.0),
            124 => (move_amount, 0.0),
            125 => (0.0, -move_amount),
            126 => (0.0, move_amount),
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
                        hex_color: hex_color.clone(),
                        scale_factor,
                    });
                }

                print!("\r\x1B[K");
                print!("RGB: ({:3}, {:3}, {:3})  |  HEX: {}  ", r_int, g_int, b_int, hex_color);
                use std::io::{self, Write};
                io::stdout().flush().unwrap();

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

        // Draw magnifier
        if let Ok(state) = MOUSE_STATE.lock() {
            if let Some(ref info) = *state {
                let current_zoom = match CURRENT_ZOOM.lock() {
                    Ok(z) => *z,
                    Err(_) => INITIAL_ZOOM_FACTOR,
                };

                let mag_size = CAPTURED_PIXELS * current_zoom;
                
                // On Retina, capture fewer points to get the same number of physical pixels
                let extra_margin = if info.scale_factor > 1.0 { 1.0 } else { 0.0 };
                let capture_points = (CAPTURED_PIXELS + extra_margin) / info.scale_factor;

                if let Some(cg_image) = capture_zoom_area(info.screen_x, info.screen_y, capture_points) {
                    let ns_image_cls = class!(NSImage);
                    let ns_image: id = msg_send![ns_image_cls, alloc];

                    let cg_image_ptr = {
                        let ptr_addr = &cg_image as *const CGImage as *const *const core_graphics::sys::CGImage;
                        *ptr_addr
                    };

                    let size = cocoa::foundation::NSSize::new(
                        cg_image.width() as f64,
                        cg_image.height() as f64
                    );

                    let ns_image: id = msg_send![ns_image, initWithCGImage:cg_image_ptr size:size];

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

                    // On Retina, offset the source rectangle by 0.5 pixel to align center
                    let source_offset = if info.scale_factor > 1.0 { 0.5 } else { 0.0 };
                    let from_rect = NSRect::new(
                        NSPoint::new(source_offset, source_offset),
                        cocoa::foundation::NSSize::new(size.width - source_offset, size.height - source_offset)
                    );

                    let _: () = msg_send![ns_image, drawInRect:mag_rect
                                          fromRect:from_rect
                                          operation:2u64
                                          fraction:1.0];

                    let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];

                    // Draw reticle
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
