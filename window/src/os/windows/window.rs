use super::gdi::*;
use super::*;
use crate::bitmaps::*;
use crate::color::Color;
use crate::connection::ConnectionOps;
use crate::{
    config, Clipboard, Dimensions, KeyCode, KeyEvent, Modifiers, MouseButtons, MouseCursor,
    MouseEvent, MouseEventKind, MousePress, Operator, Point, Rect, ScreenPoint, WindowCallbacks,
    WindowOps, WindowOpsMut,
};
use anyhow::{bail, Context};
use lazy_static::lazy_static;
use promise::Future;
use shared_library::shared_library;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryInto;
use std::ffi::OsString;
use std::io::{self, Error as IoError};
use std::os::windows::ffi::OsStringExt;
use std::ptr::{null, null_mut};
use std::rc::Rc;
use winapi::shared::minwindef::*;
use winapi::shared::ntdef::*;
use winapi::shared::windef::*;
use winapi::um::imm::*;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::wingdi::*;
use winapi::um::winuser::*;
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

const GCS_RESULTSTR: DWORD = 0x800;
extern "system" {
    pub fn ImmGetCompositionStringW(himc: HIMC, index: DWORD, buf: LPVOID, buflen: DWORD) -> LONG;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct HWindow(HWND);
unsafe impl Send for HWindow {}
unsafe impl Sync for HWindow {}

pub(crate) struct WindowInner {
    /// Non-owning reference to the window handle
    hwnd: HWindow,
    callbacks: RefCell<Box<dyn WindowCallbacks>>,
    gl_state: Option<Rc<glium::backend::Context>>,
    /// Fraction of mouse scroll
    hscroll_remainder: i16,
    vscroll_remainder: i16,

    last_size: Option<Dimensions>,
    in_size_move: bool,
    dead_pending: Option<(Modifiers, u32)>,

    keyboard_info: KeyboardLayoutInfo,
}

#[derive(Debug, Clone)]
pub struct Window(HWindow);

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

fn rc_to_pointer(arc: &Rc<RefCell<WindowInner>>) -> *const RefCell<WindowInner> {
    let cloned = Rc::clone(arc);
    Rc::into_raw(cloned)
}

fn rc_from_pointer(lparam: LPVOID) -> Rc<RefCell<WindowInner>> {
    // Turn it into an Rc
    let arc = unsafe { Rc::from_raw(std::mem::transmute(lparam)) };
    // Add a ref for the caller
    let cloned = Rc::clone(&arc);

    // We must not drop this ref though; turn it back into a raw pointer!
    Rc::into_raw(arc);

    cloned
}

fn rc_from_hwnd(hwnd: HWND) -> Option<Rc<RefCell<WindowInner>>> {
    let raw = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as LPVOID };
    if raw.is_null() {
        None
    } else {
        Some(rc_from_pointer(raw))
    }
}

fn take_rc_from_pointer(lparam: LPVOID) -> Rc<RefCell<WindowInner>> {
    unsafe { Rc::from_raw(std::mem::transmute(lparam)) }
}

fn callback_behavior() -> glium::debug::DebugCallbackBehavior {
    if cfg!(debug_assertions) && false
    /* https://github.com/glium/glium/issues/1885 */
    {
        glium::debug::DebugCallbackBehavior::DebugMessageOnError
    } else {
        glium::debug::DebugCallbackBehavior::Ignore
    }
}

impl WindowInner {
    fn enable_opengl(&mut self) -> anyhow::Result<()> {
        let window = Window(self.hwnd);
        let conn = Connection::get().unwrap();

        let gl_state = if config().prefer_egl() {
            match conn.gl_connection.borrow().as_ref() {
                None => crate::egl::GlState::create(None, self.hwnd.0),
                Some(glconn) => {
                    crate::egl::GlState::create_with_existing_connection(glconn, self.hwnd.0)
                }
            }
        } else {
            Err(anyhow::anyhow!("Config says to avoid EGL"))
        }
        .and_then(|egl| unsafe {
            log::trace!("Initialized EGL!");
            conn.gl_connection
                .borrow_mut()
                .replace(Rc::clone(egl.get_connection()));
            let backend = Rc::new(egl);
            Ok(glium::backend::Context::new(
                backend,
                true,
                callback_behavior(),
            )?)
        })
        .or_else(|err| {
            log::warn!("EGL init failed {:?}, fall back to WGL", err);
            super::wgl::GlState::create(self.hwnd.0).and_then(|state| unsafe {
                Ok(glium::backend::Context::new(
                    Rc::new(state),
                    true,
                    callback_behavior(),
                )?)
            })
        })?;

        self.gl_state.replace(gl_state.clone());

        if let Err(err) = self.callbacks.borrow_mut().created(&window, gl_state) {
            self.gl_state.take();
            Err(err)
        } else {
            Ok(())
        }
    }

    /// Check if we need to generate a resize callback.
    /// Calls resize if needed.
    /// Returns true if we did.
    fn check_and_call_resize_if_needed(&mut self) -> bool {
        let mut rect = RECT {
            left: 0,
            bottom: 0,
            right: 0,
            top: 0,
        };
        unsafe {
            GetClientRect(self.hwnd.0, &mut rect);
        }
        let pixel_width = rect_width(&rect) as usize;
        let pixel_height = rect_height(&rect) as usize;

        let current_dims = Dimensions {
            pixel_width,
            pixel_height,
            dpi: unsafe { GetDpiForWindow(self.hwnd.0) as usize },
        };

        let same = self
            .last_size
            .as_ref()
            .map(|&dims| dims == current_dims)
            .unwrap_or(false);
        self.last_size.replace(current_dims);

        if !same {
            let imc = ImmContext::get(self.hwnd.0);
            imc.set_position(0, 0);

            self.callbacks.borrow_mut().resize(current_dims);
        }

        !same
    }
}

impl Window {
    fn from_hwnd(hwnd: HWND) -> Self {
        Self(HWindow(hwnd))
    }

    fn create_window(
        class_name: &str,
        name: &str,
        width: usize,
        height: usize,
        lparam: *const RefCell<WindowInner>,
    ) -> anyhow::Result<HWND> {
        // Jamming this in here; it should really live in the application manifest,
        // but having it here means that we don't have to create a manifest
        unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }

