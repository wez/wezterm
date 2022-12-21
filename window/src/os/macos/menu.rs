use crate::macos::nsstring;
use crate::superclass;
use cocoa::appkit::{NSApp, NSApplication, NSMenu, NSMenuItem};
use cocoa::base::{id, nil, SEL};
use cocoa::foundation::NSInteger;
use config::keyassignment::KeyAssignment;
use objc::declare::ClassDecl;
use objc::rc::StrongPtr;
use objc::runtime::{Class, Object, Sel};
pub use objc::*;
use std::ffi::c_void;

pub struct Menu {
    menu: StrongPtr,
}

impl Menu {
    pub fn new_with_title(title: &str) -> Self {
        unsafe {
            let menu = NSMenu::alloc(nil);
            let menu = StrongPtr::new(menu.initWithTitle_(*nsstring(title)));
            Self { menu }
        }
    }

    pub fn item_at_index(&self, index: usize) -> Option<MenuItem> {
        let index = index as i64;
        let item = unsafe { self.menu.itemAtIndex_(index) };
        if item.is_null() {
            None
        } else {
            Some(MenuItem {
                item: unsafe { StrongPtr::retain(item) },
            })
        }
    }

    pub fn assign_as_main_menu(&self) {
        unsafe {
            let ns_app = NSApp();
            ns_app.setMainMenu_(*self.menu);
        }
    }

    pub fn assign_as_help_menu(&self) {
        unsafe {
            let ns_app = NSApp();
            let () = msg_send![ns_app, setHelpMenu:*self.menu];
        }
    }

    pub fn assign_as_windows_menu(&self) {
        unsafe {
            let ns_app = NSApp();
            ns_app.setWindowsMenu_(*self.menu);
        }
    }

    pub fn assign_as_services_menu(&self) {
        unsafe {
            let ns_app = NSApp();
            ns_app.setServicesMenu_(*self.menu);
        }
    }

    pub fn assign_as_app_menu(&self) {
        unsafe {
            let ns_app = NSApp();
            let () = msg_send![ns_app, performSelector:sel!(setAppleMenu:) withObject:*self.menu];
        }
    }

    pub fn add_item(&self, item: &MenuItem) {
        unsafe {
            self.menu.addItem_(*item.item);
        }
    }

    pub fn item_with_title(&self, title: &str) -> Option<MenuItem> {
        unsafe {
            let item: id = msg_send![*self.menu, itemWithTitle:*nsstring(title)];
            if item.is_null() {
                None
            } else {
                Some(MenuItem {
                    item: StrongPtr::retain(item),
                })
            }
        }
    }

    pub fn get_or_create_sub_menu<F: FnOnce(&Menu)>(&self, title: &str, on_create: F) -> Menu {
        match self.item_with_title(title) {
            Some(m) => m.get_sub_menu().unwrap(),
            None => {
                let item = MenuItem::new_with(title, None, "");
                let menu = Menu::new_with_title(title);
                item.set_sub_menu(&menu);
                self.add_item(&item);
                on_create(&menu);
                menu
            }
        }
    }
}

pub struct MenuItem {
    item: StrongPtr,
}

#[derive(Clone, Debug)]
pub enum RepresentedItem {
    KeyAssignment(KeyAssignment),
}

impl MenuItem {
    pub fn with_menu_item(item: id) -> Self {
        let item = unsafe { StrongPtr::retain(item) };
        Self { item }
    }

    pub fn new_separator() -> Self {
        let item = unsafe { StrongPtr::new(NSMenuItem::separatorItem(nil)) };
        Self { item }
    }

    pub fn new_with(title: &str, action: Option<SEL>, key: &str) -> Self {
        unsafe {
            let item = NSMenuItem::alloc(nil);
            let item = item.initWithTitle_action_keyEquivalent_(
                *nsstring(title),
                action.unwrap_or_else(|| SEL::from_ptr(std::ptr::null())),
                *nsstring(key),
            );

            Self {
                item: StrongPtr::new(item),
            }
        }
    }

