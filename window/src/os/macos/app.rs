use crate::connection::ConnectionOps;
use crate::macos::menu::RepresentedItem;
use crate::macos::{nsstring, nsstring_to_str};
use crate::{ApplicationEvent, Connection};
use cocoa::appkit::{NSApp, NSApplicationTerminateReply};
use cocoa::base::id;
use objc::declare::ClassDecl;
use objc::rc::StrongPtr;
use objc::runtime::{Class, Object, Sel};
use objc::*;

const CLS_NAME: &str = "WezTermAppDelegate";

#[allow(unused)]
#[link(name = "AppKit", kind = "framework")]
extern "C" {
    pub static NSAboutPanelOptionCredits: id;
    pub static NSAboutPanelOptionApplicationName: id;
    pub static NSAboutPanelOptionApplicationIcon: id;
    pub static NSAboutPanelOptionVersion: id;
    pub static NSAboutPanelOptionApplicationVersion: id;
}

extern "C" fn application_should_terminate(
    _self: &mut Object,
    _sel: Sel,
    _app: *mut Object,
) -> u64 {
    log::debug!("application termination requested");
    NSApplicationTerminateReply::NSTerminateLater as u64
}

extern "C" fn application_will_finish_launching(
    _self: &mut Object,
    _sel: Sel,
    _notif: *mut Object,
) {
    log::debug!("application_will_finish_launching");
}

extern "C" fn wezterm_perform_key_assignment(
    _self: &mut Object,
    _sel: Sel,
    menu_item: *mut Object,
) {
    let menu_item = crate::os::macos::menu::MenuItem::with_menu_item(menu_item);
    // Safe because weztermPerformKeyAssignment: is only used with KeyAssignment
    let action = menu_item.get_represented_item();
    log::debug!("wezterm_perform_key_assignment {action:?}",);
    match action {
        Some(RepresentedItem::KeyAssignment(action)) => {
            if let Some(conn) = Connection::get() {
                conn.dispatch_app_event(ApplicationEvent::PerformKeyAssignment(action));
            }
        }
        None => {}
    }
}

/// Show an about dialog with the version information
extern "C" fn wezterm_show_about(_self: &mut Object, _sel: Sel, _menu_item: *mut Object) {
    unsafe {
        let ns_app = NSApp();

        let credits = nsstring("Copyright (c) 2018-Present Wez Furlong");
        let credits = {
            let attr: id = msg_send![class!(NSAttributedString), alloc];
            let () = msg_send![attr, initWithString:*credits];
            attr
        };
        let version = nsstring(config::wezterm_version());

        let dict: id = msg_send![class!(NSMutableDictionary), alloc];
        let dict: id = msg_send![dict, init];
        let () = msg_send![dict, setObject:*version forKey:NSAboutPanelOptionVersion];
        let () = msg_send![dict, setObject:*version forKey:NSAboutPanelOptionApplicationVersion];
        let () = msg_send![dict, setObject:credits forKey:NSAboutPanelOptionCredits];

        let () = msg_send![ns_app, orderFrontStandardAboutPanelWithOptions: dict];
    }
}

extern "C" fn application_open_file(
    _self: &mut Object,
    _sel: Sel,
    _app: *mut Object,
    file_name: *mut Object,
) {
    let file_name = unsafe { nsstring_to_str(file_name) }.to_string();
    if let Some(conn) = Connection::get() {
        conn.dispatch_app_event(ApplicationEvent::OpenCommandScript(file_name));
    }
}

fn get_class() -> &'static Class {
    Class::get(CLS_NAME).unwrap_or_else(|| {
        let mut cls = ClassDecl::new(CLS_NAME, class!(NSWindow))
            .expect("Unable to register application class");

        unsafe {
            cls.add_method(
                sel!(applicationShouldTerminate:),
                application_should_terminate as extern "C" fn(&mut Object, Sel, *mut Object) -> u64,
            );
            cls.add_method(
                sel!(applicationWillFinishLaunching:),
                application_will_finish_launching as extern "C" fn(&mut Object, Sel, *mut Object),
            );
            cls.add_method(
                sel!(application:openFile:),
                application_open_file as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object),
            );
            cls.add_method(
                sel!(weztermPerformKeyAssignment:),
                wezterm_perform_key_assignment as extern "C" fn(&mut Object, Sel, *mut Object),
            );
            cls.add_method(
                sel!(weztermShowAbout:),
                wezterm_show_about as extern "C" fn(&mut Object, Sel, *mut Object),
            );
        }

        cls.register()
    })
}

pub fn create_app_delegate() -> StrongPtr {
    let cls = get_class();
    unsafe {
        let delegate: *mut Object = msg_send![cls, alloc];
        let delegate: *mut Object = msg_send![delegate, init];
        StrongPtr::new(delegate)
    }
}