        let class_name = wide_string(class_name);
        let h_inst = unsafe { GetModuleHandleW(null()) };
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW | CS_OWNDC,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_inst,
            // FIXME: this resource is specific to the wezterm build and this should
            // really be made generic for other sorts of windows.
            // The ID is defined in assets/windows/resource.rc
            hIcon: unsafe { LoadIconW(h_inst, MAKEINTRESOURCEW(0x101)) },
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
            bail!("CreateWindowExW: {}", err);
        }

        Ok(hwnd)
    }

    pub fn new_window(
        class_name: &str,
        name: &str,
        width: usize,
        height: usize,
        callbacks: Box<dyn WindowCallbacks>,
    ) -> anyhow::Result<Window> {
        let inner = Rc::new(RefCell::new(WindowInner {
            hwnd: HWindow(null_mut()),
            callbacks: RefCell::new(callbacks),
            gl_state: None,
            vscroll_remainder: 0,
            hscroll_remainder: 0,
            keyboard_info: KeyboardLayoutInfo::new(),
            last_size: None,
            in_size_move: false,
            dead_pending: None,
        }));

        // Careful: `raw` owns a ref to inner, but there is no Drop impl
        let raw = rc_to_pointer(&inner);

        let hwnd = match Self::create_window(class_name, name, width, height, raw) {
            Ok(hwnd) => HWindow(hwnd),
            Err(err) => {
                // Ensure that we drop the extra ref to raw before we return
                drop(unsafe { Rc::from_raw(raw) });
                return Err(err);
            }
        };

        enable_dark_mode(hwnd.0);
        enable_blur_behind(hwnd.0);

        Connection::get()
            .expect("Connection::init was not called")
            .windows
            .borrow_mut()
            .insert(hwnd.clone(), Rc::clone(&inner));

        let window = Window(hwnd);
        inner.borrow_mut().enable_opengl()?;

        Ok(window)
    }
}

fn schedule_show_window(hwnd: HWindow, show: bool) {
    // ShowWindow can call to the window proc and may attempt
    // to lock inner, so we avoid locking it ourselves here
    promise::spawn::spawn(async move {
        unsafe {
            ShowWindow(hwnd.0, if show { SW_NORMAL } else { SW_MINIMIZE });
        }
    })
    .detach();
}

impl WindowOpsMut for WindowInner {
    fn close(&mut self) {
        let hwnd = self.hwnd;
        promise::spawn::spawn(async move {
            unsafe {
                DestroyWindow(hwnd.0);
            }
        })
        .detach();
    }

    fn show(&mut self) {
        schedule_show_window(self.hwnd, true);
    }

    fn hide(&mut self) {
        schedule_show_window(self.hwnd, false);
    }

    fn set_cursor(&mut self, cursor: Option<MouseCursor>) {
        apply_mouse_cursor(cursor);
    }

    fn invalidate(&mut self) {
        unsafe {
            InvalidateRect(self.hwnd.0, null(), 1);
        }
    }

    fn set_inner_size(&mut self, width: usize, height: usize) {
        let (width, height) = adjust_client_to_window_dimensions(width, height);
        let hwnd = self.hwnd;
        promise::spawn::spawn(async move {
            unsafe {
                SetWindowPos(
                    hwnd.0,
                    hwnd.0,
                    0,
                    0,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOZORDER,
                );
                wm_paint(hwnd.0, 0, 0, 0);
            }
        })
        .detach();
    }

    fn set_window_position(&self, coords: ScreenPoint) {
        let hwnd = self.hwnd;

        let mut rect = RECT {
            left: 0,
            bottom: 0,
            right: 0,
            top: 0,
        };
        unsafe {
            GetWindowRect(hwnd.0, &mut rect);

            let origin = client_to_screen(hwnd.0, Point::new(0, 0));
            let delta_x = origin.x as i32 - rect.left;
            let delta_y = origin.y as i32 - rect.top;

            MoveWindow(
                hwnd.0,
                coords.x as i32 - delta_x,
                coords.y as i32 - delta_y,
                rect_width(&rect),
                rect_height(&rect),
                1,
            );
        }
    }

    fn set_title(&mut self, title: &str) {
        let title = wide_string(title);
        unsafe {
            SetWindowTextW(self.hwnd.0, title.as_ptr());
        }
    }

    fn set_text_cursor_position(&mut self, cursor: Rect) {
        let imc = ImmContext::get(self.hwnd.0);
        imc.set_position(cursor.origin.x.max(0) as i32, cursor.origin.y.max(0) as i32);
    }
}

impl WindowOps for Window {
    fn close(&self) -> Future<()> {
        Connection::with_window_inner(self.0, |inner| {
            inner.close();
            Ok(())
        })
    }

    fn show(&self) -> Future<()> {
        schedule_show_window(self.0, true);
        Future::ok(()) // FIXME: this is a lie!
    }

    fn hide(&self) -> Future<()> {
        schedule_show_window(self.0, false);
        Future::ok(()) // FIXME: this is a lie!
    }

    fn set_cursor(&self, cursor: Option<MouseCursor>) -> Future<()> {
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_cursor(cursor);
            Ok(())
        })
    }

    fn invalidate(&self) -> Future<()> {
        Connection::with_window_inner(self.0, |inner| {
            inner.invalidate();
            Ok(())
        })
    }

    fn set_title(&self, title: &str) -> Future<()> {
        let title = title.to_owned();
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_title(&title);
            Ok(())
        })
    }

    fn set_text_cursor_position(&self, cursor: Rect) -> Future<()> {
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_text_cursor_position(cursor);
            Ok(())
        })
    }

    fn set_inner_size(&self, width: usize, height: usize) -> Future<()> {
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_inner_size(width, height);
            Ok(())
        })
    }

    fn set_window_position(&self, coords: ScreenPoint) -> Future<()> {
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_window_position(coords);
            Ok(())
        })
    }

    fn apply<R, F: Send + 'static + FnMut(&mut dyn Any, &dyn WindowOps) -> anyhow::Result<R>>(
        &self,
        mut func: F,
    ) -> promise::Future<R>
    where
        Self: Sized,
        R: Send + 'static,
    {
        Connection::with_window_inner(self.0, move |inner| {
            let window = Window(inner.hwnd);
            func(inner.callbacks.borrow_mut().as_any(), &window)
        })
    }

    fn get_clipboard(&self, _clipboard: Clipboard) -> Future<String> {
        Future::result(
            clipboard_win::get_clipboard_string()
                .map(|s| s.replace("\r\n", "\n"))
                .context("Error getting clipboard"),
        )
    }

    fn set_clipboard(&self, text: String) -> Future<()> {
        Future::result(
            clipboard_win::set_clipboard_string(&text).context("Error setting clipboard"),
        )
    }
}

