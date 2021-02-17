mod dbus;
mod macos;

#[allow(unused_variables)]
pub fn persistent_toast_notification_with_click_to_open_url(title: &str, message: &str, url: &str) {
    #[cfg(target_os = "macos")]
    {
        macos::show_notif(title, message, Some(url));
    }

    #[cfg(all(not(target_os = "macos"), not(windows)))]
    {
        if let Err(err) = dbus::show_notif(title, message, Some(url)) {
            log::error!("Failed to show notification: {}", err);
        }
    }

    // No impl for the other OS's at this time
}

pub fn persistent_toast_notification(title: &str, message: &str) {
    #[cfg(target_os = "macos")]
    {
        macos::show_notif(title, message, None);
    }

    #[cfg(all(not(target_os = "macos"), not(windows)))]
    {
        if let Err(err) = dbus::show_notif(title, message, None) {
            log::error!("Failed to show notification: {}", err);
        }
    }

    #[cfg(windows)]
    {
        let title = title.to_owned();
        let message = message.to_owned();

        // We need to be in a different thread from the caller
        // in case we get called in the guts of a windows message
        // loop dispatch and are unable to pump messages
        std::thread::spawn(move || {
            use winrt_notification::Toast;

            Toast::new(Toast::POWERSHELL_APP_ID)
                .title(&title)
                .text1(&message)
                .duration(winrt_notification::Duration::Long)
                .show()
                .ok();
        });
    }
}
