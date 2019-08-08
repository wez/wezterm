use super::*;
use failure::Fallible;
use std::io::Error as IoError;
use std::ptr::{null, null_mut};
use std::sync::{Arc, Mutex};
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::winuser::*;

pub trait WindowCallbacks {
    fn can_close(&mut self) -> bool {
        true
    }
}

struct WindowInner {
    /// Non-owning reference to the window handle
    hwnd: HWND,
    callbacks: Box<WindowCallbacks>,
}

pub struct Window {
    inner: Arc<Mutex<WindowInner>>,
}

fn adjust_client_to_window_dimensions(width: usize, height: usize) -> (i32, i32) {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: width as _,
        bottom: height as _,
    };
    unsafe { AdjustWindowRect(&mut rect, WS_POPUP | WS_SYSMENU | WS_CAPTION, 0) };

    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    (width, height)
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

        match Self::create_window(class_name, name, width, height, raw) {
            Ok(hwnd) => hwnd,
            Err(err) => {
                // Ensure that we drop the extra ref to raw before we return
                drop(unsafe { Arc::from_raw(raw) });
                return Err(err);
            }
        };

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
unsafe fn wm_nccreate(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let create: &CREATESTRUCTW = &*(lparam as *const CREATESTRUCTW);
    let inner = arc_from_pointer(create.lpCreateParams);
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as _);
    inner.lock().unwrap().hwnd = hwnd;

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Called when the window is being destroyed.
/// Goal is to release the WindowInner reference that was stashed
/// in the window by wm_nccreate.
unsafe fn wm_ncdestroy(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as LPVOID;
    if !raw.is_null() {
        let inner = take_arc_from_pointer(raw);
        inner.lock().unwrap().hwnd = null_mut();
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe fn do_wnd_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => return wm_nccreate(hwnd, msg, wparam, lparam),
        WM_NCDESTROY => return wm_ncdestroy(hwnd, msg, wparam, lparam),
        WM_CLOSE => {
            if let Some(inner) = arc_from_hwnd(hwnd) {
                let mut inner = inner.lock().unwrap();
                if !inner.callbacks.can_close() {
                    // Don't let it close
                    return 0;
                }
            }
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match std::panic::catch_unwind(|| do_wnd_proc(hwnd, msg, wparam, lparam)) {
        Ok(result) => result,
        Err(_) => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn run_message_loop() -> Fallible<()> {
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    loop {
        let res = unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) };
        if res == -1 {
            return Err(IoError::last_os_error().into());
        }
        if res == 0 {
            return Ok(());
        }

        unsafe {
            TranslateMessage(&mut msg);
            DispatchMessageW(&mut msg);
        }
    }
}

pub fn terminate_message_loop() {
    unsafe {
        PostQuitMessage(0);
    }
}