/// Set up bidirectional pointers:
/// hwnd.USERDATA -> WindowInner
/// WindowInner.hwnd -> hwnd
unsafe fn wm_nccreate(hwnd: HWND, _msg: UINT, _wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    let create: &CREATESTRUCTW = &*(lparam as *const CREATESTRUCTW);
    let inner = rc_from_pointer(create.lpCreateParams);
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as _);
    inner.borrow_mut().hwnd = HWindow(hwnd);

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
        let inner = take_rc_from_pointer(raw);
        let mut inner = inner.borrow_mut();
        inner.callbacks.borrow_mut().destroy();
        inner.hwnd = HWindow(null_mut());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
    }

    None
}

/// "Blur behind" is the old vista term for a cool blurring
/// effect that the DWM could enable.  Subsequent windows
/// versions have removed the blurring.  We use this call
/// to tell DWM that we set proper alpha channel info as
/// a result of rendering our window content.
fn enable_blur_behind(hwnd: HWND) {
    use winapi::shared::minwindef::*;
    use winapi::um::dwmapi::*;

    let bb = DWM_BLURBEHIND {
        dwFlags: DWM_BB_ENABLE,
        fEnable: TRUE,
        hRgnBlur: null_mut(),
        fTransitionOnMaximized: FALSE,
    };

    unsafe {
        DwmEnableBlurBehindWindow(hwnd, &bb);
    }
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

    #[allow(non_snake_case)]
    type WINDOWCOMPOSITIONATTRIB = u32;
    const WCA_USEDARKMODECOLORS: WINDOWCOMPOSITIONATTRIB = 26;

    #[allow(non_snake_case)]
    #[repr(C)]
    pub struct WINDOWCOMPOSITIONATTRIBDATA {
        Attrib: WINDOWCOMPOSITIONATTRIB,
        pvData: PVOID,
        cbData: winapi::shared::basetsd::SIZE_T,
    }

    shared_library!(User32,
        pub fn SetWindowCompositionAttribute(hwnd: HWND, attrib: *mut WINDOWCOMPOSITIONATTRIBDATA) -> BOOL,
    );

    const DWMWA_USE_IMMERSIVE_DARK_MODE: DWORD = 19;
    unsafe {
        SetWindowTheme(
            hwnd as _,
            wide_string("DarkMode_Explorer").as_slice().as_ptr(),
            std::ptr::null_mut(),
        );

        let mut enabled: BOOL = 1;
        DwmSetWindowAttribute(
            hwnd as _,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &enabled as *const _ as *const _,
            std::mem::size_of_val(&enabled) as u32,
        );

        if let Ok(user) = User32::open(std::path::Path::new("user32.dll")) {
            (user.SetWindowCompositionAttribute)(
                hwnd,
                &mut WINDOWCOMPOSITIONATTRIBDATA {
                    Attrib: WCA_USEDARKMODECOLORS,
                    pvData: &mut enabled as *mut _ as _,
                    cbData: std::mem::size_of_val(&enabled) as _,
                },
            );
        }
    }
}

unsafe fn wm_enter_exit_size_move(
    hwnd: HWND,
    msg: UINT,
    _wparam: WPARAM,
    _lparam: LPARAM,
) -> Option<LRESULT> {
    let mut should_size = false;
    if let Some(inner) = rc_from_hwnd(hwnd) {
        let mut inner = inner.borrow_mut();
        inner.in_size_move = msg == WM_ENTERSIZEMOVE;
        should_size = !inner.in_size_move;
    }

    if should_size {
        wm_size(hwnd, 0, 0, 0)?;
    }

    Some(0)
}

/// We handle WM_WINDOWPOSCHANGED and dispatch directly to our wm_size as it
/// is a bit more efficient than letting DefWindowProcW parse this and
/// trigger WM_SIZE.
unsafe fn wm_windowposchanged(
    hwnd: HWND,
    _msg: UINT,
    _wparam: WPARAM,
    _lparam: LPARAM,
) -> Option<LRESULT> {
    // let pos = &*(lparam as *const WINDOWPOS);
    wm_size(hwnd, 0, 0, 0)?;
    Some(0)
}

unsafe fn wm_size(hwnd: HWND, _msg: UINT, _wparam: WPARAM, _lparam: LPARAM) -> Option<LRESULT> {
    let mut should_paint = false;

    if let Some(inner) = rc_from_hwnd(hwnd) {
        let mut inner = inner.borrow_mut();
        should_paint = inner.check_and_call_resize_if_needed();
    }

    if should_paint {
        wm_paint(hwnd, 0, 0, 0)?;
    }

    None
}

unsafe fn wm_set_focus(
    hwnd: HWND,
    _msg: UINT,
    _wparam: WPARAM,
    _lparam: LPARAM,
) -> Option<LRESULT> {
    if let Some(inner) = rc_from_hwnd(hwnd) {
        let inner = inner.borrow();
        inner.callbacks.borrow_mut().focus_change(true);
    }
    None
}

unsafe fn wm_kill_focus(
    hwnd: HWND,
    _msg: UINT,
    _wparam: WPARAM,
    _lparam: LPARAM,
) -> Option<LRESULT> {
    if let Some(inner) = rc_from_hwnd(hwnd) {
        let inner = inner.borrow();
        inner.callbacks.borrow_mut().focus_change(false);
    }
    None
}

