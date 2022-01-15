use super::*;
use crate::connection::ConnectionOps;
use crate::Appearance;
use crate::{
    Clipboard, DeadKeyStatus, Dimensions, Handled, KeyCode, KeyEvent, Modifiers, MouseButtons,
    MouseCursor, MouseEvent, MouseEventKind, MousePress, Point, RawKeyEvent, Rect, ScreenPoint,
    WindowDecorations, WindowEvent, WindowEventSender, WindowOps, WindowState,
};
use anyhow::{bail, Context};
use async_trait::async_trait;
use config::ConfigHandle;
use lazy_static::lazy_static;
use promise::Future;
use raw_window_handle::windows::WindowsHandle;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
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
use wezterm_font::FontConfiguration;
use winapi::shared::minwindef::*;
use winapi::shared::ntdef::*;
use winapi::shared::windef::*;
use winapi::shared::winerror::S_OK;
use winapi::um::imm::*;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::uxtheme::{
    CloseThemeData, GetThemeFont, GetThemeSysFont, OpenThemeData, SetWindowTheme,
};
use winapi::um::wingdi::LOGFONTW;
use winapi::um::winuser::*;
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

const GCS_RESULTSTR: DWORD = 0x800;
const GCS_COMPSTR: DWORD = 0x8;
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
    events: WindowEventSender,
    gl_state: Option<Rc<glium::backend::Context>>,
    /// Fraction of mouse scroll
    hscroll_remainder: i16,
    vscroll_remainder: i16,

    last_size: Option<Dimensions>,
    in_size_move: bool,
    dead_pending: Option<(Modifiers, u32)>,
    saved_placement: Option<WINDOWPLACEMENT>,

    keyboard_info: KeyboardLayoutInfo,
    appearance: Appearance,

    config: ConfigHandle,
}

#[derive(Debug, Clone)]
pub struct Window(HWindow);

fn rect_width(r: &RECT) -> i32 {
    r.right - r.left
}

fn rect_height(r: &RECT) -> i32 {
    r.bottom - r.top
}

fn adjust_client_to_window_dimensions(style: u32, width: usize, height: usize) -> (i32, i32) {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: width as _,
        bottom: height as _,
    };
    unsafe { AdjustWindowRect(&mut rect, style, 0) };

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

unsafe impl HasRawWindowHandle for WindowInner {
    fn raw_window_handle(&self) -> RawWindowHandle {
        RawWindowHandle::Windows(WindowsHandle {
            hwnd: self.hwnd.0 as *mut _,
            ..WindowsHandle::empty()
        })
    }
}

impl WindowInner {
    fn enable_opengl(&mut self) -> anyhow::Result<Rc<glium::backend::Context>> {
        let conn = Connection::get().unwrap();

        let gl_state = if self.config.prefer_egl {
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

        Ok(gl_state)
    }

    /// Check if we need to generate a resize callback.
    /// Calls resize if needed.
    /// Returns true if we did.
    fn check_and_call_resize_if_needed(&mut self) -> bool {
        if self.gl_state.is_none() {
            // Don't cache state or generate resize callbacks until
            // we've set up opengl, otherwise we can miss propagating
            // some state during the initial window setup that results
            // in the window dimensions being out of sync with the dpi
            // when eg: the system display settings are set to 200%
            // scale factor.
            return false;
        }

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

            self.events.dispatch(WindowEvent::Resized {
                dimensions: current_dims,
                window_state: if self.saved_placement.is_some() {
                    WindowState::FULL_SCREEN
                } else {
                    WindowState::default()
                },
                live_resizing: self.in_size_move,
            });
        }

        !same
    }

    fn apply_decoration(&mut self) {
        let hwnd = self.hwnd.0;
        schedule_apply_decoration(hwnd, self.config.window_decorations);
    }
}

fn schedule_apply_decoration(hwnd: HWND, decorations: WindowDecorations) {
    promise::spawn::spawn(async move {
        apply_decoration_immediate(hwnd, decorations);
    })
    .detach();
}

