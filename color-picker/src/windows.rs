// =============================================================================
// COLOR PICKER - VERSION WINDOWS (OPTIMISÉE)
// =============================================================================

use crate::config::{
    BORDER_WIDTH, HEX_FONT_SIZE, CAPTURED_PIXELS, INITIAL_ZOOM_FACTOR,
    SHIFT_MOVE_PIXELS, ZOOM_MIN, ZOOM_MAX, ZOOM_STEP, CHAR_SPACING_PIXELS,
};

use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Input::KeyboardAndMouse::*,
            WindowsAndMessaging::*,
        },
    },
};

use std::sync::Mutex;

// =============================================================================
// CONSTANTES
// =============================================================================

const CAPTURED_PIXELS_MIN: f64 = 9.0;
const CAPTURED_PIXELS_MAX: f64 = 21.0;
const CAPTURED_PIXELS_STEP: f64 = 2.0;
const WINDOW_CLASS_NAME: &str = "ColorPickerClass";
const TIMER_ID: usize = 1;
const TIMER_INTERVAL: u32 = 16; // ~60 FPS

// =============================================================================
// STRUCTURES
// =============================================================================

#[derive(Clone, Debug)]
pub struct ColorPickerResult {
    pub foreground: Option<(u8, u8, u8)>,
    pub background: Option<(u8, u8, u8)>,
}

// =============================================================================
// ÉTAT GLOBAL
// =============================================================================

static CURSOR_X: Mutex<i32> = Mutex::new(0);
static CURSOR_Y: Mutex<i32> = Mutex::new(0);
static COLOR_R: Mutex<u8> = Mutex::new(0);
static COLOR_G: Mutex<u8> = Mutex::new(0);
static COLOR_B: Mutex<u8> = Mutex::new(0);
static FG_COLOR: Mutex<Option<(u8, u8, u8)>> = Mutex::new(None);
static BG_COLOR: Mutex<Option<(u8, u8, u8)>> = Mutex::new(None);
static FG_MODE: Mutex<bool> = Mutex::new(true);
static CONTINUE_MODE: Mutex<bool> = Mutex::new(false);
static CURRENT_ZOOM: Mutex<f64> = Mutex::new(INITIAL_ZOOM_FACTOR);
static CURRENT_CAPTURED: Mutex<f64> = Mutex::new(CAPTURED_PIXELS);
static SHOULD_QUIT: Mutex<bool> = Mutex::new(false);
static SCREEN_CAPTURE: Mutex<Option<ScreenCapture>> = Mutex::new(None);

// Structure pour stocker la capture d'écran
struct ScreenCapture {
    hdc_mem: HDC,
    hbitmap: HBITMAP,
    width: i32,
    height: i32,
}

// Implémentation de Send pour ScreenCapture (les handles GDI sont thread-safe pour notre usage)
unsafe impl Send for ScreenCapture {}

// =============================================================================
// FONCTIONS UTILITAIRES
// =============================================================================

fn reset_state() {
    *CURSOR_X.lock().unwrap() = 0;
    *CURSOR_Y.lock().unwrap() = 0;
    *COLOR_R.lock().unwrap() = 0;
    *COLOR_G.lock().unwrap() = 0;
    *COLOR_B.lock().unwrap() = 0;
    *FG_COLOR.lock().unwrap() = None;
    *BG_COLOR.lock().unwrap() = None;
    *FG_MODE.lock().unwrap() = true;
    *CONTINUE_MODE.lock().unwrap() = false;
    *CURRENT_ZOOM.lock().unwrap() = INITIAL_ZOOM_FACTOR;
    *CURRENT_CAPTURED.lock().unwrap() = CAPTURED_PIXELS;
    *SHOULD_QUIT.lock().unwrap() = false;
}

fn capture_full_screen() {
    unsafe {
        let hdc_screen = GetDC(HWND::default());
        let width = GetSystemMetrics(SM_CXSCREEN);
        let height = GetSystemMetrics(SM_CYSCREEN);
        
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
        SelectObject(hdc_mem, hbitmap);
        
        // Capture tout l'écran d'un coup
        BitBlt(hdc_mem, 0, 0, width, height, hdc_screen, 0, 0, SRCCOPY);
        
        ReleaseDC(HWND::default(), hdc_screen);
        
        if let Ok(mut cap) = SCREEN_CAPTURE.lock() {
            // Libère l'ancienne capture si elle existe
            if let Some(old) = cap.take() {
                DeleteObject(old.hbitmap);
                DeleteDC(old.hdc_mem);
            }
            *cap = Some(ScreenCapture { hdc_mem, hbitmap, width, height });
        }
    }
}