unsafe fn wm_paint(hwnd: HWND, _msg: UINT, _wparam: WPARAM, _lparam: LPARAM) -> Option<LRESULT> {
    if let Some(inner) = rc_from_hwnd(hwnd) {
        let inner = inner.borrow();

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

        if let Some(gl_context) = inner.gl_state.as_ref() {
            if gl_context.is_context_lost() {
                log::error!("opengl context was lost; should reinit");
                let _ = inner
                    .callbacks
                    .borrow_mut()
                    .opengl_context_lost(&Window(inner.hwnd));
                return None;
            }

            let mut frame =
                glium::Frame::new(Rc::clone(&gl_context), (width as u32, height as u32));

            inner.callbacks.borrow_mut().paint(&mut frame);
            frame.finish().expect("frame.finish failed");
        }

        EndPaint(hwnd, &mut ps);

        Some(0)
    } else {
        None
    }
}

fn mods_and_buttons(wparam: WPARAM) -> (Modifiers, MouseButtons) {
    let mut modifiers = Modifiers::default();
    let mut buttons = MouseButtons::default();
    if wparam & MK_CONTROL != 0 {
        modifiers |= Modifiers::CTRL;
    }
    if wparam & MK_SHIFT != 0 {
        modifiers |= Modifiers::SHIFT;
    }
    if wparam & MK_LBUTTON != 0 {
        buttons |= MouseButtons::LEFT;
    }
    if wparam & MK_MBUTTON != 0 {
        buttons |= MouseButtons::MIDDLE;
    }
    if wparam & MK_RBUTTON != 0 {
        buttons |= MouseButtons::RIGHT;
    }
    // TODO: XBUTTON1 and XBUTTON2?
    (modifiers, buttons)
}

fn mouse_coords(lparam: LPARAM) -> Point {
    // Take care to get the signedness correct!
    let x = (lparam & 0xffff) as u16 as i16 as isize;
    let y = ((lparam >> 16) & 0xffff) as u16 as i16 as isize;

    Point::new(x, y)
}

fn screen_to_client(hwnd: HWND, point: ScreenPoint) -> Point {
    let mut point = POINT {
        x: point.x.try_into().unwrap(),
        y: point.y.try_into().unwrap(),
    };
    unsafe { ScreenToClient(hwnd, &mut point as *mut _) };
    Point::new(point.x.try_into().unwrap(), point.y.try_into().unwrap())
}

fn client_to_screen(hwnd: HWND, point: Point) -> ScreenPoint {
    let mut point = POINT {
        x: point.x.try_into().unwrap(),
        y: point.y.try_into().unwrap(),
    };
    unsafe { ClientToScreen(hwnd, &mut point as *mut _) };
    ScreenPoint::new(point.x.try_into().unwrap(), point.y.try_into().unwrap())
}

fn apply_mouse_cursor(cursor: Option<MouseCursor>) {
    match cursor {
        None => unsafe {
            SetCursor(null_mut());
        },
        Some(cursor) => unsafe {
            SetCursor(LoadCursorW(
                null_mut(),
                match cursor {
                    MouseCursor::Arrow => IDC_ARROW,
                    MouseCursor::Hand => IDC_HAND,
                    MouseCursor::Text => IDC_IBEAM,
                    MouseCursor::SizeUpDown => IDC_SIZENS,
                    MouseCursor::SizeLeftRight => IDC_SIZEWE,
                },
            ));
        },
    }
}

unsafe fn mouse_button(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    if let Some(inner) = rc_from_hwnd(hwnd) {
        // To support dragging the window, capture when the left
        // button goes down and release when it goes up.
        // Without this, the drag state can be confused when dragging
        // the mouse up outside of the client area.
        if msg == WM_LBUTTONDOWN {
            SetCapture(hwnd);
        } else if msg == WM_LBUTTONUP {
            ReleaseCapture();
        }
        let (modifiers, mouse_buttons) = mods_and_buttons(wparam);
        let coords = mouse_coords(lparam);
        let event = MouseEvent {
            kind: match msg {
                WM_LBUTTONDOWN => MouseEventKind::Press(MousePress::Left),
                WM_LBUTTONUP => MouseEventKind::Release(MousePress::Left),
                WM_RBUTTONDOWN => MouseEventKind::Press(MousePress::Right),
                WM_RBUTTONUP => MouseEventKind::Release(MousePress::Right),
                WM_MBUTTONDOWN => MouseEventKind::Press(MousePress::Middle),
                WM_MBUTTONUP => MouseEventKind::Release(MousePress::Middle),
                _ => return None,
            },
            coords,
            screen_coords: client_to_screen(hwnd, coords),
            mouse_buttons,
            modifiers,
        };
        let inner = inner.borrow();
        inner
            .callbacks
            .borrow_mut()
            .mouse_event(&event, &Window::from_hwnd(hwnd));
        Some(0)
    } else {
        None
    }
}

unsafe fn mouse_move(hwnd: HWND, _msg: UINT, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    if let Some(inner) = rc_from_hwnd(hwnd) {
        let (modifiers, mouse_buttons) = mods_and_buttons(wparam);
        let coords = mouse_coords(lparam);
        let event = MouseEvent {
            kind: MouseEventKind::Move,
            coords,
            screen_coords: client_to_screen(hwnd, coords),
            mouse_buttons,
            modifiers,
        };

        let inner = inner.borrow();
        inner
            .callbacks
            .borrow_mut()
            .mouse_event(&event, &Window::from_hwnd(hwnd));
        Some(0)
    } else {
        None
    }
}

lazy_static! {
    static ref WHEEL_SCROLL_LINES: i16 = read_scroll_speed("WheelScrollLines").unwrap_or(3);
    static ref WHEEL_SCROLL_CHARS: i16 = read_scroll_speed("WheelScrollChars").unwrap_or(3);
}

fn read_scroll_speed(name: &str) -> io::Result<i16> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let desktop = hkcu.open_subkey("Control Panel\\Desktop")?;
    desktop
        .get_value::<String, _>(name)
        .and_then(|v| v.parse().map_err(|_| io::ErrorKind::InvalidData.into()))
}