    pub fn set_tool_tip(&self, tip: &str) {
        unsafe {
            let () = msg_send![*self.item, setToolTip:*nsstring(tip)];
        }
    }

    pub fn set_target(&self, target: id) {
        unsafe {
            self.item.setTarget_(target);
        }
    }

    pub fn set_sub_menu(&self, menu: &Menu) {
        unsafe {
            self.item.setSubmenu_(*menu.menu);
        }
    }

    pub fn get_sub_menu(&self) -> Option<Menu> {
        unsafe {
            let menu: id = msg_send![*self.item, submenu];
            if menu.is_null() {
                None
            } else {
                Some(Menu {
                    menu: StrongPtr::retain(menu),
                })
            }
        }
    }

    pub fn get_parent_item(&self) -> Option<Self> {
        unsafe {
            let item: id = msg_send![*self.item, parentItem];
            if item.is_null() {
                None
            } else {
                Some(Self {
                    item: StrongPtr::retain(item),
                })
            }
        }
    }

    /// Set an integer tag to identify this item
    pub fn set_tag(&self, tag: NSInteger) {
        unsafe {
            let () = msg_send![*self.item, setTag: tag];
        }
    }

    pub fn get_tag(&self) -> NSInteger {
        unsafe { msg_send![*self.item, tag] }
    }

    /// Associate the item to an object
    fn set_represented_object(&self, object: id) {
        unsafe {
            let () = msg_send![*self.item, setRepresentedObject: object];
        }
    }

    fn get_represented_object(&self) -> Option<StrongPtr> {
        unsafe {
            let object: id = msg_send![*self.item, representedObject];
            if object.is_null() {
                None
            } else {
                Some(StrongPtr::retain(object))
            }
        }
    }

    pub fn set_represented_item(&self, item: RepresentedItem) {
        let wrapper: id = unsafe { msg_send![get_wrapper_class(), alloc] };
        let wrapper = unsafe { StrongPtr::new(wrapper) };
        let item = Box::new(item);
        let item: *const RepresentedItem = Box::into_raw(item);
        let item = item as *const c_void;
        unsafe {
            (**wrapper).set_ivar(WRAPPER_FIELD_NAME, item);
        }
        self.set_represented_object(*wrapper);
    }

    pub fn get_represented_item(&self) -> Option<RepresentedItem> {
        let wrapper = self.get_represented_object()?;
        unsafe {
            let item = (**wrapper).get_ivar::<*const c_void>(WRAPPER_FIELD_NAME);
            let item = (*item) as *const RepresentedItem;
            if item.is_null() {
                None
            } else {
                Some((*item).clone())
            }
        }
    }
}

const WRAPPER_CLS_NAME: &str = "WezTermNSMenuRepresentedItem";
const WRAPPER_FIELD_NAME: &str = "item";
/// Wraps RepresentedItem in an NSObject so that we can associate
/// it with a MenuItem
fn get_wrapper_class() -> &'static Class {
    Class::get(WRAPPER_CLS_NAME).unwrap_or_else(|| {
        let mut cls =
            ClassDecl::new(WRAPPER_CLS_NAME, class!(NSObject)).expect("Unable to register class");

        extern "C" fn dealloc(this: &mut Object, _sel: Sel) {
            unsafe {
                let item = this.get_ivar::<*mut c_void>(WRAPPER_FIELD_NAME);
                let item = (*item) as *mut RepresentedItem;
                let item = Box::from_raw(item);
                drop(item);
                let superclass = superclass(this);
                let () = msg_send![super(this, superclass), dealloc];
            }
        }

        cls.add_ivar::<*mut c_void>(WRAPPER_FIELD_NAME);
        unsafe {
            cls.add_method(sel!(dealloc), dealloc as extern "C" fn(&mut Object, Sel));
        }
        cls.register()
    })
}
