// =============================================================================
// COLOR PICKER - VERSION WINDOWS
// =============================================================================
// Fenêtre plein écran affichant la capture d'écran + loupe
// Fullscreen window displaying screen capture + magnifier
// =============================================================================

use crate::config::{
    BORDER_WIDTH, CAPTURED_PIXELS, INITIAL_ZOOM_FACTOR,
    SHIFT_MOVE_PIXELS, ZOOM_MIN, ZOOM_MAX, ZOOM_STEP,
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
const WINDOW_CLASS: &str = "ColorPickerFullscreen";
const TIMER_ID: usize = 1;

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

static STATE: Mutex<PickerState> = Mutex::new(PickerState::new());

struct PickerState {
    cursor_x: i32,
    cursor_y: i32,
    color: (u8, u8, u8),
    fg_color: Option<(u8, u8, u8)>,
    bg_color: Option<(u8, u8, u8)>,
    fg_mode: bool,
    continue_mode: bool,
    zoom: f64,
    captured: f64,
    quit: bool,
    screen_width: i32,
    screen_height: i32,
}

// Handle du bitmap de capture (doit être global pour WM_PAINT)
static SCREEN_BITMAP: Mutex<Option<isize>> = Mutex::new(None);
static SCREEN_DATA: Mutex<Vec<u8>> = Mutex::new(Vec::new());

impl PickerState {
    const fn new() -> Self {
        Self {
            cursor_x: 0,
            cursor_y: 0,
            color: (0, 0, 0),
            fg_color: None,
            bg_color: None,
            fg_mode: true,
            continue_mode: false,
            zoom: INITIAL_ZOOM_FACTOR,
            captured: CAPTURED_PIXELS,
            quit: false,
            screen_width: 0,
            screen_height: 0,
        }
    }
    
    fn reset(&mut self) {
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.color = (0, 0, 0);
        self.fg_color = None;
        self.bg_color = None;
        self.fg_mode = true;
        self.continue_mode = false;
        self.zoom = INITIAL_ZOOM_FACTOR;
        self.captured = CAPTURED_PIXELS;
        self.quit = false;
    }
}

// =============================================================================
// CAPTURE D'ÉCRAN
// =============================================================================

fn capture_screen() {
    unsafe {
        let width = GetSystemMetrics(SM_CXSCREEN);
        let height = GetSystemMetrics(SM_CYSCREEN);
        
        let hdc_screen = GetDC(HWND::default());
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        
        // Crée un bitmap compatible pour le fond
        // Create a compatible bitmap for the background
        let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
        
        if !hbitmap.is_invalid() {
            SelectObject(hdc_mem, hbitmap);
            let _ = BitBlt(hdc_mem, 0, 0, width, height, hdc_screen, 0, 0, SRCCOPY);
            
            // Stocke le handle du bitmap
            if let Ok(mut bmp) = SCREEN_BITMAP.lock() {
                *bmp = Some(hbitmap.0 as isize);
            }
            
            // Capture aussi les données brutes pour lire les couleurs
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // Top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            
            let mut data: Vec<u8> = vec![0; (width * height * 4) as usize];
            let _ = GetDIBits(
                hdc_mem,
                hbitmap,
                0,
                height as u32,
                Some(data.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );
            
            if let Ok(mut screen_data) = SCREEN_DATA.lock() {
                *screen_data = data;
            }
            
            if let Ok(mut state) = STATE.lock() {
                state.screen_width = width;
                state.screen_height = height;
            }
        }
        
        let _ = DeleteDC(hdc_mem);
        let _ = ReleaseDC(HWND::default(), hdc_screen);
    }
}

fn cleanup_screen_bitmap() {
    if let Ok(mut bmp) = SCREEN_BITMAP.lock() {
        if let Some(h) = bmp.take() {
            unsafe {
                let _ = DeleteObject(HBITMAP(h as *mut _));
            }
        }
    }
    if let Ok(mut data) = SCREEN_DATA.lock() {
        data.clear();
    }
}

fn get_pixel_color(x: i32, y: i32) -> (u8, u8, u8) {
    let (width, height) = {
        if let Ok(state) = STATE.lock() {
            (state.screen_width, state.screen_height)
        } else {
            return (0, 0, 0);
        }
    };
    
    if let Ok(data) = SCREEN_DATA.lock() {
        if x >= 0 && x < width && y >= 0 && y < height {
            let idx = ((y * width + x) * 4) as usize;
            if idx + 2 < data.len() {
                let b = data[idx];
                let g = data[idx + 1];
                let r = data[idx + 2];
                return (r, g, b);
            }
        }
    }
    (0, 0, 0)
}

// =============================================================================
// MISE À JOUR
// =============================================================================

fn update_cursor_pos(x: i32, y: i32) {
    let color = get_pixel_color(x, y);
    if let Ok(mut state) = STATE.lock() {
        state.cursor_x = x;
        state.cursor_y = y;
        state.color = color;
    }
}

// =============================================================================
// DESSIN
// =============================================================================

fn paint_window(hwnd: HWND, hdc: HDC) {
    let (cursor_x, cursor_y, color, fg_mode, continue_mode, zoom, captured, 
         screen_width, screen_height) = {
        let state = match STATE.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        (
            state.cursor_x, state.cursor_y, state.color,
            state.fg_mode, state.continue_mode,
            state.zoom, state.captured,
            state.screen_width, state.screen_height,
        )
    };
    
    let screen_data = match SCREEN_DATA.lock() {
        Ok(d) => d.clone(),
        Err(_) => return,
    };
    
    if screen_data.is_empty() { return; }
    
    unsafe {
        // Crée un buffer double pour éviter le scintillement
        // Create a double buffer to avoid flickering
        let hdc_mem = CreateCompatibleDC(hdc);
        let hbitmap = CreateCompatibleBitmap(hdc, screen_width, screen_height);
        
        if hbitmap.is_invalid() {
            let _ = DeleteDC(hdc_mem);
            return;
        }
        
        SelectObject(hdc_mem, hbitmap);
        
        // Dessine le fond (capture d'écran)
        if let Ok(bmp) = SCREEN_BITMAP.lock() {
            if let Some(h) = *bmp {
                let hdc_src = CreateCompatibleDC(hdc);
                SelectObject(hdc_src, HBITMAP(h as *mut _));
                let _ = BitBlt(hdc_mem, 0, 0, screen_width, screen_height, hdc_src, 0, 0, SRCCOPY);
                let _ = DeleteDC(hdc_src);
            }
        }
        
        // Paramètres de la loupe
        let mag_size = (captured * zoom) as i32;
        let zoom_i = zoom as i32;
        let captured_i = captured as i32;
        let half_cap = captured_i / 2;
        let border = BORDER_WIDTH as i32;
        let cx = cursor_x;
        let cy = cursor_y;
        let inner_radius = mag_size / 2;
        let outer_radius = inner_radius + border;
        
        // Crée une région circulaire pour la loupe
        let rgn_outer = CreateEllipticRgn(
            cx - outer_radius, cy - outer_radius,
            cx + outer_radius, cy + outer_radius
        );
        let rgn_inner = CreateEllipticRgn(
            cx - inner_radius, cy - inner_radius,
            cx + inner_radius, cy + inner_radius
        );
        
        // Dessine la bordure colorée (anneau)
        let (cr, cg, cb) = color;
        let brush_color = COLORREF(cr as u32 | ((cg as u32) << 8) | ((cb as u32) << 16));
        let brush = CreateSolidBrush(brush_color);
        
        // Soustrait le cercle intérieur pour créer un anneau
        let rgn_ring = CreateEllipticRgn(0, 0, 1, 1); // Dummy
        let _ = CombineRgn(rgn_ring, rgn_outer, rgn_inner, RGN_DIFF);
        let _ = FillRgn(hdc_mem, rgn_ring, brush);
        
        let _ = DeleteObject(brush);
        let _ = DeleteObject(rgn_ring);
        let _ = DeleteObject(rgn_outer);
        
        // Dessine les pixels zoomés dans le cercle intérieur
        let _ = SelectClipRgn(hdc_mem, rgn_inner);
        
        for py in 0..captured_i {
            for px in 0..captured_i {
                let src_x = cursor_x - half_cap + px;
                let src_y = cursor_y - half_cap + py;
                
                let (r, g, b) = if src_x >= 0 && src_x < screen_width && src_y >= 0 && src_y < screen_height {
                    let idx = ((src_y * screen_width + src_x) * 4) as usize;
                    if idx + 2 < screen_data.len() {
                        (screen_data[idx + 2], screen_data[idx + 1], screen_data[idx])
                    } else {
                        (128, 128, 128)
                    }
                } else {
                    (64, 64, 64)
                };
                
                let dst_x = cx - inner_radius + px * zoom_i;
                let dst_y = cy - inner_radius + py * zoom_i;
                
                let pixel_brush = CreateSolidBrush(COLORREF(r as u32 | ((g as u32) << 8) | ((b as u32) << 16)));
                let rect = RECT {
                    left: dst_x,
                    top: dst_y,
                    right: dst_x + zoom_i,
                    bottom: dst_y + zoom_i,
                };
                let _ = FillRect(hdc_mem, &rect, pixel_brush);
                let _ = DeleteObject(pixel_brush);
            }
        }
        
        // Enlève le clip
        let _ = SelectClipRgn(hdc_mem, HRGN::default());
        let _ = DeleteObject(rgn_inner);
        
        // Dessine le réticule central
        let ret_half = zoom_i / 2;
        let ret_x = cx - ret_half;
        let ret_y = cy - ret_half;
        let gray_pen = CreatePen(PS_SOLID, 1, COLORREF(0x606060));
        let old_pen = SelectObject(hdc_mem, gray_pen);
        let null_brush = GetStockObject(NULL_BRUSH);
        let old_brush = SelectObject(hdc_mem, null_brush);
        let _ = Rectangle(hdc_mem, ret_x, ret_y, ret_x + zoom_i, ret_y + zoom_i);
        let _ = SelectObject(hdc_mem, old_pen);
        let _ = SelectObject(hdc_mem, old_brush);
        let _ = DeleteObject(gray_pen);
        
        // Rectangle de texte sous la loupe
        let text_y = cy + inner_radius + border + 5;
        let text_h = 22;
        let text_w = 110;
        let text_x = cx - text_w / 2;
        
        let text_brush = CreateSolidBrush(brush_color);
        let text_rect = RECT {
            left: text_x,
            top: text_y,
            right: text_x + text_w,
            bottom: text_y + text_h,
        };
        let _ = FillRect(hdc_mem, &text_rect, text_brush);
        let _ = DeleteObject(text_brush);
        
        // Texte
        let hex_text = format!("{}: #{:02X}{:02X}{:02X}", 
            if fg_mode { "FG" } else { "BG" }, cr, cg, cb);
        
        let lum = 0.299 * (cr as f64) + 0.587 * (cg as f64) + 0.114 * (cb as f64);
        let text_color = if lum > 128.0 { COLORREF(0) } else { COLORREF(0xFFFFFF) };
        
        let _ = SetBkMode(hdc_mem, TRANSPARENT);
        let _ = SetTextColor(hdc_mem, text_color);
        let _ = SetTextAlign(hdc_mem, TA_CENTER);
        
        let text_wide: Vec<u16> = hex_text.encode_utf16().collect();
        let _ = TextOutW(hdc_mem, cx, text_y + 3, &text_wide);
        
        // Badge mode continue
        if continue_mode {
            let badge_x = cx + inner_radius - 5;
            let badge_y = cy - inner_radius + 5;
            let badge_r = 10;
            
            let red_brush = CreateSolidBrush(COLORREF(0x3232E6)); // Rouge en BGR
            let rgn_badge = CreateEllipticRgn(
                badge_x - badge_r, badge_y - badge_r,
                badge_x + badge_r, badge_y + badge_r
            );
            let _ = FillRgn(hdc_mem, rgn_badge, red_brush);
            let _ = DeleteObject(red_brush);
            let _ = DeleteObject(rgn_badge);
            
            // Lettre C
            let _ = SetTextColor(hdc_mem, COLORREF(0xFFFFFF));
            let c_text: Vec<u16> = "C".encode_utf16().collect();
            let _ = TextOutW(hdc_mem, badge_x, badge_y - 7, &c_text);
        }
        
        // Copie vers l'écran
        let _ = BitBlt(hdc, 0, 0, screen_width, screen_height, hdc_mem, 0, 0, SRCCOPY);
        
        let _ = DeleteObject(hbitmap);
        let _ = DeleteDC(hdc_mem);
    }
}

// =============================================================================
// ÉVÉNEMENTS
// =============================================================================

fn handle_key(hwnd: HWND, vk: VIRTUAL_KEY) {
    let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) < 0 };
    
    match vk {
        VK_ESCAPE => {
            if let Ok(mut state) = STATE.lock() {
                state.quit = true;
            }
            unsafe { PostQuitMessage(0); }
        }
        VK_RETURN | VK_SPACE => select_color(),
        VK_C => {
            if let Ok(mut state) = STATE.lock() {
                state.continue_mode = !state.continue_mode;
            }
            unsafe { let _ = InvalidateRect(hwnd, None, FALSE); }
        }
        VK_I => {
            if let Ok(mut state) = STATE.lock() {
                if shift {
                    state.captured = (state.captured + CAPTURED_PIXELS_STEP).min(CAPTURED_PIXELS_MAX);
                } else {
                    state.zoom = (state.zoom + ZOOM_STEP).min(ZOOM_MAX);
                }
            }
            unsafe { let _ = InvalidateRect(hwnd, None, FALSE); }
        }
        VK_O => {
            if let Ok(mut state) = STATE.lock() {
                if shift {
                    state.captured = (state.captured - CAPTURED_PIXELS_STEP).max(CAPTURED_PIXELS_MIN);
                } else {
                    state.zoom = (state.zoom - ZOOM_STEP).max(ZOOM_MIN);
                }
            }
            unsafe { let _ = InvalidateRect(hwnd, None, FALSE); }
        }
        VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN => {
            let amt = if shift { SHIFT_MOVE_PIXELS as i32 } else { 1 };
            unsafe {
                let mut pt = POINT::default();
                let _ = GetCursorPos(&mut pt);
                match vk {
                    VK_LEFT => pt.x -= amt,
                    VK_RIGHT => pt.x += amt,
                    VK_UP => pt.y -= amt,
                    VK_DOWN => pt.y += amt,
                    _ => {}
                }
                let _ = SetCursorPos(pt.x, pt.y);
                update_cursor_pos(pt.x, pt.y);
                let _ = InvalidateRect(hwnd, None, FALSE);
            }
        }
        _ => {}
    }
}

