use super::gdi::*;
use super::*;
use crate::bitmaps::*;
use crate::color::Color;
use crate::{Dimensions, Operator, PaintContext, WindowCallbacks};
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
        let inner = self.inner.lock().unwrap();
        unsafe { ShowWindow(inner.hwnd, SW_NORMAL) };
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

unsafe fn do_wnd_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    match msg {
        WM_NCCREATE => wm_nccreate(hwnd, msg, wparam, lparam),
        WM_NCDESTROY => wm_ncdestroy(hwnd, msg, wparam, lparam),
        WM_PAINT => wm_paint(hwnd, msg, wparam, lparam),
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
