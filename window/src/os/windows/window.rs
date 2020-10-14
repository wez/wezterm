use super::gdi::*;
use super::*;
use crate::bitmaps::*;
use crate::color::Color;
use crate::connection::ConnectionOps;
use crate::{
    Clipboard, Dimensions, KeyCode, KeyEvent, Modifiers, MouseButtons, MouseCursor, MouseEvent,
    MouseEventKind, MousePress, Operator, PaintContext, Point, Rect, ScreenPoint, WindowCallbacks,
    WindowOps, WindowOpsMut,
};
use anyhow::{bail, Context};
use lazy_static::lazy_static;
use promise::Future;
use std::any::Any;
use std::cell::RefCell;
use std::convert::TryInto;
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
    bitmap: RefCell<GdiBitmap>,
    #[cfg(feature = "opengl")]
    gl_state: Option<Rc<glium::backend::Context>>,
    /// Fraction of mouse scroll
    hscroll_remainder: i16,
    vscroll_remainder: i16,

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

impl WindowInner {
    #[cfg(feature = "opengl")]
    fn enable_opengl(&mut self) -> anyhow::Result<()> {
        let window = Window(self.hwnd);

        let gl_state = super::wgl::GlState::create(self.hwnd.0)
            .map(Rc::new)
            .and_then(|state| unsafe {
                Ok(glium::backend::Context::new(
                    Rc::clone(&state),
                    true,
                    if cfg!(debug_assertions) {
                        glium::debug::DebugCallbackBehavior::DebugMessageOnError
                    } else {
                        glium::debug::DebugCallbackBehavior::Ignore
                    },
                )?)
            });

        self.gl_state = gl_state.as_ref().map(Rc::clone).ok();

        if let Err(err) = self
            .callbacks
            .borrow_mut()
            .opengl_initialize(&window, gl_state)
        {
            self.gl_state.take();
            Err(err)
        } else {
            Ok(())
        }
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
            bitmap: RefCell::new(GdiBitmap::new_empty()),
            #[cfg(feature = "opengl")]
            gl_state: None,
            vscroll_remainder: 0,
            hscroll_remainder: 0,
            keyboard_info: KeyboardLayoutInfo::new(),
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

        Connection::get()
            .expect("Connection::init was not called")
            .windows
            .borrow_mut()
            .insert(hwnd.clone(), Rc::clone(&inner));

        let window = Window(hwnd);
        inner.borrow_mut().callbacks.borrow_mut().created(&window);

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

    #[cfg(feature = "opengl")]
    fn enable_opengl(&self) -> promise::Future<()> {
        Connection::with_window_inner(self.0, move |inner| inner.enable_opengl())
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

struct GdiGraphicsContext<'a> {
    bitmap: &'a mut GdiBitmap,
    dpi: u32,
}

impl<'a> PaintContext for GdiGraphicsContext<'a> {
    fn clear_rect(&mut self, rect: Rect, color: Color) {
        self.bitmap.clear_rect(rect, color)
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

    fn draw_image(
        &mut self,
        dest_top_left: Point,
        src_rect: Option<Rect>,
        im: &dyn BitmapImage,
        operator: Operator,
    ) {
        self.bitmap
            .draw_image(dest_top_left, src_rect, im, operator)
    }

    fn draw_line(&mut self, start: Point, end: Point, color: Color, operator: Operator) {
        self.bitmap.draw_line(start, end, color, operator);
    }
}

unsafe fn wm_size(hwnd: HWND, _msg: UINT, _wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    if let Some(inner) = rc_from_hwnd(hwnd) {
        let inner = inner.borrow();
        let pixel_width = LOWORD(lparam as DWORD) as usize;
        let pixel_height = HIWORD(lparam as DWORD) as usize;

        let imc = ImmContext::get(hwnd);
        imc.set_position(0, 0);

        inner.callbacks.borrow_mut().resize(Dimensions {
            pixel_width,
            pixel_height,
            dpi: GetDpiForWindow(hwnd) as usize,
        });
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

unsafe fn wm_paint(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    if let Some(inner) = rc_from_hwnd(hwnd) {
        let mut inner = inner.borrow_mut();

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

        #[cfg(feature = "opengl")]
        {
            if let Some(gl_context) = inner.gl_state.as_ref() {
                if gl_context.is_context_lost() {
                    log::error!("opengl context was lost; should reinit");

                    drop(inner.gl_state.take());
                    let _ = inner.callbacks.borrow_mut().opengl_initialize(
                        &Window(inner.hwnd),
                        Err(anyhow::anyhow!("opengl context lost")),
                    );

                    let _ = inner
                        .callbacks
                        .borrow_mut()
                        .opengl_context_lost(&Window(inner.hwnd));
                    inner.gl_state.take();
                    drop(inner);
                    return wm_paint(hwnd, msg, wparam, lparam);
                }

                let mut frame =
                    glium::Frame::new(Rc::clone(&gl_context), (width as u32, height as u32));

                inner.callbacks.borrow_mut().paint_opengl(&mut frame);
                frame.finish().expect("frame.finish failed");
                EndPaint(hwnd, &mut ps);
                return Some(0);
            }
        }

        if width > 0 && height > 0 {
            let mut bitmap = inner.bitmap.borrow_mut();
            let (bm_width, bm_height) = bitmap.image_dimensions();
            if bm_width != width || bm_height != height {
                *bitmap = GdiBitmap::new_compatible(width, height, dc).unwrap();
            }
            let dpi = GetDpiForWindow(hwnd);
            let mut context = GdiGraphicsContext {
                dpi,
                bitmap: &mut bitmap,
            };

            inner.callbacks.borrow_mut().paint(&mut context);
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
        let coords = mouse_coords(lparam);
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
            match std::ffi::OsString::from_wide(&wide_buf).into_string() {
                Ok(s) => {
                    let key = KeyEvent {
                        key: KeyCode::Composed(s),
                        raw_key: None,
                        raw_modifiers: Modifiers::NONE,
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
/// an AltGr key or just has a regular Right-Alt key.
struct KeyboardLayoutInfo {
    layout: HKL,
    has_alt_gr: bool,
}

impl KeyboardLayoutInfo {
    pub fn new() -> Self {
        Self {
            layout: std::ptr::null_mut(),
            has_alt_gr: false,
        }
    }

    /// Probe to detect whether an AltGr key is present.
    /// This is done by synthesizing a keyboard state with control and alt
    /// pressed and then testing the virtual key presses.  If we find that
    /// one of these yields a single unicode character output then we assume that
    /// it does have AltGr.
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
                // Dead key; keep clocking the state to clear out its effects
                while ToUnicode(vk, 0, state.as_ptr(), out.as_mut_ptr(), out.len() as i32, 0) < 0 {}
            }
        }

        SetKeyboardState(saved_state.as_mut_ptr());
        self.layout = current_layout;
    }

    pub fn has_alt_gr(&mut self) -> bool {
        unsafe {
            self.update();
        }
        self.has_alt_gr
    }
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
            _ => "WAT",
        };
        eprintln!(
            "{} c=`{}` repeat={} scan={} is_extended={} alt_pressed={} was_down={} releasing={} IME={}",
            label, wparam, repeat, scan_code, is_extended, alt_pressed, was_down, releasing, ime_active
        );
        */

        if ime_active {
            // If the IME is active, allow Windows to perform default processing
            // to drive it forwards.  It will generate a call to `ime_composition`
            // or `ime_endcomposition` when it completes.
            return None;
        }

        let mut keys = [0u8; 256];
        GetKeyboardState(keys.as_mut_ptr());

        let mut modifiers = Modifiers::default();
        if keys[VK_SHIFT as usize] & 0x80 != 0 {
            modifiers |= Modifiers::SHIFT;
        }

        if inner.keyboard_info.has_alt_gr()
            && (keys[VK_RMENU as usize] & 0x80 != 0)
            && (keys[VK_CONTROL as usize] & 0x80 != 0)
        {
            // AltGr is pressed; while AltGr is on the RHS of the keyboard
            // is is not the same thing as right-alt.
            // Windows sets RMENU and CONTROL to indicate AltGr and we
            // have to keep these in the key state in order for ToUnicode
            // to map the key correctly.
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

        let key = if msg == WM_IME_CHAR || msg == WM_CHAR {
            Some(KeyCode::Char(std::char::from_u32_unchecked(wparam as u32)))
        } else {
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
                // dead key
                -1 => None,
                0 => {
                    /*
                    let mapped_vkey = MapVirtualKeyW(scan_code.into(), MAPVK_VSC_TO_VK_EX) as i32;
                    eprintln!("mapped vkey={} vs wparam vkey {}", mapped_vkey, wparam);
                    */
                    // No unicode translation, so map the scan code to a virtual key
                    // code, and from there map it to our KeyCode type
                    match wparam as i32 {
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
                        i @ VK_NUMPAD0..=VK_NUMPAD9 => {
                            Some(KeyCode::Numpad((i - VK_NUMPAD0) as u8))
                        }
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
            }
        };

        if let Some(key) = key {
            let key = KeyEvent {
                key,
                raw_key: None,
                raw_modifiers: Modifiers::NONE,
                modifiers,
                repeat_count: repeat,
                key_is_down: !releasing,
            }
            .normalize_shift();
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
        WM_SIZE => wm_size(hwnd, msg, wparam, lparam),
        WM_SETFOCUS => wm_set_focus(hwnd, msg, wparam, lparam),
        WM_KILLFOCUS => wm_kill_focus(hwnd, msg, wparam, lparam),
        WM_KEYDOWN | WM_CHAR | WM_IME_CHAR | WM_KEYUP | WM_SYSKEYUP | WM_SYSKEYDOWN => {
            key(hwnd, msg, wparam, lparam)
        }
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