fn handle_wheel(hwnd: HWND, delta: i16) {
    let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) < 0 };
    let up = delta > 0;
    
    if let Ok(mut state) = STATE.lock() {
        if shift {
            if up {
                state.captured = (state.captured + CAPTURED_PIXELS_STEP).min(CAPTURED_PIXELS_MAX);
            } else {
                state.captured = (state.captured - CAPTURED_PIXELS_STEP).max(CAPTURED_PIXELS_MIN);
            }
        } else {
            if up {
                state.zoom = (state.zoom + ZOOM_STEP).min(ZOOM_MAX);
            } else {
                state.zoom = (state.zoom - ZOOM_STEP).max(ZOOM_MIN);
            }
        }
    }
    unsafe { let _ = InvalidateRect(hwnd, None, FALSE); }
}

fn select_color() {
    if let Ok(mut state) = STATE.lock() {
        let color = state.color;
        
        if state.continue_mode {
            let has_other = if state.fg_mode {
                state.bg_color.is_some()
            } else {
                state.fg_color.is_some()
            };
            
            if state.fg_mode {
                state.fg_color = Some(color);
            } else {
                state.bg_color = Some(color);
            }
            
            if has_other {
                state.quit = true;
                unsafe { PostQuitMessage(0); }
            } else {
                state.fg_mode = !state.fg_mode;
            }
        } else {
            if state.fg_mode {
                state.fg_color = Some(color);
            } else {
                state.bg_color = Some(color);
            }
            state.quit = true;
            unsafe { PostQuitMessage(0); }
        }
    }
}