fn apply_decoration_immediate(hwnd: HWND, decorations: WindowDecorations) {
    unsafe {
        let orig_style = GetWindowLongW(hwnd, GWL_STYLE);
        let style = decorations_to_style(decorations);
        let new_style = (orig_style & !(WS_OVERLAPPEDWINDOW as i32)) | style as i32;
        SetWindowLongW(hwnd, GWL_STYLE, new_style);
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
        );
    }
}

fn decorations_to_style(decorations: WindowDecorations) -> u32 {
    if decorations == WindowDecorations::RESIZE {
        WS_THICKFRAME
    } else if decorations == WindowDecorations::TITLE {
        WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX
    } else if decorations == WindowDecorations::NONE {
        WS_POPUP
    } else if decorations == WindowDecorations::TITLE | WindowDecorations::RESIZE {
        WS_OVERLAPPEDWINDOW
    } else {
        WS_OVERLAPPEDWINDOW
    }
}

impl Window {
    fn create_window(
        config: ConfigHandle,
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

        let decorations = config.window_decorations;
        let style = decorations_to_style(decorations);
        let (width, height) = adjust_client_to_window_dimensions(style, width, height);

        let (x, y) = if (style & WS_POPUP) == 0 {
            (CW_USEDEFAULT, CW_USEDEFAULT)
        } else {
            // WS_POPUP windows need to specify the initial position.
            // We pick the middle of the primary monitor

            unsafe {
                let mut mi: MONITORINFO = std::mem::zeroed();
                mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
                GetMonitorInfoW(
                    MonitorFromWindow(std::ptr::null_mut(), MONITOR_DEFAULTTOPRIMARY),
                    &mut mi,
                );

                let mon_width = mi.rcMonitor.right - mi.rcMonitor.left;
                let mon_height = mi.rcMonitor.bottom - mi.rcMonitor.top;

                (
                    mi.rcMonitor.left + (mon_width - width) / 2,
                    mi.rcMonitor.top + (mon_height - height) / 2,
                )
            }
        };

        let name = wide_string(name);
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                name.as_ptr(),
                style,
                x,
                y,
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

        // We have to re-apply the styles otherwise they don't
        // completely stick
        schedule_apply_decoration(hwnd, decorations);

        Ok(hwnd)
    }