fn get_pixel_from_capture(x: i32, y: i32) -> (u8, u8, u8) {
    if let Ok(cap) = SCREEN_CAPTURE.lock() {
        if let Some(ref capture) = *cap {
            if x >= 0 && x < capture.width && y >= 0 && y < capture.height {
                unsafe {
                    let color = GetPixel(capture.hdc_mem, x, y);
                    let r = (color.0 & 0xFF) as u8;
                    let g = ((color.0 >> 8) & 0xFF) as u8;
                    let b = ((color.0 >> 16) & 0xFF) as u8;
                    return (r, g, b);
                }
            }
        }
    }
    (0, 0, 0)
}

fn update_cursor_and_color() {
    unsafe {
        let mut pt = POINT::default();
        GetCursorPos(&mut pt);
        *CURSOR_X.lock().unwrap() = pt.x;
        *CURSOR_Y.lock().unwrap() = pt.y;
        
        let (r, g, b) = get_pixel_from_capture(pt.x, pt.y);
        *COLOR_R.lock().unwrap() = r;
        *COLOR_G.lock().unwrap() = g;
        *COLOR_B.lock().unwrap() = b;
    }
}

fn get_window_size() -> i32 {
    let zoom = *CURRENT_ZOOM.lock().unwrap();
    let captured = *CURRENT_CAPTURED.lock().unwrap();
    (captured * zoom) as i32 + BORDER_WIDTH as i32 * 2 + 40
}

fn stop_app() {
    *SHOULD_QUIT.lock().unwrap() = true;
    unsafe { PostQuitMessage(0); }
}

// =============================================================================
// DESSIN
// =============================================================================

fn draw_picker(hwnd: HWND, hdc: HDC) {
    let zoom = *CURRENT_ZOOM.lock().unwrap();
    let captured = *CURRENT_CAPTURED.lock().unwrap() as i32;
    let cursor_x = *CURSOR_X.lock().unwrap();
    let cursor_y = *CURSOR_Y.lock().unwrap();
    let r = *COLOR_R.lock().unwrap();
    let g = *COLOR_G.lock().unwrap();
    let b = *COLOR_B.lock().unwrap();
    let fg_mode = *FG_MODE.lock().unwrap();
    
    let zoom_int = zoom as i32;
    let mag_size = captured * zoom_int;
    let half_captured = captured / 2;
    
    unsafe {
        let mut rect = RECT::default();
        GetClientRect(hwnd, &mut rect);
        let win_w = rect.right;
        let win_h = rect.bottom;
        
        // Fond blanc
        let white_brush = CreateSolidBrush(COLORREF(0xFFFFFF));
        FillRect(hdc, &rect, white_brush);
        DeleteObject(white_brush);
        
        // Centre de la fenêtre
        let cx = win_w / 2;
        let cy = win_h / 2;
        let mag_x = cx - mag_size / 2;
        let mag_y = cy - mag_size / 2;
        
        // Dessine les pixels zoomés depuis la capture
        if let Ok(cap) = SCREEN_CAPTURE.lock() {
            if let Some(ref capture) = *cap {
                for py in 0..captured {
                    for px in 0..captured {
                        let src_x = cursor_x - half_captured + px;
                        let src_y = cursor_y - half_captured + py;
                        
                        let color = if src_x >= 0 && src_x < capture.width && src_y >= 0 && src_y < capture.height {
                            GetPixel(capture.hdc_mem, src_x, src_y)
                        } else {
                            COLORREF(0)
                        };
                        
                        let brush = CreateSolidBrush(color);
                        let dest = RECT {
                            left: mag_x + px * zoom_int,
                            top: mag_y + py * zoom_int,
                            right: mag_x + (px + 1) * zoom_int,
                            bottom: mag_y + (py + 1) * zoom_int,
                        };
                        FillRect(hdc, &dest, brush);
                        DeleteObject(brush);
                    }
                }
            }
        }
        
        // Réticule central
        let ret_x = mag_x + (captured / 2) * zoom_int;
        let ret_y = mag_y + (captured / 2) * zoom_int;
        let pen = CreatePen(PS_SOLID, 2, COLORREF(0x808080));
        let old_pen = SelectObject(hdc, pen);
        let null_brush = GetStockObject(NULL_BRUSH);
        let old_brush = SelectObject(hdc, null_brush);
        Rectangle(hdc, ret_x, ret_y, ret_x + zoom_int, ret_y + zoom_int);
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        DeleteObject(pen);
        
        // Bordure colorée (cercle)
        let color_ref = COLORREF(r as u32 | ((g as u32) << 8) | ((b as u32) << 16));
        let border_pen = CreatePen(PS_SOLID, BORDER_WIDTH as i32, color_ref);
        let old_pen2 = SelectObject(hdc, border_pen);
        let old_brush2 = SelectObject(hdc, null_brush);
        Ellipse(hdc, mag_x - 5, mag_y - 5, mag_x + mag_size + 5, mag_y + mag_size + 5);
        SelectObject(hdc, old_pen2);
        SelectObject(hdc, old_brush2);
        DeleteObject(border_pen);
        
        // Texte hex
        let hex = format!("{} #{:02X}{:02X}{:02X}", if fg_mode { "FG" } else { "BG" }, r, g, b);
        let luminance = 0.299 * (r as f64) + 0.587 * (g as f64) + 0.114 * (b as f64);
        let text_color = if luminance > 128.0 { COLORREF(0) } else { COLORREF(0xFFFFFF) };
        
        // Fond pour le texte
        let text_bg = CreateSolidBrush(color_ref);
        let text_rect = RECT {
            left: cx - 60,
            top: mag_y + mag_size + 15,
            right: cx + 60,
            bottom: mag_y + mag_size + 35,
        };
        FillRect(hdc, &text_rect, text_bg);
        DeleteObject(text_bg);
        
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, text_color);
        let text_wide: Vec<u16> = hex.encode_utf16().collect();
        SetTextAlign(hdc, TA_CENTER);
        TextOutW(hdc, cx, mag_y + mag_size + 17, &text_wide);
    }
}

