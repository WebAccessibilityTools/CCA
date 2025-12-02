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

// Objective-C runtime bindings for low-level messaging (legacy - only for msg_send! where needed)
use objc::{class, msg_send, sel, sel_impl};
use objc::runtime::{Object, Sel, BOOL};

// objc2 imports for modern Objective-C bindings
use objc2::{declare_class, mutability, ClassType, DeclaredClass};
use objc2::rc::Retained;
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
use std::sync::Mutex;

// Import shared configuration constants from config module
use crate::config::*;

// Type alias for Objective-C object pointer (replaces deprecated cocoa::base::id)
type Id = *mut Object;

// Objective-C boolean constants (replaces deprecated cocoa::base::YES/NO)
const YES: BOOL = true as BOOL;
const NO: BOOL = false as BOOL;

// =============================================================================
// CUSTOM CLASSES USING OBJC2 DECLARE_CLASS
// =============================================================================

// Instance variables for our custom view (none needed, we use global state)
pub struct ColorPickerViewIvars;

declare_class!(
    pub struct ColorPickerView;

    // SAFETY: ColorPickerView only uses immutable references
    unsafe impl ClassType for ColorPickerView {
        type Super = NSView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "ColorPickerView";
    }

    impl DeclaredClass for ColorPickerView {
        type Ivars = ColorPickerViewIvars;
    }

    unsafe impl ColorPickerView {
        #[method(acceptsFirstResponder)]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[method(mouseDown:)]
        fn mouse_down(&self, _event: &NSEvent) {
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

        #[method(mouseMoved:)]
        fn mouse_moved(&self, event: &NSEvent) {
            // Get mouse location in window coordinates
            let location: NSPoint = unsafe { event.locationInWindow() };
            
            // Get the window from the view
            let window_opt: Option<Retained<NSWindow2>> = self.window();
            
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
                        // Get scale factor
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

                    // Request redraw
                    unsafe { self.setNeedsDisplay(true) };
                }
            }
        }

        #[method(scrollWheel:)]
        fn scroll_wheel(&self, event: &NSEvent) {
            // Get the vertical scroll delta
            let delta_y: f64 = unsafe { event.deltaY() };

            if delta_y != 0.0 {
                if let Ok(mut zoom) = CURRENT_ZOOM.lock() {
                    let new_zoom = *zoom + delta_y * ZOOM_STEP;
                    *zoom = new_zoom.clamp(ZOOM_MIN, ZOOM_MAX);
                }

                // Request redraw
                unsafe { self.setNeedsDisplay(true) };
            }
        }

        #[method(keyDown:)]
        fn key_down(&self, event: &NSEvent) {
            // Get key code and modifier flags
            let key_code: u16 = unsafe { event.keyCode() };
            let modifier_flags: NSEventModifierFlags = unsafe { event.modifierFlags() };
            
            // Check if Shift key is pressed
            let shift_pressed = modifier_flags.contains(NSEventModifierFlags::NSEventModifierFlagShift);
            let move_amount = if shift_pressed { SHIFT_MOVE_PIXELS } else { 1.0 };

            // ESC = 53, Enter/Return = 36
            if key_code == 53 {
                // ESC - cancel
                stop_application();
            } else if key_code == 36 {
                // Enter - confirm selection
                if let Ok(state) = MOUSE_STATE.lock() {
                    if let Some(ref info) = *state {
                        if let Ok(mut selected) = SELECTED_COLOR.lock() {
                            *selected = Some((info.r, info.g, info.b));
                        }
                    }
                }
                stop_application();
            } else {
                // Arrow keys: left=123, right=124, down=125, up=126
                let (dx, dy): (f64, f64) = match key_code {
                    123 => (-move_amount, 0.0),  // Left
                    124 => (move_amount, 0.0),   // Right
                    125 => (0.0, -move_amount),  // Down
                    126 => (0.0, move_amount),   // Up
                    _ => (0.0, 0.0),
                };

                if dx != 0.0 || dy != 0.0 {
                    // Move cursor and update state
                    if let Ok(state) = MOUSE_STATE.lock() {
                        if let Some(ref info) = *state {
                            let new_x = info.screen_x + dx;
                            let new_y = info.screen_y + dy;
                            
                            // Get screen height for coordinate conversion
                            let main_display = CGDisplay::main();
                            let screen_height = main_display.pixels_high() as f64;
                            
                            // Convert Cocoa coordinates to Core Graphics coordinates
                            let cg_y = screen_height - new_y;
                            
                            // Move the cursor
                            let _ = CGDisplay::warp_mouse_cursor_position(core_graphics::geometry::CGPoint::new(new_x, cg_y));
                            
                            drop(state);
                            
                            // Get new color after moving cursor
                            if let Some((r, g, b)) = get_pixel_color(new_x, new_y) {
                                let r_int = (r * 255.0) as u8;
                                let g_int = (g * 255.0) as u8;
                                let b_int = (b * 255.0) as u8;

                                let hex_color = format!("#{:02X}{:02X}{:02X}", r_int, g_int, b_int);

                                if let Ok(mut state) = MOUSE_STATE.lock() {
                                    if let Some(window) = self.window() {
                                        let screen_point = NSPoint::new(new_x, new_y);
                                        let window_point: NSPoint = window.convertPointFromScreen(screen_point);

                                        let scale_factor: f64 = if let Some(screen) = window.screen() {
                                            screen.backingScaleFactor()
                                        } else {
                                            1.0
                                        };

                                        *state = Some(MouseColorInfo {
                                            x: window_point.x,
                                            y: window_point.y,
                                            screen_x: new_x,
                                            screen_y: new_y,
                                            r: r_int,
                                            g: g_int,
                                            b: b_int,
                                            hex_color,
                                            scale_factor,
                                        });
                                    }
                                }

                                // Request redraw
                                unsafe { self.setNeedsDisplay(true) };
                            }
                        }
                    }
                }
            }
        }

        #[method(drawRect:)]
        fn draw_rect(&self, _rect: NSRect) {
            draw_view(self);
        }
    }
);