// =============================================================================
// WINDOW PROCEDURE
// =============================================================================

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let _ = ShowCursor(false);
                let _ = SetTimer(hwnd, TIMER_ID, 16, None);
                LRESULT(0)
            }
            WM_DESTROY => {
                let _ = ShowCursor(true);
                let _ = KillTimer(hwnd, TIMER_ID);
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                paint_window(hwnd, hdc);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_TIMER => {
                let mut pt = POINT::default();
                let _ = GetCursorPos(&mut pt);
                update_cursor_pos(pt.x, pt.y);
                let _ = InvalidateRect(hwnd, None, FALSE);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let x = (lp.0 & 0xFFFF) as i16 as i32;
                let y = ((lp.0 >> 16) & 0xFFFF) as i16 as i32;
                update_cursor_pos(x, y);
                let _ = InvalidateRect(hwnd, None, FALSE);
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                select_color();
                LRESULT(0)
            }
            WM_RBUTTONDOWN => {
                if let Ok(mut state) = STATE.lock() {
                    state.quit = true;
                }
                PostQuitMessage(0);
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
            WM_ERASEBKGND => {
                // Ne pas effacer le fond (évite le scintillement)
                LRESULT(1)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp)
        }
    }
}

// =============================================================================
// API PUBLIQUE
// =============================================================================

