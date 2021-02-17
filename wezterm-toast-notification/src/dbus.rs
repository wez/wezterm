#![cfg(all(not(target_os = "macos"), not(windows), not(target_os="freebsd")))]
//! See <https://developer.gnome.org/notification-spec/>

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use zbus::dbus_proxy;
use zvariant::derive::Type;
use zvariant::Value;

#[derive(Debug, Type, Serialize, Deserialize)]
pub struct ServerInformation {
    /// The product name of the server.
    pub name: String,

    /// The vendor name. For example "KDE," "GNOME," "freedesktop.org" or "Microsoft".
    pub vendor: String,

    /// The server's version number.
    pub version: String,

    /// The specification version the server is compliant with.
    pub spec_version: String,
}

#[dbus_proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    /// Get server information.
    ///
    /// This message returns the information on the server.
    fn get_server_information(&self) -> zbus::Result<ServerInformation>;

    /// GetCapabilities method
    fn get_capabilities(&self) -> zbus::Result<Vec<String>>;

    /// CloseNotification method
    fn close_notification(&self, nid: u32) -> zbus::Result<()>;

    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, Value>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;

    #[dbus_proxy(signal)]
    fn action_invoked(&self, nid: u32, action_key: String) -> Result<()>;

    #[dbus_proxy(signal)]
    fn notification_closed(&self, nid: u32, reason: u32) -> Result<()>;
}

/// Timeout/expiration was reached
const REASON_EXPIRED: u32 = 1;
/// User dismissed it
const REASON_USER_DISMISSED: u32 = 2;
/// CloseNotification was called with the nid
const REASON_CLOSE_NOTIFICATION: u32 = 3;

#[derive(Debug)]
enum Reason {
    Expired,
    Dismissed,
    Closed,
    Unknown(u32),
}

impl Reason {
    fn new(n: u32) -> Self {
        match n {
            REASON_EXPIRED => Self::Expired,
            REASON_USER_DISMISSED => Self::Dismissed,
            REASON_CLOSE_NOTIFICATION => Self::Closed,
            _ => Self::Unknown(n),
        }
    }
}

pub fn show_notif(title: &str, message: &str, url: Option<&str>) -> Result<(), zbus::Error> {
    let connection = zbus::Connection::new_session()?;

    let proxy = NotificationsProxy::new(&connection)?;
    let caps = proxy.get_capabilities()?;

    if !caps.iter().any(|cap| cap == "actions") {
        // Server doesn't support actions, so skip showing this notification
        // because it might have text that says "click to see more"
        // and that just wouldn't work.
        return Ok(());
    }

    let mut hints = HashMap::new();
    let resident = Value::Bool(true);
    hints.insert("resident", resident);
    let notification = proxy.notify(
        "wezterm",
        0,
        "org.wezfurlong.wezterm",
        title,
        message,
        if url.is_some() {
            &["show", "Show"]
        } else {
            &[]
        },
        hints,
        0, // Never timeout
    )?;

    // If we have a URL, we need to listen for signals to know when/if
    // the user clicks on it.  The thread will stick around until an
    // error is encountered or the user clicks/dismisses the notification.
    if let Some(url) = &url {
        let url = url.to_string();

        struct State {
            notification: u32,
            done: bool,
            url: String,
        }

        let state = Arc::new(Mutex::new(State {
            notification,
            done: false,
            url,
        }));

        proxy.connect_action_invoked({
            let state = Arc::clone(&state);
            move |nid, _action_name| {
                let mut state = state.lock().unwrap();
                if nid == state.notification {
                    let _ = open::that(&state.url);
                    state.done = true;
                }
                Ok(())
            }
        })?;

        proxy.connect_notification_closed({
            let state = Arc::clone(&state);
            move |nid, reason| {
                let _reason = Reason::new(reason);
                let mut state = state.lock().unwrap();
                if nid == state.notification {
                    state.done = true;
                }
                Ok(())
            }
        })?;

        std::thread::spawn(move || {
            while !state.lock().unwrap().done {
                match proxy.next_signal() {
                    Err(err) => {
                        log::error!("next_signal: {:#}", err);
                        break;
                    }
                    Ok(_) => {}
                }
            }
        });
    }

    Ok(())
}