    pub async fn new_window<F>(
        class_name: &str,
        name: &str,
        width: usize,
        height: usize,
        config: Option<&ConfigHandle>,
        _font_config: Rc<FontConfiguration>,
        event_handler: F,
    ) -> anyhow::Result<Window>
    where
        F: 'static + FnMut(WindowEvent, &Window),
    {
        let events = WindowEventSender::new(event_handler);

        let config = match config {
            Some(c) => c.clone(),
            None => config::configuration(),
        };
        let appearance = get_appearance();
        let inner = Rc::new(RefCell::new(WindowInner {
            hwnd: HWindow(null_mut()),
            appearance,
            events,
            gl_state: None,
            vscroll_remainder: 0,
            hscroll_remainder: 0,
            keyboard_info: KeyboardLayoutInfo::new(),
            last_size: None,
            in_size_move: false,
            dead_pending: None,
            saved_placement: None,
            config: config.clone(),
        }));

        // Careful: `raw` owns a ref to inner, but there is no Drop impl
        let raw = rc_to_pointer(&inner);

        let hwnd = match Self::create_window(config, class_name, name, width, height, raw) {
            Ok(hwnd) => HWindow(hwnd),
            Err(err) => {
                // Ensure that we drop the extra ref to raw before we return
                drop(unsafe { Rc::from_raw(raw) });
                return Err(err);
            }
        };
        let window_handle = Window(hwnd);
        inner
            .borrow_mut()
            .events
            .assign_window(window_handle.clone());

        apply_theme(hwnd.0);
        enable_blur_behind(hwnd.0);

        Connection::get()
            .expect("Connection::init was not called")
            .windows
            .borrow_mut()
            .insert(hwnd.clone(), Rc::clone(&inner));

        Ok(window_handle)
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

impl WindowInner {
    fn close(&mut self) {
        let hwnd = self.hwnd;
        promise::spawn::spawn(async move {
            unsafe {
                DestroyWindow(hwnd.0);
            }
        })
        .detach();
    }

    fn set_cursor(&mut self, cursor: Option<MouseCursor>) {
        apply_mouse_cursor(cursor);
    }

    fn set_window_position(&self, coords: ScreenPoint) {
        let hwnd = self.hwnd.0;
        promise::spawn::spawn(async move {
            let mut rect = RECT {
                left: 0,
                bottom: 0,
                right: 0,
                top: 0,
            };
            unsafe {
                GetWindowRect(hwnd, &mut rect);

                let origin = client_to_screen(hwnd, Point::new(0, 0));
                let delta_x = origin.x as i32 - rect.left;
                let delta_y = origin.y as i32 - rect.top;

                MoveWindow(
                    hwnd,
                    coords.x as i32 - delta_x,
                    coords.y as i32 - delta_y,
                    rect_width(&rect),
                    rect_height(&rect),
                    1,
                );
            }
        })
        .detach();
    }

    fn set_title(&mut self, title: &str) {
        let title = wide_string(title);
        unsafe {
            SetWindowTextW(self.hwnd.0, title.as_ptr());
        }
    }

    fn set_text_cursor_position(&mut self, cursor: Rect) {
        let imc = ImmContext::get(self.hwnd.0);
        imc.set_position(cursor.origin.x.max(0) as i32, cursor.max_y().max(0) as i32);
    }

    fn config_did_change(&mut self, config: &ConfigHandle) {
        self.config = config.clone();
        self.apply_decoration();
    }

    fn toggle_fullscreen(&mut self) {
        unsafe {
            let hwnd = self.hwnd.0;
            let style = GetWindowLongW(hwnd, GWL_STYLE);
            let config = self.config.clone();
            if let Some(placement) = self.saved_placement.take() {
                promise::spawn::spawn(async move {
                    let style = decorations_to_style(config.window_decorations);
                    SetWindowLongW(hwnd, GWL_STYLE, style as i32);
                    SetWindowPlacement(hwnd, &placement);
                    SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE
                            | SWP_NOSIZE
                            | SWP_NOZORDER
                            | SWP_NOOWNERZORDER
                            | SWP_FRAMECHANGED,
                    );
                })
                .detach();
            } else {
                let mut placement: WINDOWPLACEMENT = std::mem::zeroed();
                GetWindowPlacement(hwnd, &mut placement);

                self.saved_placement.replace(placement);
                promise::spawn::spawn(async move {
                    let mut mi: MONITORINFO = std::mem::zeroed();
                    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
                    GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY), &mut mi);
                    SetWindowLongW(hwnd, GWL_STYLE, style & !(WS_OVERLAPPEDWINDOW as i32));
                    SetWindowPos(
                        hwnd,
                        HWND_TOP,
                        mi.rcMonitor.left,
                        mi.rcMonitor.top,
                        mi.rcMonitor.right - mi.rcMonitor.left,
                        mi.rcMonitor.bottom - mi.rcMonitor.top,
                        SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
                    );
                })
                .detach();
            }
        }
    }
}

unsafe impl HasRawWindowHandle for Window {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let conn = Connection::get().expect("raw_window_handle only callable on main thread");
        let handle = conn.get_window(self.0).expect("window handle invalid!?");

        let inner = handle.borrow();
        inner.raw_window_handle()
    }
}

#[async_trait(?Send)]
impl WindowOps for Window {
    async fn enable_opengl(&self) -> anyhow::Result<Rc<glium::backend::Context>> {
        let window = self.0;
        promise::spawn::spawn(async move {
            if let Some(handle) = Connection::get().unwrap().get_window(window) {
                let mut inner = handle.borrow_mut();
                inner.enable_opengl()
            } else {
                anyhow::bail!("invalid window");
            }
        })
        .await
    }

    fn notify<T: Any + Send + Sync>(&self, t: T)
    where
        Self: Sized,
    {
        Connection::with_window_inner(self.0, move |inner| {
            inner
                .events
                .dispatch(WindowEvent::Notification(Box::new(t)));
            Ok(())
        });
    }

    fn close(&self) {
        Connection::with_window_inner(self.0, |inner| {
            inner.close();
            Ok(())
        });
    }

    fn show(&self) {
        schedule_show_window(self.0, true);
    }