// Instance variables for our custom window (none needed)
pub struct KeyableWindowIvars;

declare_class!(
    pub struct KeyableWindow;

    // SAFETY: KeyableWindow only uses immutable references
    unsafe impl ClassType for KeyableWindow {
        type Super = NSWindow2;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "KeyableWindow";
    }

    impl DeclaredClass for KeyableWindow {
        type Ivars = KeyableWindowIvars;
    }

    unsafe impl KeyableWindow {
        #[method(canBecomeKeyWindow)]
        fn can_become_key_window(&self) -> bool {
            true
        }
    }
);

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

/// Helper function to stop the application and show cursor
fn stop_application() {
    unsafe {
        NSCursor::unhide();
    }
    
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        app.stop(None);
        
        // Create a dummy event to ensure the run loop exits
        unsafe {
            use objc2_app_kit::NSEventType;
            
            let dummy_event = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
                NSEventType::ApplicationDefined,
                NSPoint::new(0.0, 0.0),
                NSEventModifierFlags::empty(),
                0.0,
                0,
                None,
                0,
                0,
                0
            );
            
            if let Some(event) = dummy_event {
                app.postEvent_atStart(&event, true);
            }
        }
    }
}

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
    
    // Get all screens and create windows
    unsafe {
        let screens: Id = msg_send![class!(NSScreen), screens];
        let count: usize = msg_send![screens, count];
        
        for i in 0..count {
            let screen: Id = msg_send![screens, objectAtIndex: i];
            let frame: NSRect = msg_send![screen, frame];
            
            // Create window using our KeyableWindow class via msg_send
            // Convert objc2 class to raw pointer for legacy msg_send!
            let window_cls = KeyableWindow::class() as *const objc2::runtime::AnyClass as *const Object;
            let window_alloc: Id = msg_send![window_cls, alloc];
            let window: Id = msg_send![window_alloc,
                initWithContentRect: frame
                styleMask: NSWindowStyleMask::Borderless
                backing: 2u64
                defer: NO
            ];
            
            // Convert to objc2 reference for method calls
            let window_ref: &KeyableWindow = &*(window as *const KeyableWindow);
            
            window_ref.setLevel(1000);
            
            let clear_color = NSColor::clearColor();
            window_ref.setBackgroundColor(Some(&clear_color));
            
            window_ref.setOpaque(false);
            window_ref.setHasShadow(false);
            window_ref.setIgnoresMouseEvents(false);
            window_ref.setAcceptsMouseMovedEvents(true);
            let _: () = msg_send![window, setSharingType: 0u64];
            
            // Create view using our ColorPickerView class via msg_send
            let view_cls = ColorPickerView::class() as *const objc2::runtime::AnyClass as *const Object;
            let view_alloc: Id = msg_send![view_cls, alloc];
            let view: Id = msg_send![view_alloc, initWithFrame: frame];
            
            let view_ref: &ColorPickerView = &*(view as *const ColorPickerView);
            
            window_ref.setContentView(Some(view_ref));
            window_ref.makeKeyAndOrderFront(None);
            window_ref.makeFirstResponder(Some(view_ref));
        }
    }
    
    // Activate the application
    unsafe {
        let running_app: Id = msg_send![class!(NSRunningApplication), currentApplication];
        let _: () = msg_send![running_app, activateWithOptions: 0u64];
    }
    
    // Hide cursor
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
// DRAWING
// =============================================================================

