pub fn wezterm_version() -> &'static str {
    // See build.rs
    env!("WEZTERM_CI_TAG")
}

pub fn wezterm_target_triple() -> &'static str {
    // See build.rs
    env!("WEZTERM_TARGET_TRIPLE")
}

pub fn running_under_wsl() -> bool {
    #[cfg(unix)]
    unsafe {
        let mut name: libc::utsname = std::mem::zeroed();
        if libc::uname(&mut name) == 0 {
            let version = std::ffi::CStr::from_ptr(name.version.as_ptr())
                .to_string_lossy()
                .into_owned();
            return version.contains("Microsoft");
        }
    };

    false
}
