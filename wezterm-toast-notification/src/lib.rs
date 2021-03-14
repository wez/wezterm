mod dbus;
mod macos;
mod windows;

#[cfg(windows)]
use crate::windows as backend;
#[cfg(all(not(target_os = "macos"), not(windows), not(target_os = "freebsd")))]
use dbus as backend;
#[cfg(target_os = "macos")]
use macos as backend;

mod nop {
    #[allow(dead_code)]
    pub fn show_notif(_: &str, _: &str, _: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

#[cfg(target_os = "freebsd")]
use nop as backend;

pub fn persistent_toast_notification_with_click_to_open_url(title: &str, message: &str, url: &str) {
    if let Err(err) = backend::show_notif(title, message, Some(url)) {
        log::error!("Failed to show notification: {}", err);
    }
}

pub fn persistent_toast_notification(title: &str, message: &str) {
    if let Err(err) = backend::show_notif(title, message, None) {
        log::error!("Failed to show notification: {}", err);
    }
}