// =============================================================================
// ÉVÉNEMENTS
// =============================================================================

fn handle_key(hwnd: HWND, vk: VIRTUAL_KEY) {
    let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) < 0 };
    let amount = if shift { SHIFT_MOVE_PIXELS as i32 } else { 1 };
    
    match vk {
        VK_ESCAPE => stop_app(),
        VK_RETURN | VK_SPACE => select_color(),
        VK_C => {
            let mut cm = CONTINUE_MODE.lock().unwrap();
            *cm = !*cm;
        }
        VK_I => {
            if shift {
                let mut c = CURRENT_CAPTURED.lock().unwrap();
                *c = (*c + CAPTURED_PIXELS_STEP).min(CAPTURED_PIXELS_MAX);
            } else {
                let mut z = CURRENT_ZOOM.lock().unwrap();
                *z = (*z + ZOOM_STEP).min(ZOOM_MAX);
            }
            resize_window(hwnd);
        }
        VK_O => {
            if shift {
                let mut c = CURRENT_CAPTURED.lock().unwrap();
                *c = (*c - CAPTURED_PIXELS_STEP).max(CAPTURED_PIXELS_MIN);
            } else {
                let mut z = CURRENT_ZOOM.lock().unwrap();
                *z = (*z - ZOOM_STEP).max(ZOOM_MIN);
            }
            resize_window(hwnd);
        }
        VK_LEFT => unsafe { 
            let mut pt = POINT::default();
            GetCursorPos(&mut pt);
            SetCursorPos(pt.x - amount, pt.y);
        },
        VK_RIGHT => unsafe {
            let mut pt = POINT::default();
            GetCursorPos(&mut pt);
            SetCursorPos(pt.x + amount, pt.y);
        },
        VK_UP => unsafe {
            let mut pt = POINT::default();
            GetCursorPos(&mut pt);
            SetCursorPos(pt.x, pt.y - amount);
        },
        VK_DOWN => unsafe {
            let mut pt = POINT::default();
            GetCursorPos(&mut pt);
            SetCursorPos(pt.x, pt.y + amount);
        },
        _ => {}
    }
}

fn handle_wheel(hwnd: HWND, delta: i16) {
    let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) < 0 };
    let up = delta > 0;
    
    if shift {
        let mut c = CURRENT_CAPTURED.lock().unwrap();
        if up {
            *c = (*c + CAPTURED_PIXELS_STEP).min(CAPTURED_PIXELS_MAX);
        } else {
            *c = (*c - CAPTURED_PIXELS_STEP).max(CAPTURED_PIXELS_MIN);
        }
    } else {
        let mut z = CURRENT_ZOOM.lock().unwrap();
        if up {
            *z = (*z + ZOOM_STEP).min(ZOOM_MAX);
        } else {
            *z = (*z - ZOOM_STEP).max(ZOOM_MIN);
        }
    }
    resize_window(hwnd);
}

