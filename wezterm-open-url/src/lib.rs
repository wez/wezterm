// Portions of this file are derived from code that is
// Copyright © 2015 Sebastian Thiel
// <https://github.com/Byron/open-rs>

#[cfg(not(windows))]
pub fn open_url(url: &str) {
    let url = url.to_string();
    std::thread::spawn(move || {
        #[cfg(target_os = "macos")]
        let candidates: &[&[&str]] = &[&["/usr/bin/open", &url]];

        #[cfg(not(target_os = "macos"))]
        let candidates: &[&[&str]] = &[
            &["xdg-open", &url],
            &["gio", "open", &url] as &[_],
            &["gnome-open", &url],
            &["kde-open", &url],
            &["wslview", &url],
        ];

        for candidate in candidates {
            let mut cmd = std::process::Command::new(candidate[0]);
            cmd.args(&candidate[1..]);

            if let Ok(status) = cmd.status() {
                if status.success() {
                    return;
                }
            }
        }
    });
}

#[cfg(not(windows))]
pub fn open_with(url: &str, app: &str) {
    let url = url.to_string();
    let app = app.to_string();

    std::thread::spawn(move || {
        #[cfg(target_os = "macos")]
        let args: &[&str] = &["/usr/bin/open", "-a", &app, &url];

        #[cfg(not(target_os = "macos"))]
        let args: &[&str] = &[&app, &url];

        let mut cmd = std::process::Command::new(args[0]);
        cmd.args(&args[1..]);

        if let Ok(status) = cmd.status() {
            if status.success() {
                return;
            }
        }
    });
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
