use std::sync::OnceLock;

static VERSION: OnceLock<&'static str> = OnceLock::new();
static TRIPLE: OnceLock<&'static str> = OnceLock::new();

pub fn assign_version_info(version: &'static str, triple: &'static str) {
    VERSION.set(version).unwrap();
    TRIPLE.set(triple).unwrap();
}

pub fn wezterm_version() -> &'static str {
    VERSION
        .get()
        .unwrap_or(&"someone forgot to call assign_version_info")
}

pub fn wezterm_target_triple() -> &'static str {
    TRIPLE
        .get()
        .unwrap_or(&"someone forgot to call assign_version_info")
}

pub fn running_under_wsl() -> bool {
    #[cfg(unix)]
    unsafe {
        let mut name: libc::utsname = std::mem::zeroed();
        if libc::uname(&mut name) == 0 {
            // 'microsoft' is usually in version, in some cases it can be in release instead
            // (see #7136)
            let version = format!(
                "{} {}",
                std::ffi::CStr::from_ptr(name.version.as_ptr()).to_string_lossy(),
                std::ffi::CStr::from_ptr(name.release.as_ptr()).to_string_lossy()
            );
            return version.to_ascii_lowercase().contains("microsoft");
        }
    };

    false
}