pub fn run(fg: bool) -> ColorPickerResult {
    if let Ok(mut state) = STATE.lock() {
        state.reset();
        state.fg_mode = fg;
    }
    
    // Capture l'écran AVANT de créer la fenêtre
    capture_screen();
    
    unsafe {
        let hinst = GetModuleHandleW(None).unwrap();
        let class_wide: Vec<u16> = WINDOW_CLASS.encode_utf16().chain(std::iter::once(0)).collect();
        let class_name = PCWSTR(class_wide.as_ptr());
        
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst.into(),
            hCursor: HCURSOR::default(),
            lpszClassName: class_name,
            ..Default::default()
        };
        
        if RegisterClassExW(&wc) == 0 {
            cleanup_screen_bitmap();
            return ColorPickerResult { foreground: None, background: None };
        }
        
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);
        
        // Fenêtre plein écran, toujours au-dessus
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            class_name,
            w!(""),
            WS_POPUP,
            0, 0, screen_width, screen_height,
            None, None, hinst, None,
        );
        
        if hwnd.is_err() {
            let _ = UnregisterClassW(class_name, hinst);
            cleanup_screen_bitmap();
            return ColorPickerResult { foreground: None, background: None };
        }
        
        let hwnd = hwnd.unwrap();
        
        // Position initiale
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        update_cursor_pos(pt.x, pt.y);
        
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(hwnd);
        let _ = SetCapture(hwnd);
        
        // Boucle de messages
        let mut msg = MSG::default();
        loop {
            let quit = STATE.lock().map(|s| s.quit).unwrap_or(false);
            if quit { break; }
            
            if GetMessageW(&mut msg, HWND::default(), 0, 0).0 <= 0 {
                break;
            }
            
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        
        let _ = ReleaseCapture();
        let _ = DestroyWindow(hwnd);
        let _ = UnregisterClassW(class_name, hinst);
    }
    
    cleanup_screen_bitmap();
    
    if let Ok(state) = STATE.lock() {
        ColorPickerResult {
            foreground: state.fg_color,
            background: state.bg_color,
        }
    } else {
        ColorPickerResult { foreground: None, background: None }
    }
}