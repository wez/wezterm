#![cfg(windows)]

use crate::ToastNotification as TN;
use xml::escape::escape_str_pcdata;

use windows::core::{Error as WinError, IInspectable, Interface, HSTRING};
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::Win32::Foundation::E_POINTER;
use windows::UI::Notifications::{
    ToastActivatedEventArgs, ToastNotification, ToastNotificationManager,
};

fn unwrap_arg<T>(a: &Option<T>) -> Result<&T, WinError> {
    match a {
        Some(t) => Ok(t),
        None => Err(WinError::new(E_POINTER, HSTRING::from("option is none"))),
    }
}

fn show_notif_impl(toast: TN) -> Result<(), Box<dyn std::error::Error>> {
    let xml = XmlDocument::new()?;

    let url_actions = if toast.url.is_some() {
        r#"
        <actions>
           <action content="Show" arguments="show" />
        </actions>
        "#
    } else {
        ""
    };

    xml.LoadXml(HSTRING::from(format!(
        r#"<toast duration="long">
        <visual>
            <binding template="ToastGeneric">
                <text>{}</text>
                <text>{}</text>
            </binding>
        </visual>
        {}
    </toast>"#,
        escape_str_pcdata(&toast.title),
        escape_str_pcdata(&toast.message),
        url_actions
    )))?;

    let notif = ToastNotification::CreateToastNotification(xml)?;

    notif.Activated(TypedEventHandler::new(
        move |_: &Option<ToastNotification>, result: &Option<IInspectable>| {
            // let myself = unwrap_arg(myself)?;
            let result = unwrap_arg(result)?.cast::<ToastActivatedEventArgs>()?;

            let args = result.Arguments()?;

            if args == "show" {
                if let Some(url) = toast.url.as_ref() {
                    wezterm_open_url::open_url(url);
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

    let notifier = ToastNotificationManager::CreateToastNotifierWithId(HSTRING::from(
        "org.wezfurlong.wezterm",
    ))?;

    notifier.Show(&notif)?;

    Ok(())
}

pub fn show_notif(notif: TN) -> Result<(), Box<dyn std::error::Error>> {
    // We need to be in a different thread from the caller
    // in case we get called in the guts of a windows message
    // loop dispatch and are unable to pump messages
    std::thread::spawn(move || {
        if let Err(err) = show_notif_impl(notif) {
            log::error!("Failed to show toast notification: {:#}", err);
        }
    });

    Ok(())
}
