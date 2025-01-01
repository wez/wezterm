use crate::connection::ConnectionOps;
use crate::macos::menu::RepresentedItem;
use crate::macos::{nsstring, nsstring_to_str};
use crate::menu::{Menu, MenuItem};
use crate::{ApplicationEvent, Connection};
use cocoa::appkit::NSApplicationTerminateReply;
use cocoa::base::id;
use cocoa::foundation::NSInteger;
use config::keyassignment::KeyAssignment;
use config::WindowCloseConfirmation;
use objc::declare::ClassDecl;
use objc::rc::StrongPtr;
use objc::runtime::{Class, Object, Sel, BOOL, NO, YES};
use objc::*;

const CLS_NAME: &str = "WezTermAppDelegate";

extern "C" fn application_should_terminate(
    _self: &mut Object,
    _sel: Sel,
    _app: *mut Object,
) -> u64 {
    log::debug!("application termination requested");
    unsafe {
        match config::configuration().window_close_confirmation {
            WindowCloseConfirmation::NeverPrompt => terminate_now(),
            WindowCloseConfirmation::AlwaysPrompt => {
                let alert: id = msg_send![class!(NSAlert), alloc];
                let alert: id = msg_send![alert, init];
                let message_text = nsstring("Terminate WezTerm?");
                let info_text = nsstring("Detach and close all panes and terminate wezterm?");
                let cancel = nsstring("Cancel");
                let ok = nsstring("Ok");

                let () = msg_send![alert, setMessageText: message_text];
                let () = msg_send![alert, setInformativeText: info_text];
                let () = msg_send![alert, addButtonWithTitle: cancel];
                let () = msg_send![alert, addButtonWithTitle: ok];
                #[allow(non_upper_case_globals)]
                const NSModalResponseCancel: NSInteger = 1000;
                #[allow(non_upper_case_globals, dead_code)]
                const NSModalResponseOK: NSInteger = 1001;
                let result: NSInteger = msg_send![alert, runModal];
                log::info!("alert result is {result}");

                if result == NSModalResponseCancel {
                    NSApplicationTerminateReply::NSTerminateCancel as u64
                } else {
                    terminate_now()
                }
            }
        }
    }
}

fn terminate_now() -> u64 {
    if let Some(conn) = Connection::get() {
        conn.terminate_message_loop();
    }
    NSApplicationTerminateReply::NSTerminateNow as u64
}

extern "C" fn application_will_finish_launching(
    _self: &mut Object,
    _sel: Sel,
    _notif: *mut Object,
) {
    log::debug!("application_will_finish_launching");
}

extern "C" fn application_did_finish_launching(this: &mut Object, _sel: Sel, _notif: *mut Object) {
    log::debug!("application_did_finish_launching");
    unsafe {
        (*this).set_ivar("launched", YES);
    }
}

extern "C" fn application_open_untitled_file(
    this: &mut Object,
    _sel: Sel,
    _app: *mut Object,
) -> BOOL {
    let launched: BOOL = unsafe { *this.get_ivar("launched") };
    log::debug!("application_open_untitled_file launched={launched}");
    if let Some(conn) = Connection::get() {
        if launched == YES {
            conn.dispatch_app_event(ApplicationEvent::PerformKeyAssignment(
                KeyAssignment::SpawnWindow,
            ));
        }
        return YES;
    }
    NO
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

extern "C" fn application_open_file(
    this: &mut Object,
    _sel: Sel,
    _app: *mut Object,
    file_name: *mut Object,
) {
    let launched: BOOL = unsafe { *this.get_ivar("launched") };
    if launched == YES {
        let file_name = unsafe { nsstring_to_str(file_name) }.to_string();
        if let Some(conn) = Connection::get() {
            log::debug!("application_open_file {file_name}");
            conn.dispatch_app_event(ApplicationEvent::OpenCommandScript(file_name));
        }
    }
}

extern "C" fn application_dock_menu(
    _self: &mut Object,
    _sel: Sel,
    _app: *mut Object,
) -> *mut Object {
    let dock_menu = Menu::new_with_title("");
    let new_window_item =
        MenuItem::new_with("New Window", Some(sel!(weztermPerformKeyAssignment:)), "");
    new_window_item
        .set_represented_item(RepresentedItem::KeyAssignment(KeyAssignment::SpawnWindow));
    dock_menu.add_item(&new_window_item);
    dock_menu.autorelease()
}

fn get_class() -> &'static Class {
    Class::get(CLS_NAME).unwrap_or_else(|| {
        let mut cls = ClassDecl::new(CLS_NAME, class!(NSWindow))
            .expect("Unable to register application class");

        cls.add_ivar::<BOOL>("launched");

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
                sel!(applicationDidFinishLaunching:),
                application_did_finish_launching as extern "C" fn(&mut Object, Sel, *mut Object),
            );
            cls.add_method(
                sel!(application:openFile:),
                application_open_file as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object),
            );
            cls.add_method(
                sel!(applicationDockMenu:),
                application_dock_menu
                    as extern "C" fn(&mut Object, Sel, *mut Object) -> *mut Object,
            );
            cls.add_method(
                sel!(weztermPerformKeyAssignment:),
                wezterm_perform_key_assignment as extern "C" fn(&mut Object, Sel, *mut Object),
            );
            cls.add_method(
                sel!(applicationOpenUntitledFile:),
                application_open_untitled_file
                    as extern "C" fn(&mut Object, Sel, *mut Object) -> BOOL,
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
        (*delegate).set_ivar("launched", NO);
        StrongPtr::new(delegate)
    }
}