/// Main drawing function called from ColorPickerView's drawRect method
fn draw_view(view: &NSView) {
    // Draw faint overlay
    let overlay_color = unsafe { NSColor::colorWithCalibratedWhite_alpha(0.0, 0.05) };
    unsafe { overlay_color.set() };
    
    // Get bounds and fill with overlay color
    let bounds: NSRect = view.bounds();
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

                    let circular_clip = NSBezierPath::bezierPathWithOvalInRect(mag_rect);

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
                                          fraction:1.0f64];

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
                    let square_rect = NSRect::new(
                        NSPoint::new(reticle_center_x - half_pixel, reticle_center_y - half_pixel),
                        NSSize::new(pixel_size, pixel_size)
                    );

                    let gray_color = NSColor::colorWithCalibratedRed_green_blue_alpha(0.5, 0.5, 0.5, 1.0);
                    gray_color.setStroke();

                    let reticle_path = NSBezierPath::bezierPathWithRect(square_rect);
                    reticle_path.setLineWidth(1.0);
                    reticle_path.stroke();

                    // Draw border
                    let hex = &info.hex_color[1..];
                    let r_val = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f64 / 255.0;
                    let g_val = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f64 / 255.0;
                    let b_val = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f64 / 255.0;

                    let border_rect = NSRect::new(
                        NSPoint::new(mag_x - BORDER_WIDTH / 2.0, mag_y - BORDER_WIDTH / 2.0),
                        NSSize::new(mag_size + BORDER_WIDTH, mag_size + BORDER_WIDTH)
                    );

                    let border_color = NSColor::colorWithCalibratedRed_green_blue_alpha(r_val, g_val, b_val, 1.0);
                    border_color.setStroke();

                    let border_path = NSBezierPath::bezierPathWithOvalInRect(border_rect);
                    border_path.setLineWidth(BORDER_WIDTH);
                    border_path.stroke();

                    // Draw hex text
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

                        let char_str = c.to_string();
                        let ns_char = NSString::from_str(&char_str);
                        
                        use objc2_foundation::NSDictionary;
                        use objc2::runtime::AnyObject;
                        
                        let font_attr_key = NSString::from_str("NSFont");
                        let color_attr_key = NSString::from_str("NSColor");
                        
                        let font_retained: Retained<AnyObject> = 
                            Retained::retain(font as *mut AnyObject).unwrap();
                        let color_retained: Retained<AnyObject> = 
                            Retained::cast(text_color.clone());
                        
                        let keys: &[&NSString] = &[&font_attr_key, &color_attr_key];
                        let values: Vec<Retained<AnyObject>> = vec![font_retained, color_retained];
                        let attributes = NSDictionary::from_vec(keys, values);

                        let char_size: NSSize = ns_char.sizeWithAttributes(Some(&attributes));

                        let transform = NSAffineTransform::transform();
                        transform.translateXBy_yBy(char_x, char_y);

                        let rotation_angle = angle - std::f64::consts::PI / 2.0;
                        transform.rotateByRadians(rotation_angle);

                        transform.concat();

                        let draw_point = NSPoint::new(-char_size.width, -char_size.height);
                        ns_char.drawAtPoint_withAttributes(draw_point, Some(&attributes));

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