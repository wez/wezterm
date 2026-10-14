mod dbus;
mod macos;
mod windows;

pub struct ToastNotification {
    pub title: String,
    pub message: String,
    pub url: Option<String>,
    pub timeout: Option<std::time::Duration>,
    /// Called when the user clicks the notification.
    /// Use this to focus the pane/tab that triggered the notification.
    pub on_click: Option<Box<dyn FnOnce() + Send + 'static>>,
}

impl std::fmt::Debug for ToastNotification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToastNotification")
            .field("title", &self.title)
            .field("message", &self.message)
            .field("url", &self.url)
            .field("timeout", &self.timeout)
            .field("on_click", &self.on_click.as_ref().map(|_| "..."))
            .finish()
    }
}

impl ToastNotification {
    pub fn show(self) {
        show(self)
    }
}

#[cfg(windows)]
use crate::windows as backend;
#[cfg(all(not(target_os = "macos"), not(windows)))]
use dbus as backend;
#[cfg(target_os = "macos")]
use macos as backend;

mod nop {
    use super::*;

    #[allow(dead_code)]
    pub fn show_notif(_: ToastNotification) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

pub fn show(notif: ToastNotification) {
    if let Err(err) = backend::show_notif(notif) {
        log::error!("Failed to show notification: {}", err);
    }
}

pub fn persistent_toast_notification_with_click_to_open_url(title: &str, message: &str, url: &str) {
    show(ToastNotification {
        title: title.to_string(),
        message: message.to_string(),
        url: Some(url.to_string()),
        timeout: None,
        on_click: None,
    });
}

pub fn persistent_toast_notification(title: &str, message: &str) {
    show(ToastNotification {
        title: title.to_string(),
        message: message.to_string(),
        url: None,
        timeout: None,
        on_click: None,
    });
}

#[cfg(target_os = "macos")]
pub use macos::initialize as macos_initialize;