fn select_color() {
    let r = *COLOR_R.lock().unwrap();
    let g = *COLOR_G.lock().unwrap();
    let b = *COLOR_B.lock().unwrap();
    let fg_mode = *FG_MODE.lock().unwrap();
    let continue_mode = *CONTINUE_MODE.lock().unwrap();
    
    if continue_mode {
        let has_other = if fg_mode {
            BG_COLOR.lock().unwrap().is_some()
        } else {
            FG_COLOR.lock().unwrap().is_some()
        };
        
        if fg_mode {
            *FG_COLOR.lock().unwrap() = Some((r, g, b));
        } else {
            *BG_COLOR.lock().unwrap() = Some((r, g, b));
        }
        
        if has_other {
            stop_app();
        } else {
            *FG_MODE.lock().unwrap() = !fg_mode;
        }
    } else {
        if fg_mode {
            *FG_COLOR.lock().unwrap() = Some((r, g, b));
        } else {
            *BG_COLOR.lock().unwrap() = Some((r, g, b));
        }
        stop_app();
    }
}

fn resize_window(hwnd: HWND) {
    let size = get_window_size();
    unsafe {
        SetWindowPos(hwnd, HWND::default(), 0, 0, size, size, 
            SWP_NOMOVE | SWP_NOZORDER);
    }
}

fn move_window_to_cursor(hwnd: HWND) {
    let cx = *CURSOR_X.lock().unwrap();
    let cy = *CURSOR_Y.lock().unwrap();
    let size = get_window_size();
    
    // Position avec offset pour ne pas cacher le curseur
    let mut x = cx + 30;
    let mut y = cy + 30;
    
    // Ajuste si sort de l'écran
    unsafe {
        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        
        if x + size > sw { x = cx - size - 10; }
        if y + size > sh { y = cy - size - 10; }
        if x < 0 { x = 0; }
        if y < 0 { y = 0; }
        
        SetWindowPos(hwnd, HWND::default(), x, y, 0, 0, 
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

// =============================================================================
// WINDOW PROC
// =============================================================================

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                SetTimer(hwnd, TIMER_ID, TIMER_INTERVAL, None);
                LRESULT(0)
            }
            WM_DESTROY => {
                KillTimer(hwnd, TIMER_ID);
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_TIMER => {
                update_cursor_and_color();
                move_window_to_cursor(hwnd);
                InvalidateRect(hwnd, None, FALSE);
                LRESULT(0)
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                draw_picker(hwnd, hdc);
                EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_KEYDOWN => {
                handle_key(hwnd, VIRTUAL_KEY(wp.0 as u16));
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let delta = ((wp.0 >> 16) & 0xFFFF) as i16;
                handle_wheel(hwnd, delta);
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                select_color();
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp)
        }
    }
}

// =============================================================================
// API PUBLIQUE
// =============================================================================

pub fn run(fg: bool) -> ColorPickerResult {
    reset_state();
    *FG_MODE.lock().unwrap() = fg;
    
    // Capture l'écran une fois au démarrage
    capture_full_screen();
    
    unsafe {
        let hinst = GetModuleHandleW(None).unwrap();
        let class_wide: Vec<u16> = WINDOW_CLASS_NAME.encode_utf16().chain(std::iter::once(0)).collect();
        let class_name = PCWSTR(class_wide.as_ptr());
        
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst.into(),
            hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
            lpszClassName: class_name,
            ..Default::default()
        };
        
        if RegisterClassExW(&wc) == 0 {
            return ColorPickerResult { foreground: None, background: None };
        }
        
        let size = get_window_size();
        
        // Fenêtre popup sans bordure, toujours au-dessus
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
            w!("Color Picker"),
            WS_POPUP | WS_VISIBLE,
            100, 100, size, size,
            None, None, hinst, None,
        );
        
        if hwnd.is_err() {
            return ColorPickerResult { foreground: None, background: None };
        }
        
        let hwnd = hwnd.unwrap();
        
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        SetFocus(hwnd);
        
        // Boucle de messages
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND::default(), 0, 0).into() {
            if *SHOULD_QUIT.lock().unwrap() { break; }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        
        DestroyWindow(hwnd);
        UnregisterClassW(class_name, hinst);
        
        // Libère la capture d'écran
        if let Ok(mut cap) = SCREEN_CAPTURE.lock() {
            if let Some(c) = cap.take() {
                DeleteObject(c.hbitmap);
                DeleteDC(c.hdc_mem);
            }
        }
    }
    
    ColorPickerResult {
        foreground: *FG_COLOR.lock().unwrap(),
        background: *BG_COLOR.lock().unwrap(),
    }
}