unsafe fn mouse_wheel(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    if let Some(inner) = rc_from_hwnd(hwnd) {
        let (modifiers, mouse_buttons) = mods_and_buttons(wparam);
        // Wheel events return screen coordinates!
        let coords = mouse_coords(lparam);
        let screen_coords = ScreenPoint::new(coords.x, coords.y);
        let coords = screen_to_client(hwnd, screen_coords);
        let delta = GET_WHEEL_DELTA_WPARAM(wparam);
        let scaled_delta = if msg == WM_MOUSEWHEEL {
            delta * (*WHEEL_SCROLL_LINES)
        } else {
            delta * (*WHEEL_SCROLL_CHARS)
        };
        let mut position = scaled_delta / WHEEL_DELTA;
        let remainder = delta % WHEEL_DELTA;
        let event = MouseEvent {
            kind: if msg == WM_MOUSEHWHEEL {
                let mut inner = inner.borrow_mut();
                inner.hscroll_remainder += remainder;
                position += inner.hscroll_remainder / WHEEL_DELTA;
                inner.hscroll_remainder %= WHEEL_DELTA;
                MouseEventKind::HorzWheel(position)
            } else {
                let mut inner = inner.borrow_mut();
                inner.vscroll_remainder += remainder;
                position += inner.vscroll_remainder / WHEEL_DELTA;
                inner.vscroll_remainder %= WHEEL_DELTA;
                MouseEventKind::VertWheel(position)
            },
            coords,
            screen_coords,
            mouse_buttons,
            modifiers,
        };
        let inner = inner.borrow();
        inner
            .callbacks
            .borrow_mut()
            .mouse_event(&event, &Window::from_hwnd(hwnd));
        Some(0)
    } else {
        None
    }
}

/// Helper for managing the IME Manager
struct ImmContext {
    hwnd: HWND,
    imc: HIMC,
}

impl ImmContext {
    /// Obtain the IMM context; it will be released automatically
    /// when dropped
    pub fn get(hwnd: HWND) -> Self {
        Self {
            hwnd,
            imc: unsafe { ImmGetContext(hwnd) },
        }
    }

    /// Set the position of the IME window relative to the top left
    /// of this window
    pub fn set_position(&self, x: i32, y: i32) {
        let mut cf = COMPOSITIONFORM {
            dwStyle: CFS_POINT,
            ptCurrentPos: POINT { x, y },
            rcArea: RECT {
                bottom: 0,
                left: 0,
                right: 0,
                top: 0,
            },
        };
        unsafe {
            ImmSetCompositionWindow(self.imc, &mut cf);
        }
    }
}

impl Drop for ImmContext {
    fn drop(&mut self) {
        unsafe {
            ImmReleaseContext(self.hwnd, self.imc);
        }
    }
}

unsafe fn ime_composition(
    hwnd: HWND,
    _msg: UINT,
    _wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    if let Some(inner) = rc_from_hwnd(hwnd) {
        let inner = inner.borrow();

        if (lparam as DWORD) & GCS_RESULTSTR == 0 {
            // No finished result; continue with the default
            // processing
            return None;
        }

        let imc = ImmContext::get(hwnd);

        // This returns a size in bytes even though it is for a buffer of u16!
        let byte_size = ImmGetCompositionStringW(imc.imc, GCS_RESULTSTR, std::ptr::null_mut(), 0);
        if byte_size > 0 {
            let word_size = byte_size as usize / 2;
            let mut wide_buf = vec![0u16; word_size];
            ImmGetCompositionStringW(
                imc.imc,
                GCS_RESULTSTR,
                wide_buf.as_mut_ptr() as *mut _,
                byte_size as u32,
            );
            match OsString::from_wide(&wide_buf).into_string() {
                Ok(s) => {
                    let key = KeyEvent {
                        key: KeyCode::Composed(s),
                        raw_key: None,
                        raw_modifiers: Modifiers::NONE,
                        raw_code: None,
                        modifiers: Modifiers::NONE,
                        repeat_count: 1,
                        key_is_down: true,
                    }
                    .normalize_shift();
                    inner
                        .callbacks
                        .borrow_mut()
                        .key_event(&key, &Window::from_hwnd(hwnd));

                    return Some(1);
                }
                Err(_) => eprintln!("cannot represent IME as unicode string!?"),
            };
        }
    }
    None
}

/// Holds information about the current keyboard layout.
/// This is used to determine whether the layout includes
/// an AltGr key or just has a regular Right-Alt key,
/// as well as to build out information about dead keys.
struct KeyboardLayoutInfo {
    layout: HKL,
    has_alt_gr: bool,
    dead_keys: HashMap<(Modifiers, u8), DeadKey>,
}

#[derive(Debug)]
struct DeadKey {
    dead_char: char,
    vk: u8,
    mods: Modifiers,
    map: HashMap<(Modifiers, u8), char>,
}

#[derive(Debug)]
enum ResolvedDeadKey {
    InvalidDeadKey,
    Combined(char),
    InvalidCombination(char),
}

impl KeyboardLayoutInfo {
    pub fn new() -> Self {
        Self {
            layout: std::ptr::null_mut(),
            has_alt_gr: false,
            dead_keys: HashMap::new(),
        }
    }

    unsafe fn clear_key_state() {
        let mut out = [0u16; 16];
        let state = [0u8; 256];
        let scan = MapVirtualKeyW(VK_DECIMAL as _, MAPVK_VK_TO_VSC);
        // keep clocking the state to clear out its effects
        while ToUnicode(
            VK_DECIMAL as _,
            scan,
            state.as_ptr(),
            out.as_mut_ptr(),
            out.len() as i32,
            0,
        ) < 0
        {}
    }

    /// Probe to detect whether an AltGr key is present.
    /// This is done by synthesizing a keyboard state with control and alt
    /// pressed and then testing the virtual key presses.  If we find that
    /// one of these yields a single unicode character output then we assume that
    /// it does have AltGr.
    unsafe fn probe_alt_gr(&mut self) {
        self.has_alt_gr = false;

        let mut state = [0u8; 256];
        state[VK_CONTROL as usize] = 0x80;
        state[VK_MENU as usize] = 0x80;

        for vk in 0..=255u32 {
            if vk == VK_PACKET as _ {
                // Avoid false positives
                continue;
            }

            let mut out = [0u16; 16];
            let ret = ToUnicode(vk, 0, state.as_ptr(), out.as_mut_ptr(), out.len() as i32, 0);
            if ret == 1 {
                self.has_alt_gr = true;
                break;
            }

            if ret == -1 {
                // Dead key.
                // keep clocking the state to clear out its effects
                while ToUnicode(vk, 0, state.as_ptr(), out.as_mut_ptr(), out.len() as i32, 0) < 0 {}
            }
        }
    }