    fn hide(&self) {
        schedule_show_window(self.0, false);
    }

    fn set_cursor(&self, cursor: Option<MouseCursor>) {
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_cursor(cursor);
            Ok(())
        });
    }

    fn invalidate(&self) {
        let hwnd = self.0 .0;
        log::trace!("WindowOps::invalidate calling InvalidateRect");
        unsafe {
            InvalidateRect(hwnd, null(), 0);
        }
    }

    fn set_title(&self, title: &str) {
        let title = title.to_owned();
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_title(&title);
            Ok(())
        });
    }

    fn toggle_fullscreen(&self) {
        Connection::with_window_inner(self.0, move |inner| {
            inner.toggle_fullscreen();
            Ok(())
        });
    }

    fn config_did_change(&self, config: &ConfigHandle) {
        let config = config.clone();
        Connection::with_window_inner(self.0, move |inner| {
            inner.config_did_change(&config);
            Ok(())
        });
    }

    fn set_text_cursor_position(&self, cursor: Rect) {
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_text_cursor_position(cursor);
            Ok(())
        });
    }

    fn set_inner_size(&self, width: usize, height: usize) {
        Connection::with_window_inner(self.0, move |inner| {
            let (width, height) = adjust_client_to_window_dimensions(
                decorations_to_style(inner.config.window_decorations),
                width,
                height,
            );
            let hwnd = inner.hwnd;
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
            Ok(())
        });
    }

    fn set_window_position(&self, coords: ScreenPoint) {
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_window_position(coords);
            Ok(())
        });
    }

    fn get_clipboard(&self, _clipboard: Clipboard) -> Future<String> {
        Future::result(
            clipboard_win::get_clipboard_string()
                .map(|s| s.replace("\r\n", "\n"))
                .context("Error getting clipboard"),
        )
    }

    fn set_clipboard(&self, _clipboard: Clipboard, text: String) {
        clipboard_win::set_clipboard_string(&text).ok();
    }

    fn get_title_font_and_point_size(&self) -> Option<(wezterm_font::parser::ParsedFont, f64)> {
        const TMT_CAPTIONFONT: i32 = 801;
        const HP_HEADERITEM: i32 = 1;
        const HIS_NORMAL: i32 = 1;

        unsafe fn populate_log_font(hwnd: HWND, hdc: HDC) -> Option<LOGFONTW> {
            let mut log_font = LOGFONTW {
                lfHeight: 0,
                lfWidth: 0,
                lfEscapement: 0,
                lfOrientation: 0,
                lfWeight: 0,
                lfItalic: 0,
                lfUnderline: 0,
                lfStrikeOut: 0,
                lfCharSet: 0,
                lfOutPrecision: 0,
                lfClipPrecision: 0,
                lfQuality: 0,
                lfPitchAndFamily: 0,
                lfFaceName: [0u16; 32],
            };
            let theme = OpenThemeData(
                hwnd,
                [
                    'H' as u16, 'E' as u16, 'A' as u16, 'D' as u16, 'E' as u16, 'R' as u16, 0u16,
                ]
                .as_ptr(),
            );
            if !theme.is_null() {
                let res = GetThemeFont(
                    theme,
                    hdc,
                    HP_HEADERITEM,
                    HIS_NORMAL,
                    TMT_CAPTIONFONT,
                    &mut log_font,
                );
                if res == S_OK {
                    CloseThemeData(theme);
                    return Some(log_font);
                }
            }

            let res = GetThemeSysFont(theme, TMT_CAPTIONFONT, &mut log_font);
            if !theme.is_null() {
                CloseThemeData(theme);
            }

            if res == S_OK {
                Some(log_font)
            } else {
                None
            }
        }
        unsafe {
            let hwnd = self.0 .0;
            let hdc = GetDC(hwnd);
            let result = match populate_log_font(hwnd, hdc) {
                Some(lf) => wezterm_font::locator::gdi::parse_log_font(&lf, hdc).ok(),
                None => None,
            };
            ReleaseDC(hwnd, hdc);
            result
        }
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
        inner.events.dispatch(WindowEvent::Destroyed);
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
    use winapi::um::wingdi::*;

    unsafe {
        let region = CreateRectRgn(0, 0, -1, -1);

        let bb = DWM_BLURBEHIND {
            dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
            fEnable: TRUE,
            hRgnBlur: region,
            fTransitionOnMaximized: FALSE,
        };

        DwmEnableBlurBehindWindow(hwnd, &bb);

        DeleteObject(region as _);
    }
}

