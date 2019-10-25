use failure::Fallible;
use std::ffi::c_void;

#[allow(non_camel_case_types)]
pub mod ffi {
    // gl_generator emits these weird cyclical and redundant type references;
    // the types appear to have to be in a module and need to reference super,
    // with some of the types specified in both scopes :-/
    pub mod generated {
        pub type khronos_utime_nanoseconds_t = super::khronos_utime_nanoseconds_t;
        pub type khronos_uint64_t = super::khronos_uint64_t;
        pub type khronos_ssize_t = super::khronos_ssize_t;
        pub type EGLNativeDisplayType = super::EGLNativeDisplayType;
        pub type EGLNativePixmapType = super::EGLNativePixmapType;
        pub type EGLNativeWindowType = super::EGLNativeWindowType;
        pub type EGLint = super::EGLint;
        pub type NativeDisplayType = super::EGLNativeDisplayType;
        pub type NativePixmapType = super::EGLNativePixmapType;
        pub type NativeWindowType = super::EGLNativeWindowType;

        include!(concat!(env!("OUT_DIR"), "/egl_bindings.rs"));
    }

    pub use generated::*;

    use std::os::raw;

    pub type EGLint = i32;
    pub type khronos_ssize_t = raw::c_long;
    pub type khronos_utime_nanoseconds_t = khronos_uint64_t;
    pub type khronos_uint64_t = u64;
    pub type EGLNativeDisplayType = *const raw::c_void;
    pub type EGLNativePixmapType = *const raw::c_void;

    #[cfg(target_os = "windows")]
    pub type EGLNativeWindowType = winapi::shared::windef::HWND;
    #[cfg(not(target_os = "windows"))]
    pub type EGLNativeWindowType = *const raw::c_void;
}

struct EglWrapper {
    _lib: libloading::Library,
    egl: ffi::Egl,
}

pub struct GlState {
    egl: EglWrapper,
    display: ffi::types::EGLDisplay,
    surface: ffi::types::EGLSurface,
    context: ffi::types::EGLContext,
}

type GetProcAddressFunc =
    unsafe extern "C" fn(*const std::os::raw::c_char) -> *const std::os::raw::c_void;

impl EglWrapper {
    pub fn load_egl(lib: libloading::Library) -> Fallible<Self> {
        let get_proc_address: libloading::Symbol<GetProcAddressFunc> =
            unsafe { lib.get(b"eglGetProcAddress\0")? };
        let egl = ffi::Egl::load_with(|s: &'static str| {
            let sym_name = std::ffi::CString::new(s).expect("symbol to be cstring compatible");
            if let Ok(sym) = unsafe { lib.get(sym_name.as_bytes_with_nul()) } {
                return *sym;
            }
            unsafe { get_proc_address(sym_name.as_ptr()) }
        });
        Ok(Self { _lib: lib, egl })
    }

    pub fn get_display(
        &self,
        display: Option<ffi::EGLNativeDisplayType>,
    ) -> Fallible<ffi::types::EGLDisplay> {
        let display = unsafe { self.egl.GetDisplay(display.unwrap_or(ffi::DEFAULT_DISPLAY)) };
        if display.is_null() {
            Err(self.error("egl GetDisplay"))
        } else {
            Ok(display)
        }
    }

    pub fn error(&self, context: &str) -> failure::Error {
        let label = match unsafe { self.egl.GetError() } as u32 {
            ffi::NOT_INITIALIZED => "NOT_INITIALIZED".into(),
            ffi::BAD_ACCESS => "BAD_ACCESS".into(),
            ffi::BAD_ALLOC => "BAD_ALLOC".into(),
            ffi::BAD_ATTRIBUTE => "BAD_ATTRIBUTE".into(),
            ffi::BAD_CONTEXT => "BAD_CONTEXT".into(),
            ffi::BAD_CURRENT_SURFACE => "BAD_CURRENT_SURFACE".into(),
            ffi::BAD_DISPLAY => "BAD_DISPLAY".into(),
            ffi::BAD_SURFACE => "BAD_SURFACE".into(),
            ffi::BAD_MATCH => "BAD_MATCH".into(),
            ffi::BAD_PARAMETER => "BAD_PARAMETER".into(),
            ffi::BAD_NATIVE_PIXMAP => "BAD_NATIVE_PIXMAP".into(),
            ffi::BAD_NATIVE_WINDOW => "BAD_NATIVE_WINDOW".into(),
            ffi::CONTEXT_LOST => "CONTEXT_LOST".into(),
            ffi::SUCCESS => "Failed but with error code: SUCCESS".into(),
            err => format!("EGL Error code: {}", err),
        };
        failure::format_err!("{}: {}", context, label)
    }

    pub fn initialize_and_get_version(
        &self,
        display: ffi::types::EGLDisplay,
    ) -> Fallible<(ffi::EGLint, ffi::EGLint)> {
        let mut major = 0;
        let mut minor = 0;
        unsafe {
            if self.egl.Initialize(display, &mut major, &mut minor) != 0 {
                Ok((major, minor))
            } else {
                Err(self.error("egl Initialize"))
            }
        }
    }

    pub fn choose_config(
        &self,
        display: ffi::types::EGLDisplay,
        attributes: &[u32],
    ) -> Fallible<Vec<ffi::types::EGLConfig>> {
        failure::ensure!(
            attributes.len() > 0 && attributes[attributes.len() - 1] == ffi::NONE,
            "attributes list must be terminated with ffi::NONE"
        );

        let mut num_configs = 0;
        if unsafe {
            self.egl
                .GetConfigs(display, std::ptr::null_mut(), 0, &mut num_configs)
        } != 1
        {
            return Err(self.error("egl GetConfigs to count possible number of configurations"));
        }

        let mut configs = vec![std::ptr::null(); num_configs as usize];

        if unsafe {
            self.egl.ChooseConfig(
                display,
                attributes.as_ptr() as *const ffi::EGLint,
                configs.as_mut_ptr(),
                configs.len() as ffi::EGLint,
                &mut num_configs,
            )
        } != 1
        {
            return Err(self.error("egl ChooseConfig to select configurations"));
        }

        configs.resize(num_configs as usize, std::ptr::null());

        Ok(configs)
    }

    pub fn create_window_surface(
        &self,
        display: ffi::types::EGLDisplay,
        config: ffi::types::EGLConfig,
        window: ffi::EGLNativeWindowType,
    ) -> Fallible<ffi::types::EGLSurface> {
        let surface = unsafe {
            self.egl
                .CreateWindowSurface(display, config, window, std::ptr::null())
        };
        if surface.is_null() {
            Err(self.error("EGL CreateWindowSurface"))
        } else {
            Ok(surface)
        }
    }

    pub fn create_context(
        &self,
        display: ffi::types::EGLDisplay,
        config: ffi::types::EGLConfig,
        share_context: ffi::types::EGLContext,
        attributes: &[u32],
    ) -> Fallible<ffi::types::EGLConfig> {
        failure::ensure!(
            attributes.len() > 0 && attributes[attributes.len() - 1] == ffi::NONE,
            "attributes list must be terminated with ffi::NONE"
        );
        let context = unsafe {
            self.egl.CreateContext(
                display,
                config,
                share_context,
                attributes.as_ptr() as *const i32,
            )
        };
        if context.is_null() {
            Err(self.error("EGL CreateContext"))
        } else {
            Ok(context)
        }
    }
}

