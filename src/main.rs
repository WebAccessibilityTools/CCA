use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSAutoreleasePool, NSRect, NSArray};
use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationPolicyRegular, NSBackingStoreBuffered,
    NSColor, NSWindow, NSWindowStyleMask, NSEvent,
    NSRunningApplication, NSApplicationActivateIgnoringOtherApps, NSScreen, NSView
};
use objc::{class, msg_send, sel, sel_impl};
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel};

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

            // Create and set the custom view
            let view: id = msg_send![view_class, alloc];
            let view: id = msg_send![view, initWithFrame:frame];
            window.setContentView_(view);

            // Make window key and visible
            window.makeKeyAndOrderFront_(nil);
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
        let color: id = msg_send![cls, colorWithCalibratedWhite:0.0 alpha:0.5];

        let _: () = msg_send![color, set];
        let bounds: NSRect = msg_send![_this, bounds];
        cocoa::appkit::NSRectFill(bounds);
    }
}

