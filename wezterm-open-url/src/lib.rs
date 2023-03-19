#[cfg(not(windows))]
pub fn open_url(url: &str) {
    let url = url.to_string();
    std::thread::spawn(move || {
        let _ = open::that(&url);
    });
}

#[cfg(not(windows))]
pub fn open_with(url: &str, app: &str) {
    open::with_in_background(url, app);
}

#[cfg(windows)]
fn shell_execute(url: String, with: Option<String>) {
    use std::os::windows::ffi::OsStrExt;
    use winapi::um::shellapi::ShellExecuteW;
    /// Convert a rust string to a windows wide string
    fn wide_string(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
    std::thread::spawn(move || {
        let operation = wide_string("open");

        let url = wide_string(&url);
        let with = with.map(|s| wide_string(&s));

        let (app, path) = match with {
            Some(app) => (app.as_ptr(), url.as_ptr()),
            None => (url.as_ptr(), std::ptr::null()),
        };

        unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                app,
                path,
                std::ptr::null(),
                winapi::um::winuser::SW_SHOW,
            );
        }
    });
}

#[cfg(windows)]
pub fn open_url(url: &str) {
    shell_execute(url.to_string(), None);
}

#[cfg(windows)]
pub fn open_with(url: &str, app: &str) {
    shell_execute(url.to_string(), Some(app.to_string()));
}