    fn apply_mods(mods: Modifiers, state: &mut [u8; 256]) {
        if mods.contains(Modifiers::SHIFT) {
            state[VK_SHIFT as usize] = 0x80;
        }
        if mods.contains(Modifiers::CTRL) || mods.contains(Modifiers::RIGHT_ALT) {
            state[VK_CONTROL as usize] = 0x80;
        }
        if mods.contains(Modifiers::RIGHT_ALT) || mods.contains(Modifiers::ALT) {
            state[VK_MENU as usize] = 0x80;
        }
    }

    /// Probe the keymap to figure out which keys are dead keys
    unsafe fn probe_dead_keys(&mut self) {
        self.dead_keys.clear();

        let shift_states = [
            Modifiers::NONE,
            Modifiers::SHIFT,
            Modifiers::SHIFT | Modifiers::CTRL,
            Modifiers::ALT,
            Modifiers::RIGHT_ALT, // AltGr
        ];

        for &mods in &shift_states {
            let mut state = [0u8; 256];
            Self::apply_mods(mods, &mut state);

            for vk in 0..=255u32 {
                if vk == VK_PACKET as _ {
                    // Avoid false positives
                    continue;
                }

                let scan = MapVirtualKeyW(vk, MAPVK_VK_TO_VSC);

                Self::clear_key_state();
                let mut out = [0u16; 16];
                let ret = ToUnicode(
                    vk,
                    scan,
                    state.as_ptr(),
                    out.as_mut_ptr(),
                    out.len() as i32,
                    0,
                );

                if ret != -1 {
                    continue;
                }

                // Found a Dead key.
                let dead_char = std::char::from_u32_unchecked(out[0] as u32);

                let mut map = HashMap::new();

                for &smod in &shift_states {
                    let mut second_state = [0u8; 256];
                    Self::apply_mods(smod, &mut second_state);

                    for ik in 0..=255u32 {
                        // Re-initiate the dead key starting state
                        Self::clear_key_state();
                        if ToUnicode(
                            vk,
                            scan,
                            state.as_ptr(),
                            out.as_mut_ptr(),
                            out.len() as i32,
                            0,
                        ) != -1
                        {
                            continue;
                        }

                        let scan = MapVirtualKeyW(ik, MAPVK_VK_TO_VSC);

                        let ret = ToUnicode(
                            ik,
                            scan,
                            second_state.as_ptr(),
                            out.as_mut_ptr(),
                            out.len() as i32,
                            0,
                        );

                        if ret == 1 {
                            // Found a combination
                            let c = std::char::from_u32_unchecked(out[0] as u32);
                            // clock through again to get the base
                            ToUnicode(
                                ik,
                                scan,
                                second_state.as_ptr(),
                                out.as_mut_ptr(),
                                out.len() as i32,
                                0,
                            );
                            let base = std::char::from_u32_unchecked(out[0] as u32);

                            if ((smod == Modifiers::CTRL)
                                || (smod == Modifiers::CTRL | Modifiers::SHIFT))
                                && c == base
                                && (c as u32) < 0x20
                            {
                                continue;
                            }

                            log::trace!(
                                "{:?}: {:?} {:?} + {:?} {:?} -> {:?} base={:?}",
                                dead_char,
                                mods,
                                vk,
                                smod,
                                ik,
                                c,
                                base
                            );

                            map.insert((smod, ik as u8), c);
                        }
                    }
                }

                self.dead_keys.insert(
                    (mods, vk as u8),
                    DeadKey {
                        dead_char,
                        mods,
                        vk: vk as u8,
                        map,
                    },
                );
            }
        }
        Self::clear_key_state();
    }

    unsafe fn update(&mut self) {
        let current_layout = GetKeyboardLayout(0);
        if current_layout == self.layout {
            // Avoid recomputing this if the layout hasn't changed
            return;
        }

        let mut saved_state = [0u8; 256];
        if GetKeyboardState(saved_state.as_mut_ptr()) == 0 {
            return;
        }

        self.probe_alt_gr();
        self.probe_dead_keys();
        log::trace!("dead_keys: {:#x?}", self.dead_keys);

        SetKeyboardState(saved_state.as_mut_ptr());
        self.layout = current_layout;
    }

    pub fn has_alt_gr(&mut self) -> bool {
        unsafe {
            self.update();
        }
        self.has_alt_gr
    }

    pub fn is_dead_key_leader(&mut self, mods: Modifiers, vk: u32) -> Option<char> {
        unsafe {
            self.update();
        }
        if vk <= u8::MAX.into() {
            self.dead_keys
                .get(&(mods, vk as u8))
                .map(|dead| dead.dead_char)
        } else {
            None
        }
    }

    pub fn resolve_dead_key(
        &mut self,
        leader: (Modifiers, u32),
        key: (Modifiers, u32),
    ) -> ResolvedDeadKey {
        unsafe {
            self.update();
        }
        if leader.1 <= u8::MAX.into() && key.1 <= u8::MAX.into() {
            if let Some(dead) = self.dead_keys.get(&(leader.0, leader.1 as u8)) {
                if let Some(c) = dead.map.get(&(key.0, key.1 as u8)).map(|&c| c) {
                    ResolvedDeadKey::Combined(c)
                } else {
                    ResolvedDeadKey::InvalidCombination(dead.dead_char)
                }
            } else {
                ResolvedDeadKey::InvalidDeadKey
            }
        } else {
            ResolvedDeadKey::InvalidDeadKey
        }
    }
}

/// Generate a MSG and call TranslateMessage upon it
unsafe fn translate_message(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) {
    use winapi::um::sysinfoapi::GetTickCount;
    TranslateMessage(&MSG {
        hwnd,
        message: msg,
        wParam: wparam,
        lParam: lparam,
        pt: POINT { x: 0, y: 0 },
        time: GetTickCount(),
    });
}

