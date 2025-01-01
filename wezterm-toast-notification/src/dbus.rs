#![cfg(all(not(target_os = "macos"), not(windows)))]
//! See <https://developer.gnome.org/notification-spec/>

use crate::ToastNotification;
use futures_util::stream::{abortable, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zbus::proxy;
use zvariant::{Type, Value};

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

#[proxy(
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
        hints: &HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;

    #[zbus(signal)]
    fn action_invoked(&self, nid: u32, action_key: String) -> zbus::Result<()>;

    #[zbus(signal)]
    fn notification_closed(&self, nid: u32, reason: u32) -> zbus::Result<()>;
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
    #[allow(dead_code)]
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

async fn show_notif_impl(notif: ToastNotification) -> Result<(), Box<dyn std::error::Error>> {
    let connection = zbus::ConnectionBuilder::session()?.build().await?;

    let proxy = NotificationsProxy::new(&connection).await?;
    let caps = proxy.get_capabilities().await?;

    if notif.url.is_some() && !caps.iter().any(|cap| cap == "actions") {
        // Server doesn't support actions, so skip showing this notification
        // because it might have text that says "click to see more"
        // and that just wouldn't work.
        return Ok(());
    }

    let mut hints = HashMap::new();
    hints.insert("urgency", Value::U8(2 /* Critical */));
    let notification = proxy
        .notify(
            "wezterm",
            0,
            "org.wezfurlong.wezterm",
            &notif.title,
            &notif.message,
            if notif.url.is_some() {
                &["show", "Show"]
            } else {
                &[]
            },
            &hints,
            notif.timeout.map(|d| d.as_millis() as _).unwrap_or(0),
        )
        .await?;

    let (mut invoked_stream, abort_invoked) = abortable(proxy.receive_action_invoked().await?);
    let (mut closed_stream, abort_closed) = abortable(proxy.receive_notification_closed().await?);

    futures_util::try_join!(
        async {
            while let Some(signal) = invoked_stream.next().await {
                let args = signal.args()?;
                if args.nid == notification {
                    if let Some(url) = notif.url.as_ref() {
                        wezterm_open_url::open_url(url);
                        abort_closed.abort();
                        break;
                    }
                }
            }
            Ok::<(), zbus::Error>(())
        },
        async {
            while let Some(signal) = closed_stream.next().await {
                let args = signal.args()?;
                let _reason = Reason::new(args.reason);
                if args.nid == notification {
                    abort_invoked.abort();
                    break;
                }
            }
            Ok(())
        }
    )?;

    Ok(())
}

pub fn show_notif(notif: ToastNotification) -> Result<(), Box<dyn std::error::Error>> {
    // Run this in a separate thread as we don't know if dbus or the notification
    // service on the other end are up, and we'd otherwise block for some time.
    std::thread::spawn(move || {
        let res = async_io::block_on(async move { show_notif_impl(notif).await });
        if let Err(err) = res {
            log::error!("while showing notification: {:#}", err);
        }
    });
    Ok(())
}