fn apply_theme(hwnd: HWND) -> Option<LRESULT> {
    // Check for OS app theme, and set window attributes accordingly.
    // Note that the MS terminal app uses the logic found here for this stuff:
    // https://github.com/microsoft/terminal/blob/9b92986b49bed8cc41fde4d6ef080921c41e6d9e/src/interactivity/win32/windowtheme.cpp#L62
    use winapi::um::dwmapi::DwmSetWindowAttribute;

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
        let appearance = get_appearance();
        let theme_string = if appearance == Appearance::Dark {
            "DarkMode_Explorer"
        } else {
            ""
        };

        SetWindowTheme(
            hwnd as _,
            wide_string(theme_string).as_slice().as_ptr(),
            std::ptr::null_mut(),
        );

        let mut enabled: BOOL = if appearance == Appearance::Dark { 1 } else { 0 };
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
        };

        if let Some(inner) = rc_from_hwnd(hwnd) {
            let mut inner = inner.borrow_mut();
            if appearance != inner.appearance {
                inner.appearance = appearance;
                inner
                    .events
                    .dispatch(WindowEvent::AppearanceChanged(appearance));
            }
        }
    }

    None
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
    let mut should_pump = false;

    if let Some(inner) = rc_from_hwnd(hwnd) {
        let mut inner = inner.borrow_mut();
        should_paint = inner.check_and_call_resize_if_needed();
        should_pump = inner.in_size_move;
    }

    if should_paint {
        wm_paint(hwnd, 0, 0, 0)?;
        if should_pump {
            crate::spawn::SPAWN_QUEUE.run();
        }
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
        inner
            .borrow_mut()
            .events
            .dispatch(WindowEvent::FocusChanged(true));
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
        inner
            .borrow_mut()
            .events
            .dispatch(WindowEvent::FocusChanged(false));
    }
    None
}