unsafe fn key(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    if let Some(inner) = rc_from_hwnd(hwnd) {
        let mut inner = inner.borrow_mut();
        let repeat = (lparam & 0xffff) as u16;
        let scan_code = ((lparam >> 16) & 0xff) as u8;
        let releasing = (lparam & (1 << 31)) != 0;
        let ime_active = wparam == VK_PROCESSKEY as _;

        /*
        let alt_pressed = (lparam & (1 << 29)) != 0;
        let is_extended = (lparam & (1 << 24)) != 0;
        let was_down = (lparam & (1 << 30)) != 0;
        let label = match msg {
            WM_CHAR => "WM_CHAR",
            WM_IME_CHAR => "WM_IME_CHAR",
            WM_KEYDOWN => "WM_KEYDOWN",
            WM_KEYUP => "WM_KEYUP",
            WM_SYSKEYUP => "WM_SYSKEYUP",
            WM_SYSKEYDOWN => "WM_SYSKEYDOWN",
            WM_SYSCHAR => "WM_SYSCHAR",
            WM_DEADCHAR => "WM_DEADCHAR",
            _ => "WAT",
        };
        log::error!(
            "{} c=`{}` repeat={} scan={} is_extended={} alt_pressed={} was_down={} \
             releasing={} IME={} dead_pending={:?}",
            label,
            wparam,
            repeat,
            scan_code,
            is_extended,
            alt_pressed,
            was_down,
            releasing,
            ime_active,
            inner.dead_pending,
        );
        */

        if ime_active {
            // If the IME is active, allow Windows to perform default processing
            // to drive it forwards.  It will generate a call to `ime_composition`
            // or `ime_endcomposition` when it completes.

            if msg == WM_KEYDOWN {
                // Explicitly allow the built-in translation to occur for the IME
                translate_message(hwnd, msg, wparam, lparam);
                return Some(0);
            }

            return None;
        }

        if msg == WM_DEADCHAR {
            // Ignore WM_DEADCHAR; we only care about the resultant WM_CHAR
            return Some(0);
        }

        let mut keys = [0u8; 256];
        GetKeyboardState(keys.as_mut_ptr());

        let mut modifiers = Modifiers::default();
        if keys[VK_SHIFT as usize] & 0x80 != 0 {
            modifiers |= Modifiers::SHIFT;
        }

        if inner.keyboard_info.has_alt_gr()
            && ((keys[VK_RMENU as usize] & 0x80 != 0) || (keys[VK_MENU as usize] & 0x80 != 0))
            && (keys[VK_CONTROL as usize] & 0x80 != 0)
        {
            // AltGr is pressed; while AltGr is on the RHS of the keyboard
            // is is not the same thing as right-alt.
            // Windows sets RMENU and CONTROL to indicate AltGr and we
            // have to keep these in the key state in order for ToUnicode
            // to map the key correctly.
            // We set RIGHT_ALT as a hint to ourselves that AltGr is in
            // use (we use regular ALT otherwise) so that our dead key
            // resolution can do the right thing.
            //
            // When running inside a VNC session, VNC emulates the AltGr keypresses
            // by sending plain VK_MENU (rather than RMENU) + VK_CONTROL.
            // For compatibility with that we also treat MENU+CONTROL as equivalent
            // to RMENU+CONTROL even though it is technically a lossy transformation.
            // We only do that when the keyboard layout has AltGr so that we don't
            // screw things up for other keyboard layouts.
            // See issue #392 for some more context.
            modifiers |= Modifiers::RIGHT_ALT;
        } else {
            if keys[VK_CONTROL as usize] & 0x80 != 0 {
                modifiers |= Modifiers::CTRL;
            }
            if keys[VK_MENU as usize] & 0x80 != 0 {
                modifiers |= Modifiers::ALT;
            }
        }
        if keys[VK_LWIN as usize] & 0x80 != 0 || keys[VK_RWIN as usize] & 0x80 != 0 {
            modifiers |= Modifiers::SUPER;
        }

        // If control is pressed, clear that out and remember it in our
        // own set of modifiers.
        // We used to also remove shift from this set, but it impacts
        // handling of eg: ctrl+shift+' (which is equivalent to ctrl+" in a US English
        // layout.
        // The shift normalization is now handled by the normalize_shift() method.
        if modifiers.contains(Modifiers::CTRL) {
            keys[VK_CONTROL as usize] = 0;
            keys[VK_LCONTROL as usize] = 0;
            keys[VK_RCONTROL as usize] = 0;
        }

        let raw_modifiers = modifiers;

        let mut raw = None;

        let key = if msg == WM_IME_CHAR || msg == WM_CHAR {
            // If we were sent a character by the IME, some other apps,
            // or by ourselves via TranslateMessage, then take that
            // value as-is.
            Some(KeyCode::Char(std::char::from_u32_unchecked(wparam as u32)))
        } else {
            // Otherwise we're dealing with a raw key message.
            // ToUnicode has frustrating statefulness so we take care to
            // call it only when we think it will give consistent results.

            if releasing {
                // Don't care about key-up events
                return Some(0);
            }

            // Determine the raw, underlying key event
            raw = match wparam as i32 {
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
                i @ 0x41..=0x5a => Some(KeyCode::Char((i as u8 as char).to_ascii_lowercase())),
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
            };

            let is_modifier_only = raw.as_ref().map(|r| r.is_modifier()).unwrap_or(false);
            if is_modifier_only {
                // If this event is only modifiers then don't ask the system
                // for further resolution, as we don't want ToUnicode to
                // perturb its inscrutable global state
                raw.clone()
            } else {
                // If we think this might be a dead key, process it for ourselves.
                // Our KeyboardLayoutInfo struct probed the layout for the key
                // combinations that start a dead key sequence, as well as those
                // that are valid end states for dead keys, so we can resolve
                // these for ourselves in a couple of quick hash lookups.
                let vk = wparam as u32;

                // If we previously had the start of a dead key...
                let dead = if let Some(leader) = inner.dead_pending.take() {
                    // look to see how the current event resolves it
                    match inner
                        .keyboard_info
                        .resolve_dead_key(leader, (modifiers, vk))
                    {
                        // Valid combination produces a single character
                        ResolvedDeadKey::Combined(c) => Some(KeyCode::Char(c)),
                        ResolvedDeadKey::InvalidCombination(c) => {
                            // An invalid combination results in the deferred
                            // keypress triggering the original key first,
                            // and then we process the current key.

                            // Emit an event for the leader of the failed
                            // dead key combination
                            let key = KeyEvent {
                                key: KeyCode::Char(c),
                                raw_key: None,
                                raw_modifiers: Modifiers::NONE,
                                raw_code: Some(wparam as u32),
                                modifiers,
                                repeat_count: 1,
                                key_is_down: !releasing,
                            }
                            .normalize_shift()
                            .normalize_ctrl();

                            inner
                                .callbacks
                                .borrow_mut()
                                .key_event(&key, &Window::from_hwnd(hwnd));

                            // And then we'll perform normal processing on the
                            // current key press
                            if inner
                                .keyboard_info
                                .is_dead_key_leader(modifiers, vk)
                                .is_some()
                            {
                                // Happens to be the start of its own new
                                // dead key sequence
                                inner.dead_pending.replace((modifiers, vk));
                                return Some(0);
                            }

                            // We don't know; allow normal ToUnicode processing
                            None
                        }

                        // We thought we had a dead key last time around,
                        // but this time it didn't resolve.  Most likely
                        // because the keyboard layout changed in the middle
                        // of the keypress.
                        // We're effectively swallowing the original dead
                        // key event here, but we could potentially re-process
                        // the original and current one here if needed.
                        // Seems like a real edge case.
                        ResolvedDeadKey::InvalidDeadKey => None,
                    }
                } else if let Some(c) = inner.keyboard_info.is_dead_key_leader(modifiers, vk) {
                    // They pressed a dead key.
                    // If they want dead key processing, then record that and
                    // wait for a subsequent keypress.
                    if config().use_dead_keys() {
                        inner.dead_pending.replace((modifiers, vk));
                        return Some(0);
                    }
                    // They don't want dead keys; just return the base character
                    Some(KeyCode::Char(c))
                } else {
                    // Not a dead key as far as we know
                    None
                };

                if dead.is_some() {
                    dead
                } else {
                    // We get here for the various UP (but not DOWN as we shortcircuit
                    // those above) messages.
                    // We perform conversion to unicode for ourselves,
                    // rather than calling TranslateMessage to do it for us,
                    // so that we have tighter control over the key processing.
                    let mut out = [0u16; 16];
                    let res = ToUnicode(
                        wparam as u32,
                        scan_code as u32,
                        keys.as_ptr(),
                        out.as_mut_ptr(),
                        out.len() as i32,
                        0,
                    );

                    match res {
                        1 => {
                            // Remove our AltGr placeholder modifier flag now that the
                            // key press has been expanded.
                            modifiers.remove(Modifiers::RIGHT_ALT);
                            Some(KeyCode::Char(std::char::from_u32_unchecked(out[0] as u32)))
                        }
                        // No mapping, so use our raw info
                        0 => raw.clone(),
                        _ => {
                            // dead key: if our dead key mapping in KeyboardLayoutInfo was
                            // correct, we shouldn't be able to get here as we should have
                            // landed in the dead key case above.
                            // If somehow we do get here, we don't have a valid mapping
                            // as -1 indicates the start of a dead key sequence,
                            // and any other n > 1 indicates an ambiguous expansion.
                            // Either way, indicate that we don't have a valid result.
                            log::error!("unexpected dead key expansion: {:?}", out);
                            KeyboardLayoutInfo::clear_key_state();
                            None
                        }
                    }
                }
            }
        };

        if let Some(key) = key {
            let is_composed = raw != Some(key.clone()) || modifiers != raw_modifiers;
            let key = KeyEvent {
                key,
                raw_key: if is_composed { raw } else { None },
                raw_modifiers,
                raw_code: Some(wparam as u32),
                modifiers,
                repeat_count: repeat,
                key_is_down: !releasing,
            }
            .normalize_shift()
            .normalize_ctrl();

            // Special case for ALT-space to show the system menu, and
            // ALT-F4 to close the window.
            if key.modifiers == Modifiers::ALT
                && (key.key == KeyCode::Char(' ') || key.key == KeyCode::Function(4))
            {
                translate_message(hwnd, msg, wparam, lparam);
                return None;
            }

            let handled = inner
                .callbacks
                .borrow_mut()
                .key_event(&key, &Window::from_hwnd(hwnd));

            if handled {
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
        WM_ENTERSIZEMOVE | WM_EXITSIZEMOVE => wm_enter_exit_size_move(hwnd, msg, wparam, lparam),
        WM_WINDOWPOSCHANGED => wm_windowposchanged(hwnd, msg, wparam, lparam),
        WM_SETFOCUS => wm_set_focus(hwnd, msg, wparam, lparam),
        WM_KILLFOCUS => wm_kill_focus(hwnd, msg, wparam, lparam),
        WM_DEADCHAR | WM_KEYDOWN | WM_KEYUP | WM_SYSCHAR | WM_CHAR | WM_IME_CHAR | WM_SYSKEYUP
        | WM_SYSKEYDOWN => key(hwnd, msg, wparam, lparam),
        WM_IME_COMPOSITION => ime_composition(hwnd, msg, wparam, lparam),
        WM_MOUSEMOVE => mouse_move(hwnd, msg, wparam, lparam),
        WM_MOUSEHWHEEL | WM_MOUSEWHEEL => mouse_wheel(hwnd, msg, wparam, lparam),
        WM_LBUTTONDBLCLK | WM_RBUTTONDBLCLK | WM_MBUTTONDBLCLK | WM_LBUTTONDOWN | WM_LBUTTONUP
        | WM_RBUTTONDOWN | WM_RBUTTONUP | WM_MBUTTONDOWN | WM_MBUTTONUP => {
            mouse_button(hwnd, msg, wparam, lparam)
        }
        WM_CLOSE => {
            if let Some(inner) = rc_from_hwnd(hwnd) {
                let inner = inner.borrow();
                if !inner.callbacks.borrow_mut().can_close() {
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
        Err(e) => {
            log::error!("caught {:?}", e);
            std::process::exit(1)
        }
    }
}
