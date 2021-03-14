#![cfg(windows)]

use xml::escape::escape_str_pcdata;

#[allow(dead_code)]
mod bindings {
    ::windows::include_bindings!();
}

use bindings::{
    windows::data::xml::dom::XmlDocument, windows::foundation::*, windows::ui::notifications::*,
};
use windows::{Error as WinError, Interface, Object};

fn unwrap_arg<T>(a: &Option<T>) -> Result<&T, WinError> {
    match a {
        Some(t) => Ok(t),
        None => Err(WinError::new(
            ::windows::ErrorCode::E_POINTER,
            "option is none",
        )),
    }
}

fn show_notif_impl(
    title: String,
    message: String,
    url: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let xml = XmlDocument::new()?;

    let url_actions = if url.is_some() {
        r#"
        <actions>
           <action content="Show" arguments="show" />
        </actions>
        "#
    } else {
        ""
    };

    xml.load_xml(format!(
        r#"<toast duration="long">
        <visual>
            <binding template="ToastGeneric">
                <text>{}</text>
                <text>{}</text>
            </binding>
        </visual>
        {}
    </toast>"#,
        escape_str_pcdata(&title),
        escape_str_pcdata(&message),
        url_actions
    ))?;

    let notif = ToastNotification::create_toast_notification(xml)?;

    notif.activated(TypedEventHandler::new(
        move |_: &Option<ToastNotification>, result: &Option<Object>| {
            // let myself = unwrap_arg(myself)?;
            let result = unwrap_arg(result)?.cast::<ToastActivatedEventArgs>()?;

            let args = result.arguments()?;

            if args == "show" {
                if let Some(url) = url.as_ref() {
                    let _ = open::that(url);
                }
            }

            Ok(())
        },
    ))?;

    /*
    notif.dismissed(TypedEventHandler::new(|sender, result| {
        log::info!("dismissed {:?}", result);
        Ok(())
    }))?;

    notif.failed(TypedEventHandler::new(|sender, result| {
        log::warn!("toasts are disabled {:?}", result);
        Ok(())
    }))?;
    */

    let notifier =
        ToastNotificationManager::create_toast_notifier_with_id("org.wezfurlong.wezterm")?;

    notifier.show(&notif)?;

    Ok(())
}

pub fn show_notif(
    title: &str,
    message: &str,
    url: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let title = title.to_owned();
    let message = message.to_owned();
    let url = url.map(|s| s.to_string());

    // We need to be in a different thread from the caller
    // in case we get called in the guts of a windows message
    // loop dispatch and are unable to pump messages
    std::thread::spawn(move || {
        if let Err(err) = show_notif_impl(title, message, url) {
            log::error!("Failed to show toast notification: {:#}", err);
        }
    });

    Ok(())
}