unsafe fn wm_paint(hwnd: HWND, _msg: UINT, _wparam: WPARAM, _lparam: LPARAM) -> Option<LRESULT> {
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
        let _ = BeginPaint(hwnd, &mut ps);
        // Do nothing right now
        EndPaint(hwnd, &mut ps);

        // Ask the app to repaint in a bit
        inner.events.dispatch(WindowEvent::NeedRepaint);

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
        inner
            .borrow_mut()
            .events
            .dispatch(WindowEvent::MouseEvent(event));
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

        inner
            .borrow_mut()
            .events
            .dispatch(WindowEvent::MouseEvent(event));
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
                log::trace!(
                    "mouse_hwheel delta={} scaled={} remainder={} pos={}",
                    delta,
                    scaled_delta,
                    inner.hscroll_remainder,
                    position
                );
                MouseEventKind::HorzWheel(position)
            } else {
                let mut inner = inner.borrow_mut();
                inner.vscroll_remainder += remainder;
                position += inner.vscroll_remainder / WHEEL_DELTA;
                inner.vscroll_remainder %= WHEEL_DELTA;
                log::trace!(
                    "mouse_wheel delta={} scaled={} remainder={} pos={}",
                    delta,
                    scaled_delta,
                    inner.vscroll_remainder,
                    position
                );
                MouseEventKind::VertWheel(position)
            },
            coords,
            screen_coords,
            mouse_buttons,
            modifiers,
        };
        inner
            .borrow_mut()
            .events
            .dispatch(WindowEvent::MouseEvent(event));
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

    pub fn get_str(&self, which: DWORD) -> Result<String, OsString> {
        // This returns a size in bytes even though it is for a buffer of u16!
        let byte_size =
            unsafe { ImmGetCompositionStringW(self.imc, which, std::ptr::null_mut(), 0) };
        if byte_size > 0 {
            let word_size = byte_size as usize / 2;
            let mut wide_buf = vec![0u16; word_size];
            unsafe {
                ImmGetCompositionStringW(
                    self.imc,
                    which,
                    wide_buf.as_mut_ptr() as *mut _,
                    byte_size as u32,
                )
            };
            OsString::from_wide(&wide_buf).into_string()
        } else {
            Ok(String::new())
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
        let mut inner = inner.borrow_mut();
        let imc = ImmContext::get(hwnd);

        let lparam = lparam as DWORD;

        if lparam == 0 {
            // IME was cancelled
            inner
                .events
                .dispatch(WindowEvent::AdviseDeadKeyStatus(DeadKeyStatus::None));
            return Some(1);
        }

        if lparam & GCS_RESULTSTR == 0 {
            // No finished result; continue with the default
            // processing
            if let Ok(composing) = imc.get_str(GCS_COMPSTR) {
                inner
                    .events
                    .dispatch(WindowEvent::AdviseDeadKeyStatus(DeadKeyStatus::Composing(
                        composing,
                    )));
            }
            // We will show the composing string ourselves.
            // Suppress the default composition display.
            return Some(1);
        }

        match imc.get_str(GCS_RESULTSTR) {
            Ok(s) if !s.is_empty() => {
                let key = KeyEvent {
                    key: KeyCode::Composed(s),
                    modifiers: Modifiers::NONE,
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                };
                inner
                    .events
                    .dispatch(WindowEvent::AdviseDeadKeyStatus(DeadKeyStatus::None));
                inner.events.dispatch(WindowEvent::KeyEvent(key));

                return Some(1);
            }
            Ok(_) => {}
            Err(_) => eprintln!("cannot represent IME as unicode string!?"),
        };
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
    _vk: u8,
    _mods: Modifiers,
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
            if vk == VK_PACKET as u32 {
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
                if vk == VK_PACKET as u32 {
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
                        _mods: mods,
                        _vk: vk as u8,
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
        let ime_active = wparam == VK_PROCESSKEY as WPARAM;
        let phys_code = super::keycodes::vkey_to_phys(wparam);

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
        log::trace!(
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
            && (keys[VK_RMENU as usize] & 0x80 != 0)
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
            modifiers |= Modifiers::RIGHT_ALT;
        } else if inner.keyboard_info.has_alt_gr()
            && inner.config.treat_left_ctrlalt_as_altgr
            && (keys[VK_MENU as usize] & 0x80 != 0)
            && (keys[VK_CONTROL as usize] & 0x80 != 0)
        {
            // When running inside a VNC session, VNC emulates the AltGr keypresses
            // by sending plain VK_MENU (rather than VK_RMENU) + VK_CONTROL.
            // For compatibility with that the option `treat_left_ctrlalt_as_altgr` allows
            // to treat MENU+CONTROL as equivalent to RMENU+CONTROL (AltGr) even though it is
            // technically a lossy transformation.
            //
            // We only do that when the keyboard layout has AltGr and the option is enabled,
            // so that we don't screw things up by default or for other keyboard layouts.
            // See issue #392 & #472 for some more context.
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

        let handled_raw = Handled::new();
        let raw_key_event = RawKeyEvent {
            key: match phys_code {
                Some(phys) => KeyCode::Physical(phys),
                None => KeyCode::RawCode(wparam as _),
            },
            phys_code,
            raw_code: wparam as _,
            scan_code: scan_code as _,
            modifiers,
            repeat_count: 1,
            key_is_down: !releasing,
            handled: handled_raw.clone(),
        };

        let key = if msg == WM_IME_CHAR || msg == WM_CHAR {
            // If we were sent a character by the IME, some other apps,
            // or by ourselves via TranslateMessage, then take that
            // value as-is.
            Some(KeyCode::Char(std::char::from_u32_unchecked(wparam as u32)))
        } else {
            // Otherwise we're dealing with a raw key message.
            // ToUnicode has frustrating statefulness so we take care to
            // call it only when we think it will give consistent results.

            inner
                .events
                .dispatch(WindowEvent::RawKeyEvent(raw_key_event.clone()));
            if handled_raw.is_handled() {
                // Cancel any pending dead key
                if inner.dead_pending.take().is_some() {
                    inner
                        .events
                        .dispatch(WindowEvent::AdviseDeadKeyStatus(DeadKeyStatus::None));
                }
                log::trace!("raw key was handled; not processing further");
                return Some(0);
            }

            let is_modifier_only = phys_code.map(|p| p.is_modifier()).unwrap_or(false);
            if is_modifier_only {
                // If this event is only modifiers then don't ask the system
                // for further resolution, as we don't want ToUnicode to
                // perturb its inscrutable global state.
                phys_code.map(|p| p.to_key_code())
            } else {
                // If we think this might be a dead key, process it for ourselves.
                // Our KeyboardLayoutInfo struct probed the layout for the key
                // combinations that start a dead key sequence, as well as those
                // that are valid end states for dead keys, so we can resolve
                // these for ourselves in a couple of quick hash lookups.
                let vk = wparam as u32;

                if releasing && inner.dead_pending.is_some() {
                    // Don't care about key-up events while processing dead keys
                    return Some(0);
                }

                // If we previously had the start of a dead key...
                let dead = if let Some(leader) = inner.dead_pending.take() {
                    inner
                        .events
                        .dispatch(WindowEvent::AdviseDeadKeyStatus(DeadKeyStatus::None));
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
                                modifiers,
                                repeat_count: 1,
                                key_is_down: !releasing,
                                raw: None,
                            }
                            .normalize_shift()
                            .normalize_ctrl();

                            inner.events.dispatch(WindowEvent::KeyEvent(key));

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
                    if releasing {
                        // Don't care about key-up events while processing dead keys
                        return Some(0);
                    }

                    // They pressed a dead key.
                    // If they want dead key processing, then record that and
                    // wait for a subsequent keypress.
                    if inner.config.use_dead_keys {
                        inner.dead_pending.replace((modifiers, vk));
                        inner.events.dispatch(WindowEvent::AdviseDeadKeyStatus(
                            DeadKeyStatus::Composing(c.to_string()),
                        ));
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
                        0 => {
                            log::trace!(
                                "ToUnicode had no mapping for {:?} wparam={}",
                                phys_code,
                                wparam
                            );
                            phys_code.map(|p| p.to_key_code())
                        }
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
            // FIXME: verify this behavior: Urgh, special case for ctrl and non-latin layouts.
            // In order to avoid a situation like #678, if CTRL is the only
            // modifier and we've got composed text, then discard the composed
            // text.
            let key = KeyEvent {
                key,
                modifiers,
                repeat_count: repeat,
                key_is_down: !releasing,
                raw: Some(raw_key_event),
            }
            .normalize_shift();

            // Special case for ALT-space to show the system menu, and
            // ALT-F4 to close the window.
            if key.modifiers == Modifiers::ALT
                && (key.key == KeyCode::Char(' ') || key.key == KeyCode::Function(4))
            {
                translate_message(hwnd, msg, wparam, lparam);
                return None;
            }

            inner.events.dispatch(WindowEvent::KeyEvent(key));
            return Some(0);
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
        WM_SIZING => {
            // Allow events to be processed during live resize
            crate::spawn::SPAWN_QUEUE.run();
            None
        }
        WM_SETTINGCHANGE => apply_theme(hwnd),
        WM_IME_COMPOSITION => ime_composition(hwnd, msg, wparam, lparam),
        WM_MOUSEMOVE => mouse_move(hwnd, msg, wparam, lparam),
        WM_MOUSEHWHEEL | WM_MOUSEWHEEL => mouse_wheel(hwnd, msg, wparam, lparam),
        WM_LBUTTONDBLCLK | WM_RBUTTONDBLCLK | WM_MBUTTONDBLCLK | WM_LBUTTONDOWN | WM_LBUTTONUP
        | WM_RBUTTONDOWN | WM_RBUTTONUP | WM_MBUTTONDOWN | WM_MBUTTONUP => {
            mouse_button(hwnd, msg, wparam, lparam)
        }
        WM_ERASEBKGND => Some(1),
        WM_CLOSE => {
            if let Some(inner) = rc_from_hwnd(hwnd) {
                let mut inner = inner.borrow_mut();
                inner.events.dispatch(WindowEvent::CloseRequested);
                // Don't let it close
                return Some(0);
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
