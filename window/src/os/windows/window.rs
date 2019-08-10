use super::gdi::*;
use super::*;
use crate::bitmaps::*;
use crate::color::Color;
use crate::{Dimensions, KeyCode, KeyEvent, Modifiers, Operator, PaintContext, WindowCallbacks};
use failure::Fallible;
use std::io::Error as IoError;
use std::ptr::{null, null_mut};
use std::sync::{Arc, Mutex};
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

struct WindowInner {
    /// Non-owning reference to the window handle
    hwnd: HWND,
    callbacks: Box<WindowCallbacks>,
}

pub struct Window {
    inner: Arc<Mutex<WindowInner>>,
}

fn rect_width(r: &RECT) -> i32 {
    r.right - r.left
}

fn rect_height(r: &RECT) -> i32 {
    r.bottom - r.top
}

fn adjust_client_to_window_dimensions(width: usize, height: usize) -> (i32, i32) {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: width as _,
        bottom: height as _,
    };
    unsafe { AdjustWindowRect(&mut rect, WS_POPUP | WS_SYSMENU | WS_CAPTION, 0) };

    (rect_width(&rect), rect_height(&rect))
}

fn arc_to_pointer(arc: &Arc<Mutex<WindowInner>>) -> *const Mutex<WindowInner> {
    let cloned = Arc::clone(arc);
    Arc::into_raw(cloned)
}

fn arc_from_pointer(lparam: LPVOID) -> Arc<Mutex<WindowInner>> {
    // Turn it into an arc
    let arc = unsafe { Arc::from_raw(std::mem::transmute(lparam)) };
    // Add a ref for the caller
    let cloned = Arc::clone(&arc);

    // We must not drop this ref though; turn it back into a raw pointer!
    Arc::into_raw(arc);

    cloned
}

fn arc_from_hwnd(hwnd: HWND) -> Option<Arc<Mutex<WindowInner>>> {
    let raw = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as LPVOID };
    if raw.is_null() {
        None
    } else {
        Some(arc_from_pointer(raw))
    }
}

fn take_arc_from_pointer(lparam: LPVOID) -> Arc<Mutex<WindowInner>> {
    unsafe { Arc::from_raw(std::mem::transmute(lparam)) }
}

impl Window {
    fn create_window(
        class_name: &str,
        name: &str,
        width: usize,
        height: usize,
        lparam: *const Mutex<WindowInner>,
    ) -> Fallible<HWND> {
        // Jamming this in here; it should really live in the application manifest,
        // but having it here means that we don't have to create a manifest
        unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }

        let class_name = wide_string(class_name);
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW | CS_OWNDC,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: unsafe { GetModuleHandleW(null()) },
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class_name.as_ptr(),
        };

        if unsafe { RegisterClassW(&class) } == 0 {
            let err = IoError::last_os_error();
            match err.raw_os_error() {
                Some(code)
                    if code == winapi::shared::winerror::ERROR_CLASS_ALREADY_EXISTS as i32 => {}
                _ => return Err(err.into()),
            }
        }

        let (width, height) = adjust_client_to_window_dimensions(width, height);

        let name = wide_string(name);
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                name.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                width,
                height,
                null_mut(),
                null_mut(),
                null_mut(),
                std::mem::transmute(lparam),
            )
        };

        if hwnd.is_null() {
            let err = IoError::last_os_error();
            failure::bail!("CreateWindowExW: {}", err);
        }

        Ok(hwnd)
    }

    pub fn new_window(
        class_name: &str,
        name: &str,
        width: usize,
        height: usize,
        callbacks: Box<WindowCallbacks>,
    ) -> Fallible<Window> {
        let inner = Arc::new(Mutex::new(WindowInner {
            hwnd: null_mut(),
            callbacks,
        }));

        // Careful: `raw` owns a ref to inner, but there is no Drop impl
        let raw = arc_to_pointer(&inner);

        let hwnd = match Self::create_window(class_name, name, width, height, raw) {
            Ok(hwnd) => hwnd,
            Err(err) => {
                // Ensure that we drop the extra ref to raw before we return
                drop(unsafe { Arc::from_raw(raw) });
                return Err(err);
            }
        };

        enable_dark_mode(hwnd);

        Ok(Window { inner })
    }

    pub fn show(&self) {
        // ShowWindow can call to the window proc and may attempt
        // to lock inner, so take care here!
        let hwnd = self.inner.lock().unwrap().hwnd;
        unsafe { ShowWindow(hwnd, SW_NORMAL) };
    }
}

/// Set up bidirectional pointers:
/// hwnd.USERDATA -> WindowInner
/// WindowInner.hwnd -> hwnd
unsafe fn wm_nccreate(hwnd: HWND, _msg: UINT, _wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    let create: &CREATESTRUCTW = &*(lparam as *const CREATESTRUCTW);
    let inner = arc_from_pointer(create.lpCreateParams);
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as _);
    inner.lock().unwrap().hwnd = hwnd;

    None
}

