#![cfg(target_os = "macos")]

use crate::ToastNotification;
use cocoa::base::*;
use cocoa::foundation::{NSDictionary, NSString};
use core_foundation::dictionary::CFMutableDictionary;
use core_foundation::string::CFString;
use objc::declare::ClassDecl;
use objc::rc::StrongPtr;
use objc::runtime::{Class, Object, Protocol, Sel};
use objc::*;

const DELEGATE_CLS_NAME: &str = "WezTermNotifDelegate";

struct NotifDelegate {}

impl NotifDelegate {
    fn get_class() -> &'static Class {
        Class::get(DELEGATE_CLS_NAME).unwrap_or_else(Self::define_class)
    }

    fn define_class() -> &'static Class {
        let mut cls = ClassDecl::new(DELEGATE_CLS_NAME, class!(NSObject))
            .expect("Unable to register notif delegate class");

        cls.add_protocol(
            Protocol::get("NSUserNotificationCenterDelegate")
                .expect("failed to get NSUserNotificationCenterDelegate protocol"),
        );

        unsafe {
            cls.add_method(
                sel!(userNotificationCenter:didDismissAlert:),
                Self::did_dismiss_alert as extern "C" fn(&mut Object, Sel, id, id),
            );

            cls.add_method(
                sel!(userNotificationCenter:didDeliverNotification:),
                Self::did_deliver_notif as extern "C" fn(&mut Object, Sel, id, id),
            );

            cls.add_method(
                sel!(userNotificationCenter:didActivateNotification:),
                Self::did_activate_notif as extern "C" fn(&mut Object, Sel, id, id),
            );
        }

        cls.register()
    }

    extern "C" fn did_dismiss_alert(_: &mut Object, _sel: Sel, center: id, notif: id) {
        unsafe {
            let () = msg_send![center, removeDeliveredNotification: notif];
        }
    }

    extern "C" fn did_deliver_notif(_: &mut Object, _sel: Sel, _center: id, _notif: id) {}

    extern "C" fn did_activate_notif(_: &mut Object, _sel: Sel, center: id, notif: id) {
        unsafe {
            let info: *mut Object = msg_send![notif, userInfo];

            // If the notification had an associated URL, open it!
            let url = info.valueForKey_(*nsstring("url"));
            if !url.is_null() {
                let url = std::slice::from_raw_parts(url.UTF8String() as *const u8, url.len());
                let url = String::from_utf8_lossy(url);
                let _ = open::that(&*url);
            }
            let () = msg_send![center, removeDeliveredNotification: notif];
        }
    }

    fn alloc() -> StrongPtr {
        let cls = Self::get_class();
        let d_id: StrongPtr = unsafe { StrongPtr::new(msg_send![cls, new]) };
        d_id
    }
}

/// Convert a rust string to a cocoa string
fn nsstring(s: &str) -> StrongPtr {
    unsafe { StrongPtr::new(NSString::alloc(nil).init_str(s)) }
}

/// A little wrapper to make StrongPtr Send.
/// it's actually fine but isn't implemented on the underlying
/// type.
/// https://github.com/SSheldon/rust-objc/issues/44
struct SendStrongPtr(StrongPtr);
unsafe impl Send for SendStrongPtr {}

struct SendId(id);
unsafe impl Send for SendId {}

pub fn show_notif(toast: ToastNotification) -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let center: id = msg_send![
            class!(NSUserNotificationCenter),
            defaultUserNotificationCenter
        ];

        let notif: id = msg_send![class!(NSUserNotification), alloc];
        let notif: id = msg_send![notif, init];
        let notif = StrongPtr::new(notif);

        let () = msg_send![*notif, setTitle: nsstring(&toast.title)];
        let () = msg_send![*notif, setInformativeText: nsstring(&toast.message)];

        let mut info = CFMutableDictionary::new();
        if let Some(url) = toast.url {
            info.set(CFString::from_static_string("url"), CFString::new(&url));
            let () = msg_send![*notif, setUserInfo: info];
        }

        let delegate = NotifDelegate::alloc();
        let () = msg_send![center, setDelegate: delegate];
        let () = msg_send![center, deliverNotification: *notif];

        if let Some(timeout) = toast.timeout {
            let center = SendId(center);
            let notif = SendStrongPtr(notif);
            // Spawn a thread to wait. This could be more efficient.
            // We cannot simply use performSelector:withObject:afterDelay:
            // because we're not guaranteed to be called from the main
            // thread.  We also don't have access to the executor machinery
            // from the window crate here, so we just do this basic take.
            std::thread::spawn(move || {
                std::thread::sleep(timeout);
                let () = msg_send![center.0, removeDeliveredNotification: *notif.0];
            });
        }
    }

    Ok(())
}