impl GlState {
    pub fn create(
        display: Option<ffi::EGLNativeDisplayType>,
        window: ffi::EGLNativeWindowType,
    ) -> Fallible<Self> {
        let paths = [
            // While EGL is cross platform, it isn't available on macOS nor is it
            // available on my nvidia based system
            #[cfg(target_os = "windows")]
            "libEGL.dll",
            #[cfg(target_os = "windows")]
            "atioglxx.dll",
            #[cfg(not(target_os = "windows"))]
            "libEGL.so.1",
            #[cfg(not(target_os = "windows"))]
            "libEGL.so",
        ];
        for path in &paths {
            eprintln!("trying {}", path);
            if let Ok(lib) = libloading::Library::new(path) {
                if let Ok(egl) = EglWrapper::load_egl(lib) {
                    let egl_display = egl.get_display(display)?;

                    let (major, minor) = egl.initialize_and_get_version(egl_display)?;
                    eprintln!("initialized EGL version {}.{}", major, minor);

                    let configs = egl.choose_config(
                        egl_display,
                        &[
                            ffi::ALPHA_SIZE,
                            8,
                            ffi::RED_SIZE,
                            8,
                            ffi::GREEN_SIZE,
                            8,
                            ffi::BLUE_SIZE,
                            8,
                            ffi::DEPTH_SIZE,
                            24,
                            ffi::CONFORMANT,
                            ffi::OPENGL_ES3_BIT,
                            ffi::RENDERABLE_TYPE,
                            ffi::OPENGL_ES3_BIT,
                            ffi::SURFACE_TYPE,
                            ffi::WINDOW_BIT | ffi::PBUFFER_BIT | ffi::PIXMAP_BIT,
                            ffi::NONE,
                        ],
                    )?;

                    let first_config = *configs.first().ok_or_else(|| {
                        failure::err_msg("no compatible EGL configuration was found")
                    })?;

                    let surface = egl.create_window_surface(egl_display, first_config, window)?;

                    let context = egl.create_context(
                        egl_display,
                        first_config,
                        std::ptr::null(),
                        &[ffi::CONTEXT_MAJOR_VERSION, 3, ffi::NONE],
                    )?;

                    return Ok(Self {
                        egl,
                        display: egl_display,
                        context,
                        surface,
                    });
                }
            }
        }
        failure::bail!("EGL library not found")
    }
}

unsafe impl glium::backend::Backend for GlState {
    fn swap_buffers(&self) -> Result<(), glium::SwapBuffersError> {
        let res = unsafe { self.egl.egl.SwapBuffers(self.display, self.surface) };
        if res != 1 {
            Err(glium::SwapBuffersError::AlreadySwapped)
        } else {
            Ok(())
        }
    }

    unsafe fn get_proc_address(&self, symbol: &str) -> *const c_void {
        let sym_name = std::ffi::CString::new(symbol).expect("symbol to be cstring compatible");
        std::mem::transmute(self.egl.egl.GetProcAddress(sym_name.as_ptr()))
    }

    fn get_framebuffer_dimensions(&self) -> (u32, u32) {
        let mut width = 0;
        let mut height = 0;

        unsafe {
            self.egl
                .egl
                .QuerySurface(self.display, self.surface, ffi::WIDTH as i32, &mut width);
        }
        unsafe {
            self.egl
                .egl
                .QuerySurface(self.display, self.surface, ffi::HEIGHT as i32, &mut height);
        }
        (width as u32, height as u32)
    }

    fn is_current(&self) -> bool {
        unsafe { self.egl.egl.GetCurrentContext() == self.context }
    }

    unsafe fn make_current(&self) {
        self.egl
            .egl
            .MakeCurrent(self.display, self.surface, self.surface, self.context);
    }
}