/// Called when the window is being destroyed.
/// Goal is to release the WindowInner reference that was stashed
/// in the window by wm_nccreate.
unsafe fn wm_ncdestroy(
    hwnd: HWND,
    _msg: UINT,
    _wparam: WPARAM,
    _lparam: LPARAM,
) -> Option<LRESULT> {
    let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as LPVOID;
    if !raw.is_null() {
        let inner = take_arc_from_pointer(raw);
        let mut inner = inner.lock().unwrap();
        inner.callbacks.destroy();
        inner.hwnd = null_mut();
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
    }

    None
}

fn enable_dark_mode(hwnd: HWND) {
    // Prefer to run in dark mode. This could be made configurable without
    // a huge amount of effort, but I think it's fine to just be always
    // dark mode by default :-p
    // Note that the MS terminal app uses the logic found here for this
    // stuff:
    // https://github.com/microsoft/terminal/blob/9b92986b49bed8cc41fde4d6ef080921c41e6d9e/src/interactivity/win32/windowtheme.cpp#L62
    use winapi::um::dwmapi::DwmSetWindowAttribute;
    use winapi::um::uxtheme::SetWindowTheme;

    const DWMWA_USE_IMMERSIVE_DARK_MODE: DWORD = 19;
    unsafe {
        SetWindowTheme(
            hwnd as _,
            wide_string("DarkMode_Explorer").as_slice().as_ptr(),
            std::ptr::null_mut(),
        );

        let enabled: BOOL = 1;
        DwmSetWindowAttribute(
            hwnd as _,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &enabled as *const _ as *const _,
            std::mem::size_of_val(&enabled) as u32,
        );
    }
}

struct GdiGraphicsContext {
    bitmap: GdiBitmap,
    dpi: u32,
}

impl PaintContext for GdiGraphicsContext {
    fn clear_rect(
        &mut self,
        dest_x: isize,
        dest_y: isize,
        width: usize,
        height: usize,
        color: Color,
    ) {
        self.bitmap.clear_rect(dest_x, dest_y, width, height, color)
    }

    fn clear(&mut self, color: Color) {
        self.bitmap.clear(color);
    }

    fn get_dimensions(&self) -> Dimensions {
        let (pixel_width, pixel_height) = self.bitmap.image_dimensions();
        Dimensions {
            pixel_width,
            pixel_height,
            dpi: self.dpi as usize,
        }
    }

    fn draw_image_subset(
        &mut self,
        dest_x: isize,
        dest_y: isize,
        src_x: usize,
        src_y: usize,
        width: usize,
        height: usize,
        im: &dyn BitmapImage,
        operator: Operator,
    ) {
        self.bitmap
            .draw_image_subset(dest_x, dest_y, src_x, src_y, width, height, im, operator)
    }
}

unsafe fn wm_size(hwnd: HWND, _msg: UINT, _wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    if let Some(inner) = arc_from_hwnd(hwnd) {
        let mut inner = inner.lock().unwrap();
        let pixel_width = LOWORD(lparam as DWORD) as usize;
        let pixel_height = HIWORD(lparam as DWORD) as usize;
        inner.callbacks.resize(Dimensions {
            pixel_width,
            pixel_height,
            dpi: GetDpiForWindow(hwnd) as usize,
        });
    }
    None
}

unsafe fn wm_paint(hwnd: HWND, _msg: UINT, _wparam: WPARAM, _lparam: LPARAM) -> Option<LRESULT> {
    if let Some(inner) = arc_from_hwnd(hwnd) {
        let mut inner = inner.lock().unwrap();

        let mut ps = PAINTSTRUCT {
            fErase: 0,
            fIncUpdate: 0,
            fRestore: 0,
            hdc: std::ptr::null_mut(),
            rcPaint: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            rgbReserved: [0; 32],
        };
        let dc = BeginPaint(hwnd, &mut ps);

        let mut rect = RECT {
            left: 0,
            bottom: 0,
            right: 0,
            top: 0,
        };
        GetClientRect(hwnd, &mut rect);
        let width = rect_width(&rect) as usize;
        let height = rect_height(&rect) as usize;

        if width > 0 && height > 0 {
            let dpi = GetDpiForWindow(hwnd);
            let bitmap = GdiBitmap::new_compatible(width, height, dc).unwrap();
            let mut context = GdiGraphicsContext { dpi, bitmap };

            inner.callbacks.paint(&mut context);
            BitBlt(
                dc,
                0,
                0,
                width as i32,
                height as i32,
                context.bitmap.hdc(),
                0,
                0,
                SRCCOPY,
            );
        }

        EndPaint(hwnd, &mut ps);

        Some(0)
    } else {
        None
    }
}

