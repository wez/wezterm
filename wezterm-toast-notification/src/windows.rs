#![cfg(windows)]
use winrt_notification::{Duration, Toast};

pub fn show_notif(
    title: &str,
    message: &str,
    url: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    if url.is_some() {
        return Ok(());
    }

    let title = title.to_owned();
    let message = message.to_owned();

    // We need to be in a different thread from the caller
    // in case we get called in the guts of a windows message
    // loop dispatch and are unable to pump messages
    std::thread::spawn(move || {
        Toast::new("org.wezfurlong.wezterm")
            .title(&title)
            .text1(&message)
            .duration(Duration::Long)
            .show()
            .ok();
    });

    Ok(())
}
