#![cfg(windows)]

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
        use winrt_notification::Toast;

        Toast::new(Toast::POWERSHELL_APP_ID)
            .title(&title)
            .text1(&message)
            .duration(winrt_notification::Duration::Long)
            .show()
            .ok();
    });

    Ok(())
}