unsafe fn key(hwnd: HWND, _msg: UINT, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    if let Some(inner) = arc_from_hwnd(hwnd) {
        let mut inner = inner.lock().unwrap();
        let repeat = (lparam & 0xffff) as u16;
        let scan_code = ((lparam >> 16) & 0xff) as u8;
        let releasing = (lparam & (1 << 31)) != 0;

        /*
        let alt_pressed = (lparam & (1 << 29)) != 0;
        let was_down = (lparam & (1 << 30)) != 0;
        let label = match msg {
            WM_CHAR => "WM_CHAR",
            WM_KEYDOWN => "WM_KEYDOWN",
            WM_KEYUP => "WM_KEYUP",
            WM_SYSKEYUP => "WM_SYSKEYUP",
            WM_SYSKEYDOWN => "WM_SYSKEYDOWN",
            _ => "WAT",
        };
        eprintln!(
            "{} c=`{}` repeat={} scan={} alt_pressed={} was_down={} releasing={}",
            label, wparam, repeat, scan_code, alt_pressed, was_down, releasing
        );
        */

        let mut keys = [0u8; 256];
        GetKeyboardState(keys.as_mut_ptr());

        let mut modifiers = Modifiers::default();
        if keys[VK_CONTROL as usize] & 0x80 != 0 {
            modifiers |= Modifiers::CTRL;
        }
        if keys[VK_SHIFT as usize] & 0x80 != 0 {
            modifiers |= Modifiers::SHIFT;
        }
        if keys[VK_MENU as usize] & 0x80 != 0 {
            modifiers |= Modifiers::ALT;
        }
        if keys[VK_LWIN as usize] & 0x80 != 0 || keys[VK_RWIN as usize] & 0x80 != 0 {
            modifiers |= Modifiers::SUPER;
        }

        // If control is pressed, clear the shift state.
        // That gives us a normalized, unshifted/lowercase version of the
        // key for processing elsewhere.
        if modifiers.contains(Modifiers::CTRL) {
            keys[VK_CONTROL as usize] = 0;
            keys[VK_LCONTROL as usize] = 0;
            keys[VK_RCONTROL as usize] = 0;
            keys[VK_SHIFT as usize] = 0;
            keys[VK_LSHIFT as usize] = 0;
            keys[VK_RSHIFT as usize] = 0;
        }

        let mut out = [0u16; 16];
        let res = ToUnicode(
            wparam as u32,
            scan_code as u32,
            keys.as_ptr(),
            out.as_mut_ptr(),
            out.len() as i32,
            0,
        );
        let key = match res {
            // dead key
            -1 => None,
            0 => {
                // No unicode translation, so map the scan code to a virtual key
                // code, and from there map it to our KeyCode type
                match MapVirtualKeyW(scan_code.into(), MAPVK_VSC_TO_VK_EX) as i32 {
                    0 => None,
                    VK_CANCEL => Some(KeyCode::Cancel),
                    VK_BACK => Some(KeyCode::Char('\u{8}')),
                    VK_TAB => Some(KeyCode::Char('\t')),
                    VK_CLEAR => Some(KeyCode::Clear),
                    VK_RETURN => Some(KeyCode::Char('\r')),
                    VK_SHIFT => Some(KeyCode::Shift),
                    VK_CONTROL => Some(KeyCode::Control),
                    VK_MENU => Some(KeyCode::Alt),
                    VK_PAUSE => Some(KeyCode::Pause),
                    VK_CAPITAL => Some(KeyCode::CapsLock),
                    VK_ESCAPE => Some(KeyCode::Char('\u{1b}')),
                    VK_SPACE => Some(KeyCode::Char(' ')),
                    VK_PRIOR => Some(KeyCode::PageUp),
                    VK_NEXT => Some(KeyCode::PageDown),
                    VK_END => Some(KeyCode::End),
                    VK_HOME => Some(KeyCode::Home),
                    VK_LEFT => Some(KeyCode::LeftArrow),
                    VK_UP => Some(KeyCode::UpArrow),
                    VK_RIGHT => Some(KeyCode::RightArrow),
                    VK_DOWN => Some(KeyCode::DownArrow),
                    VK_SELECT => Some(KeyCode::Select),
                    VK_PRINT => Some(KeyCode::Print),
                    VK_EXECUTE => Some(KeyCode::Execute),
                    VK_SNAPSHOT => Some(KeyCode::PrintScreen),
                    VK_INSERT => Some(KeyCode::Insert),
                    VK_DELETE => Some(KeyCode::Char('\u{7f}')),
                    VK_HELP => Some(KeyCode::Help),
                    // 0-9 happen to overlap with ascii
                    i @ 0x30..=0x39 => Some(KeyCode::Char(i as u8 as char)),
                    // a-z also overlap with ascii
                    i @ 0x41..=0x5a => Some(KeyCode::Char(i as u8 as char)),
                    VK_LWIN => Some(KeyCode::LeftWindows),
                    VK_RWIN => Some(KeyCode::RightWindows),
                    VK_APPS => Some(KeyCode::Applications),
                    VK_SLEEP => Some(KeyCode::Sleep),
                    i @ VK_NUMPAD0..=VK_NUMPAD9 => Some(KeyCode::Numpad((i - VK_NUMPAD0) as u8)),
                    VK_MULTIPLY => Some(KeyCode::Multiply),
                    VK_ADD => Some(KeyCode::Add),
                    VK_SEPARATOR => Some(KeyCode::Separator),
                    VK_SUBTRACT => Some(KeyCode::Subtract),
                    VK_DECIMAL => Some(KeyCode::Decimal),
                    VK_DIVIDE => Some(KeyCode::Divide),
                    i @ VK_F1..=VK_F24 => Some(KeyCode::Function((1 + i - VK_F1) as u8)),
                    VK_NUMLOCK => Some(KeyCode::NumLock),
                    VK_SCROLL => Some(KeyCode::ScrollLock),
                    VK_LSHIFT => Some(KeyCode::LeftShift),
                    VK_RSHIFT => Some(KeyCode::RightShift),
                    VK_LCONTROL => Some(KeyCode::LeftControl),
                    VK_RCONTROL => Some(KeyCode::RightControl),
                    VK_LMENU => Some(KeyCode::LeftAlt),
                    VK_RMENU => Some(KeyCode::RightAlt),
                    VK_BROWSER_BACK => Some(KeyCode::BrowserBack),
                    VK_BROWSER_FORWARD => Some(KeyCode::BrowserForward),
                    VK_BROWSER_REFRESH => Some(KeyCode::BrowserRefresh),
                    VK_BROWSER_STOP => Some(KeyCode::BrowserStop),
                    VK_BROWSER_SEARCH => Some(KeyCode::BrowserSearch),
                    VK_BROWSER_FAVORITES => Some(KeyCode::BrowserFavorites),
                    VK_BROWSER_HOME => Some(KeyCode::BrowserHome),
                    VK_VOLUME_MUTE => Some(KeyCode::VolumeMute),
                    VK_VOLUME_DOWN => Some(KeyCode::VolumeDown),
                    VK_VOLUME_UP => Some(KeyCode::VolumeUp),
                    VK_MEDIA_NEXT_TRACK => Some(KeyCode::MediaNextTrack),
                    VK_MEDIA_PREV_TRACK => Some(KeyCode::MediaPrevTrack),
                    VK_MEDIA_STOP => Some(KeyCode::MediaStop),
                    VK_MEDIA_PLAY_PAUSE => Some(KeyCode::MediaPlayPause),
                    _ => None,
                }
            }
            1 => Some(KeyCode::Char(std::char::from_u32_unchecked(out[0] as u32))),
            n => {
                let s = &out[0..n as usize];
                match String::from_utf16(s) {
                    Ok(s) => Some(KeyCode::Composed(s)),
                    Err(err) => {
                        eprintln!("translated to {} WCHARS, err: {}", n, err);
                        None
                    }
                }
            }
        };

        if let Some(key) = key {
            let key = KeyEvent {
                key,
                modifiers,
                repeat_count: repeat,
                key_is_down: !releasing,
            };
            if inner.callbacks.key_event(&key) {
                return Some(0);
            }
        }
    }
    None
}

unsafe fn do_wnd_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    match msg {
        WM_NCCREATE => wm_nccreate(hwnd, msg, wparam, lparam),
        WM_NCDESTROY => wm_ncdestroy(hwnd, msg, wparam, lparam),
        WM_PAINT => wm_paint(hwnd, msg, wparam, lparam),
        WM_SIZE => wm_size(hwnd, msg, wparam, lparam),
        WM_KEYDOWN | WM_KEYUP | WM_SYSKEYUP | WM_SYSKEYDOWN => key(hwnd, msg, wparam, lparam),
        WM_CLOSE => {
            if let Some(inner) = arc_from_hwnd(hwnd) {
                let mut inner = inner.lock().unwrap();
                if !inner.callbacks.can_close() {
                    // Don't let it close
                    return Some(0);
                }
            }
            None
        }
        _ => None,
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match std::panic::catch_unwind(|| {
        do_wnd_proc(hwnd, msg, wparam, lparam)
            .unwrap_or_else(|| DefWindowProcW(hwnd, msg, wparam, lparam))
    }) {
        Ok(result) => result,
        Err(_) => std::process::exit(1),
    }
}
