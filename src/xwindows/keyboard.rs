use super::libc;
use super::{KeyCode, KeyModifiers};
use std::cell::RefCell;
use xkbcommon::xkb;

use xkb::compose::Status as ComposeStatus;

pub struct Keyboard {
    context: xkb::Context,
    keymap: RefCell<xkb::Keymap>,
    device_id: i32,

    state: RefCell<xkb::State>,
    compose_state: RefCell<xkb::compose::State>,
}

impl Keyboard {
    pub fn new(connection: &xcb::Connection) -> (Keyboard, u8, u8) {
        connection.prefetch_extension_data(xcb::xkb::id());

        let (first_ev, first_er) = match connection.get_extension_data(xcb::xkb::id()) {
            Some(r) => (r.first_event(), r.first_error()),
            None => {
                panic!("could not get xkb extension data");
            }
        };

        {
            let cookie = xcb::xkb::use_extension(
                &connection,
                xkb::x11::MIN_MAJOR_XKB_VERSION,
                xkb::x11::MIN_MINOR_XKB_VERSION,
            );
            match cookie.get_reply() {
                Ok(r) => {
                    if !r.supported() {
                        panic!(
                            "required xcb-xkb-{}-{} is not supported",
                            xkb::x11::MIN_MAJOR_XKB_VERSION,
                            xkb::x11::MIN_MINOR_XKB_VERSION
                        );
                    }
                }
                Err(_) => {
                    panic!("could not check if xkb is supported");
                }
            }
        }

        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let device_id = xkb::x11::get_core_keyboard_device_id(&connection);
        if device_id == -1 {
            panic!("Couldn't find core keyboard device");
        }

        let keymap = xkb::x11::keymap_new_from_device(
            &context,
            &connection,
            device_id,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        );
        let state = xkb::x11::state_new_from_device(&keymap, &connection, device_id);

        use std::ffi::CStr;
        let locale = unsafe{CStr::from_ptr(libc::setlocale(libc::LC_CTYPE, std::ptr::null()))}
            .to_str()
            .expect("Failed to query locale");

        let table =
            xkb::compose::Table::new_from_locale(&context, locale, xkb::compose::COMPILE_NO_FLAGS)
                .expect("Failed to get ctable");
        let compose_state = xkb::compose::State::new(&table, xkb::compose::STATE_NO_FLAGS);

        {
            let map_parts = xcb::xkb::MAP_PART_KEY_TYPES
                | xcb::xkb::MAP_PART_KEY_SYMS
                | xcb::xkb::MAP_PART_MODIFIER_MAP
                | xcb::xkb::MAP_PART_EXPLICIT_COMPONENTS
                | xcb::xkb::MAP_PART_KEY_ACTIONS
                | xcb::xkb::MAP_PART_KEY_BEHAVIORS
                | xcb::xkb::MAP_PART_VIRTUAL_MODS
                | xcb::xkb::MAP_PART_VIRTUAL_MOD_MAP;

            let events = xcb::xkb::EVENT_TYPE_NEW_KEYBOARD_NOTIFY
                | xcb::xkb::EVENT_TYPE_MAP_NOTIFY
                | xcb::xkb::EVENT_TYPE_STATE_NOTIFY;

            let cookie = xcb::xkb::select_events_checked(
                &connection,
                device_id as u16,
                events as u16,
                0,
                events as u16,
                map_parts as u16,
                map_parts as u16,
                None,
            );

            cookie
                .request_check()
                .expect("failed to select notify events from xcb xkb");
        }

        let kbd = Keyboard {
            context: context,
            device_id: device_id,
            keymap: RefCell::new(keymap),
            state: RefCell::new(state),
            compose_state: RefCell::new(compose_state),
        };

        (kbd, first_ev, first_er)
    }

    pub fn process_key_event(
        &self,
        xcb_ev: &xcb::KeyPressEvent,
    ) -> Option<(KeyCode, KeyModifiers)> {
        let xcode = xcb_ev.detail() as xkb::Keycode;
        let xsym = self.state.borrow().key_get_one_sym(xcode);
        let pressed = (xcb_ev.response_type() & !0x80) == xcb::KEY_PRESS;
        if pressed {
            self.compose_state.borrow_mut().feed(xsym);
        } else {
            return None;
        }

        let cstate = self.compose_state.borrow().status().clone();
        let ksym = match cstate {
            ComposeStatus::Composing => {
                // eat
                return None;
            }
            ComposeStatus::Composed => {
                let res = self.compose_state.borrow().keysym();
                self.compose_state.borrow_mut().reset();
                res.unwrap_or(xsym)
            }
            ComposeStatus::Nothing => xsym,
            ComposeStatus::Cancelled => {
                self.compose_state.borrow_mut().reset();
                return None;
            }
        };

        // could be from_u32_unchecked
        let ks_char = std::char::from_u32(xkb::keysym_to_utf32(ksym));

        let kc = match ks_char {
            Some(c) if (c as u32) >= 0x20 && (c as u32) != 0x7f => KeyCode::Char(c),
            _ => {
                if let Some(key) = keysym_to_keycode(xsym) {
                    key
                } else {
                    debug!("xkbc:Missing xcb keysym {} definition", xsym);
                    return None;
                }
            }
        };

        Some((kc, self.get_key_modifiers()))
    }

    fn mod_is_active(&self, modifier: &str) -> bool {
        // [TODO] consider state  Depressed & consumed mods
        self.state
            .borrow()
            .mod_name_is_active(modifier, xkb::STATE_MODS_EFFECTIVE)
    }

    pub fn get_key_modifiers(&self) -> KeyModifiers {
        let mut res = KeyModifiers::default();

        if self.mod_is_active(xkb::MOD_NAME_SHIFT) {
            res |= KeyModifiers::SHIFT;
        }
        if self.mod_is_active(xkb::MOD_NAME_CTRL) {
            res |= KeyModifiers::CTRL;
        }
        if self.mod_is_active(xkb::MOD_NAME_ALT) {
            // Mod1
            res |= KeyModifiers::ALT;
        }
        if self.mod_is_active(xkb::MOD_NAME_LOGO) {
            // Mod4
            res |= KeyModifiers::SUPER;
        }
        if self.mod_is_active("Mod3") {
            res |= KeyModifiers::SUPER;
        }
        //Mod2 is numlock
        res
    }

    pub fn process_xkb_event(&self, connection: &xcb::Connection, event: &xcb::GenericEvent) {
        let xkb_ev: &XkbGenericEvent = unsafe { xcb::cast_event(&event) };

        if xkb_ev.device_id() == self.get_device_id() as u8 {
            match xkb_ev.xkb_type() {
                xcb::xkb::STATE_NOTIFY => {
                    self.update_state(unsafe { xcb::cast_event(&event) });
                }
                xcb::xkb::MAP_NOTIFY | xcb::xkb::NEW_KEYBOARD_NOTIFY => {
                    self.update_keymap(connection);
                }
                _ => {}
            }
        }
    }
    // for convenience, this fn takes &self, not &mut self
    pub fn update_state(&self, ev: &xcb::xkb::StateNotifyEvent) {
        self.state.borrow_mut().update_mask(
            ev.base_mods() as xkb::ModMask,
            ev.latched_mods() as xkb::ModMask,
            ev.locked_mods() as xkb::ModMask,
            ev.base_group() as xkb::LayoutIndex,
            ev.latched_group() as xkb::LayoutIndex,
            ev.locked_group() as xkb::LayoutIndex,
        );
    }

    pub fn update_keymap(&self, connection: &xcb::Connection) {
        let new_keymap = xkb::x11::keymap_new_from_device(
            &self.context,
            &connection,
            self.get_device_id(),
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        );
        if new_keymap.get_raw_ptr() == std::ptr::null_mut() {
            eprintln!("problem with new keymap");
        }
        let new_state = xkb::x11::state_new_from_device(&new_keymap, &connection, self.device_id);
        if new_state.get_raw_ptr() == std::ptr::null_mut() {
            eprintln!("problem with new state");
        }
        self.state.replace(new_state);
        self.keymap.replace(new_keymap);
        eprintln!("updated keymap");
    }

    pub fn get_device_id(&self) -> i32 {
        self.device_id
    }
}

#[cfg_attr(rustfmt, rustfmt_skip)]
fn keysym_to_keycode(keysym: u32) -> Option<KeyCode> {
    let res =
        match keysym {
            xkb::KEY_Escape =>                   KeyCode::Escape,
            xkb::KEY_Tab =>                      KeyCode::Tab,
          //xkb::KEY_ISO_Left_Tab =>             KeyCode::LeftTab,
            xkb::KEY_BackSpace =>                KeyCode::Backspace,
            xkb::KEY_Return =>                   KeyCode::Char(0xdu8 as char),
            xkb::KEY_Insert =>                   KeyCode::Insert,
            xkb::KEY_Delete =>                   KeyCode::Delete,
            xkb::KEY_Clear =>                    KeyCode::Delete,
            xkb::KEY_Pause =>                    KeyCode::Pause,
            xkb::KEY_Print =>                    KeyCode::Print,
          // 0x1005FF60 =>                       KeyCode::SysRq,         // hardcoded Sun SysReq
          // 0x1007ff00 =>                       KeyCode::SysRq,         // hardcoded X386 SysReq

          // cursor movement

            xkb::KEY_Home =>                     KeyCode::Home,
            xkb::KEY_End =>                      KeyCode::End,
            xkb::KEY_Left =>                     KeyCode::LeftArrow,
            xkb::KEY_Up =>                       KeyCode::UpArrow,
            xkb::KEY_Right =>                    KeyCode::RightArrow,
            xkb::KEY_Down =>                     KeyCode::DownArrow,
            xkb::KEY_Page_Up =>                  KeyCode::PageUp,
            xkb::KEY_Page_Down =>                KeyCode::PageDown,
          //xkb::KEY_Prior =>                    KeyCode::PageUp,
          //xkb::KEY_Next =>                     KeyCode::PageDown,

          // modifiers

            xkb::KEY_Shift_L =>                  KeyCode::Shift,
            xkb::KEY_Shift_R =>                  KeyCode::Shift,
          //xkb::KEY_Shift_Lock =>               KeyCode::Shift,
            xkb::KEY_Control_L =>                KeyCode::Control,
            xkb::KEY_Control_R =>                KeyCode::Control,
          //xkb::KEY_Meta_L =>                   KeyCode::LeftMeta,
          //xkb::KEY_Meta_R =>                   KeyCode::RightMeta,
            xkb::KEY_Alt_L =>                    KeyCode::Alt,
            xkb::KEY_Alt_R =>                    KeyCode::Alt,
            xkb::KEY_Caps_Lock =>                KeyCode::CapsLock,
            xkb::KEY_Num_Lock =>                 KeyCode::NumLock,
            xkb::KEY_Scroll_Lock =>              KeyCode::ScrollLock,
            xkb::KEY_Super_L =>                  KeyCode::Super,
            xkb::KEY_Super_R =>                  KeyCode::Super,
            xkb::KEY_Menu =>                     KeyCode::Menu,
            xkb::KEY_Help =>                     KeyCode::Help,
          //0x1000FF74 =>                       KeyCode::LeftTab, // hardcoded HP backtab
          //0x1005FF10 =>                       KeyCode::F11,     // hardcoded Sun F36 (labeled F11)
          //0x1005FF11 =>                       KeyCode::F12,     // hardcoded Sun F37 (labeled F12)

            xkb::KEY_F1 =>                       KeyCode::Function(1),
            xkb::KEY_F2 =>                       KeyCode::Function(2),
            xkb::KEY_F3 =>                       KeyCode::Function(3),
            xkb::KEY_F4 =>                       KeyCode::Function(4),
            xkb::KEY_F5 =>                       KeyCode::Function(5),
            xkb::KEY_F6 =>                       KeyCode::Function(6),
            xkb::KEY_F7 =>                       KeyCode::Function(7),
            xkb::KEY_F8 =>                       KeyCode::Function(8),
            xkb::KEY_F9 =>                       KeyCode::Function(9),
            xkb::KEY_F10 =>                      KeyCode::Function(10),
            xkb::KEY_F11 =>                      KeyCode::Function(11),
            xkb::KEY_F12 =>                      KeyCode::Function(12),


          // numeric and function keypad keys

            xkb::KEY_KP_Enter =>                 KeyCode::Char(0xdu8 as char),
            xkb::KEY_KP_Delete =>                KeyCode::Delete,
            xkb::KEY_KP_Home =>                  KeyCode::Home,
          //xkb::KEY_KP_Begin =>                 KeyCode::KP_Begin,
          //xkb::KEY_KP_End =>                   KeyCode::KP_End,
            xkb::KEY_KP_Page_Up =>               KeyCode::PageUp,
            xkb::KEY_KP_Page_Down =>             KeyCode::PageDown,
          //xkb::KEY_KP_Up =>                    KeyCode::KP_Up,
          //xkb::KEY_KP_Down =>                  KeyCode::KP_Down,
          //xkb::KEY_KP_Left =>                  KeyCode::KP_Left,
          //xkb::KEY_KP_Right =>                 KeyCode::KP_Right,
          //xkb::KEY_KP_Equal =>                 KeyCode::KP_Equal,
            xkb::KEY_KP_Multiply =>              KeyCode::Multiply,
            xkb::KEY_KP_Add =>                   KeyCode::Add,
            xkb::KEY_KP_Divide =>                KeyCode::Divide,
            xkb::KEY_KP_Subtract =>              KeyCode::Subtract,
            xkb::KEY_KP_Decimal =>               KeyCode::Decimal,
            xkb::KEY_KP_Separator =>             KeyCode::Separator,

            xkb::KEY_KP_0 =>                     KeyCode::Numpad0,
            xkb::KEY_KP_1 =>                     KeyCode::Numpad1,
            xkb::KEY_KP_2 =>                     KeyCode::Numpad2,
            xkb::KEY_KP_3 =>                     KeyCode::Numpad3,
            xkb::KEY_KP_4 =>                     KeyCode::Numpad4,
            xkb::KEY_KP_6 =>                     KeyCode::Numpad6,
            xkb::KEY_KP_7 =>                     KeyCode::Numpad7,
            xkb::KEY_KP_8 =>                     KeyCode::Numpad8,
            xkb::KEY_KP_9 =>                     KeyCode::Numpad9,

          // International input method support keys

          // International & multi-key character composition
          //xkb::KEY_ISO_Level3_Shift =>         KeyCode::AltGr, // AltGr
          //xkb::KEY_Multi_key =>                KeyCode::Multi_key,
          //xkb::KEY_Codeinput =>                KeyCode::Codeinput,
          //xkb::KEY_SingleCandidate =>          KeyCode::SingleCandidate,
          //xkb::KEY_MultipleCandidate =>        KeyCode::MultipleCandidate,
          //xkb::KEY_PreviousCandidate =>        KeyCode::PreviousCandidate,

          // Misc Functions
          //xkb::KEY_Mode_switch =>              KeyCode::ModeSwitch,

          //// Japanese keyboard support
          //xkb::KEY_Kanji =>                    KeyCode::Kanji,
          //xkb::KEY_Muhenkan =>                 KeyCode::Muhenkan,
          ////xkb::KEY_Henkan_Mode =>            KeyCode::Henkan_Mode,
          //xkb::KEY_Henkan_Mode =>              KeyCode::Henkan,
          //xkb::KEY_Henkan =>                   KeyCode::Henkan,
          //xkb::KEY_Romaji =>                   KeyCode::Romaji,
          //xkb::KEY_Hiragana =>                 KeyCode::Hiragana,
          //xkb::KEY_Katakana =>                 KeyCode::Katakana,
          //xkb::KEY_Hiragana_Katakana =>        KeyCode::Hiragana_Katakana,
          //xkb::KEY_Zenkaku =>                  KeyCode::Zenkaku,
          //xkb::KEY_Hankaku =>                  KeyCode::Hankaku,
          //xkb::KEY_Zenkaku_Hankaku =>          KeyCode::Zenkaku_Hankaku,
          //xkb::KEY_Touroku =>                  KeyCode::Touroku,
          //xkb::KEY_Massyo =>                   KeyCode::Massyo,
          //xkb::KEY_Kana_Lock =>                KeyCode::Kana_Lock,
          //xkb::KEY_Kana_Shift =>               KeyCode::Kana_Shift,
          //xkb::KEY_Eisu_Shift =>               KeyCode::Eisu_Shift,
          //xkb::KEY_Eisu_toggle =>              KeyCode::Eisu_toggle,
          ////xkb::KEY_Kanji_Bangou =>           KeyCode::Kanji_Bangou,
          ////xkb::KEY_Zen_Koho =>               KeyCode::Zen_Koho,
          ////xkb::KEY_Mae_Koho =>               KeyCode::Mae_Koho,
          //xkb::KEY_Kanji_Bangou =>             KeyCode::Codeinput,
          //xkb::KEY_Zen_Koho =>                 KeyCode::MultipleCandidate,
          //xkb::KEY_Mae_Koho =>                 KeyCode::PreviousCandidate,

          //// Korean keyboard support
          //xkb::KEY_Hangul =>                   KeyCode::Hangul,
          //xkb::KEY_Hangul_Start =>             KeyCode::Hangul_Start,
          //xkb::KEY_Hangul_End =>               KeyCode::Hangul_End,
          //xkb::KEY_Hangul_Hanja =>             KeyCode::Hangul_Hanja,
          //xkb::KEY_Hangul_Jamo =>              KeyCode::Hangul_Jamo,
          //xkb::KEY_Hangul_Romaja =>            KeyCode::Hangul_Romaja,
          ////xkb::KEY_Hangul_Codeinput =>       KeyCode::Hangul_Codeinput,
          //xkb::KEY_Hangul_Codeinput =>         KeyCode::Codeinput,
          //xkb::KEY_Hangul_Jeonja =>            KeyCode::Hangul_Jeonja,
          //xkb::KEY_Hangul_Banja =>             KeyCode::Hangul_Banja,
          //xkb::KEY_Hangul_PreHanja =>          KeyCode::Hangul_PreHanja,
          //xkb::KEY_Hangul_PostHanja =>         KeyCode::Hangul_PostHanja,
          ////xkb::KEY_Hangul_SingleCandidate => KeyCode::Hangul_SingleCandidate,
          ////xkb::KEY_Hangul_MultipleCandidate, ey.Hangul_MultipleCandidate,
          ////xkb::KEY_Hangul_PreviousCandidate, ey.Hangul_PreviousCandidate,
          //xkb::KEY_Hangul_SingleCandidate =>   KeyCode::SingleCandidate,
          //xkb::KEY_Hangul_MultipleCandidate => KeyCode::MultipleCandidate,
          //xkb::KEY_Hangul_PreviousCandidate => KeyCode::PreviousCandidate,
          //xkb::KEY_Hangul_Special =>           KeyCode::Hangul_Special,
          ////xkb::KEY_Hangul_switch =>          KeyCode::Hangul_switch,
          //xkb::KEY_Hangul_switch =>            KeyCode::Mode_switch,


          // Special keys from X.org - This include multimedia keys,
          // wireless/bluetooth/uwb keys, special launcher keys, etc.
            xkb::KEY_XF86Back =>                 KeyCode::BrowserBack,
            xkb::KEY_XF86Forward =>              KeyCode::BrowserForward,
            xkb::KEY_XF86Stop =>                 KeyCode::BrowserStop,
            xkb::KEY_XF86Refresh =>              KeyCode::BrowserRefresh,
            xkb::KEY_XF86Favorites =>            KeyCode::BrowserFavorites,
          //xkb::KEY_XF86AudioMedia =>           KeyCode::LaunchMedia,
          //xkb::KEY_XF86OpenURL =>              KeyCode::OpenUrl,
            xkb::KEY_XF86HomePage =>             KeyCode::BrowserHome,
          //xkb::KEY_XF86Search =>               KeyCode::Search,
            xkb::KEY_XF86AudioLowerVolume =>     KeyCode::VolumeDown,
            xkb::KEY_XF86AudioMute =>            KeyCode::VolumeMute,
            xkb::KEY_XF86AudioRaiseVolume =>     KeyCode::VolumeUp,
         // xkb::KEY_XF86AudioPlay =>            KeyCode::MediaPlay,
         // xkb::KEY_XF86AudioStop =>            KeyCode::MediaStop,
         // xkb::KEY_XF86AudioPrev =>            KeyCode::MediaPrevious,
         // xkb::KEY_XF86AudioNext =>            KeyCode::MediaNext,
         // xkb::KEY_XF86AudioRecord =>          KeyCode::MediaRecord,
         // xkb::KEY_XF86AudioPause =>           KeyCode::MediaPause,
         // xkb::KEY_XF86Mail =>                 KeyCode::LaunchMail,
         // xkb::KEY_XF86MyComputer =>           KeyCode::MyComputer,
         // xkb::KEY_XF86Calculator =>           KeyCode::Calculator,
         // xkb::KEY_XF86Memo =>                 KeyCode::Memo,
         // xkb::KEY_XF86ToDoList =>             KeyCode::ToDoList,
         // xkb::KEY_XF86Calendar =>             KeyCode::Calendar,
         // xkb::KEY_XF86PowerDown =>            KeyCode::PowerDown,
         // xkb::KEY_XF86ContrastAdjust =>       KeyCode::ContrastAdjust,
         // xkb::KEY_XF86Standby =>              KeyCode::Standby,
         // xkb::KEY_XF86MonBrightnessUp =>      KeyCode::MonBrightnessUp,
         // xkb::KEY_XF86MonBrightnessDown =>    KeyCode::MonBrightnessDown,
         // xkb::KEY_XF86KbdLightOnOff =>        KeyCode::KeyboardLightOnOff,
         // xkb::KEY_XF86KbdBrightnessUp =>      KeyCode::KeyboardBrightnessUp,
         // xkb::KEY_XF86KbdBrightnessDown =>    KeyCode::KeyboardBrightnessDown,
         // xkb::KEY_XF86PowerOff =>             KeyCode::PowerOff,
         // xkb::KEY_XF86WakeUp =>               KeyCode::WakeUp,
         // xkb::KEY_XF86Eject =>                KeyCode::Eject,
         // xkb::KEY_XF86ScreenSaver =>          KeyCode::ScreenSaver,
         // xkb::KEY_XF86WWW =>                  KeyCode::WWW,
         // xkb::KEY_XF86Sleep =>                KeyCode::Sleep,
         // xkb::KEY_XF86LightBulb =>            KeyCode::LightBulb,
         // xkb::KEY_XF86Shop =>                 KeyCode::Shop,
         // xkb::KEY_XF86History =>              KeyCode::History,
         // xkb::KEY_XF86AddFavorite =>          KeyCode::AddFavorite,
         // xkb::KEY_XF86HotLinks =>             KeyCode::HotLinks,
         // xkb::KEY_XF86BrightnessAdjust =>     KeyCode::BrightnessAdjust,
         // xkb::KEY_XF86Finance =>              KeyCode::Finance,
         // xkb::KEY_XF86Community =>            KeyCode::Community,
         // xkb::KEY_XF86AudioRewind =>          KeyCode::AudioRewind,
         // xkb::KEY_XF86BackForward =>          KeyCode::BackForward,
         // xkb::KEY_XF86ApplicationLeft =>      KeyCode::ApplicationLeft,
         // xkb::KEY_XF86ApplicationRight =>     KeyCode::ApplicationRight,
         // xkb::KEY_XF86Book =>                 KeyCode::Book,
         // xkb::KEY_XF86CD =>                   KeyCode::CD,
         // xkb::KEY_XF86Calculater =>           KeyCode::Calculator,
         // xkb::KEY_XF86Clear =>                KeyCode::Clear,
         // xkb::KEY_XF86ClearGrab =>            KeyCode::ClearGrab,
         // xkb::KEY_XF86Close =>                KeyCode::Close,
         // xkb::KEY_XF86Copy =>                 KeyCode::Copy,
         // xkb::KEY_XF86Cut =>                  KeyCode::Cut,
         // xkb::KEY_XF86Display =>              KeyCode::Display,
         // xkb::KEY_XF86DOS =>                  KeyCode::DOS,
         // xkb::KEY_XF86Documents =>            KeyCode::Documents,
         // xkb::KEY_XF86Excel =>                KeyCode::Excel,
         // xkb::KEY_XF86Explorer =>             KeyCode::Explorer,
         // xkb::KEY_XF86Game =>                 KeyCode::Game,
         // xkb::KEY_XF86Go =>                   KeyCode::Go,
         // xkb::KEY_XF86iTouch =>               KeyCode::iTouch,
         // xkb::KEY_XF86LogOff =>               KeyCode::LogOff,
         // xkb::KEY_XF86Market =>               KeyCode::Market,
         // xkb::KEY_XF86Meeting =>              KeyCode::Meeting,
         // xkb::KEY_XF86MenuKB =>               KeyCode::MenuKB,
         // xkb::KEY_XF86MenuPB =>               KeyCode::MenuPB,
         // xkb::KEY_XF86MySites =>              KeyCode::MySites,
         // xkb::KEY_XF86New =>                  KeyCode::New,
         // xkb::KEY_XF86News =>                 KeyCode::News,
         // xkb::KEY_XF86OfficeHome =>           KeyCode::OfficeHome,
         // xkb::KEY_XF86Open =>                 KeyCode::Open,
         // xkb::KEY_XF86Option =>               KeyCode::Option,
         // xkb::KEY_XF86Paste =>                KeyCode::Paste,
         // xkb::KEY_XF86Phone =>                KeyCode::Phone,
         // xkb::KEY_XF86Reply =>                KeyCode::Reply,
         // xkb::KEY_XF86Reload =>               KeyCode::Reload,
         // xkb::KEY_XF86RotateWindows =>        KeyCode::RotateWindows,
         // xkb::KEY_XF86RotationPB =>           KeyCode::RotationPB,
         // xkb::KEY_XF86RotationKB =>           KeyCode::RotationKB,
         // xkb::KEY_XF86Save =>                 KeyCode::Save,
         // xkb::KEY_XF86Send =>                 KeyCode::Send,
         // xkb::KEY_XF86Spell =>                KeyCode::Spell,
         // xkb::KEY_XF86SplitScreen =>          KeyCode::SplitScreen,
         // xkb::KEY_XF86Support =>              KeyCode::Support,
         // xkb::KEY_XF86TaskPane =>             KeyCode::TaskPane,
         // xkb::KEY_XF86Terminal =>             KeyCode::Terminal,
         // xkb::KEY_XF86Tools =>                KeyCode::Tools,
         // xkb::KEY_XF86Travel =>               KeyCode::Travel,
         // xkb::KEY_XF86Video =>                KeyCode::Video,
         // xkb::KEY_XF86Word =>                 KeyCode::Word,
         // xkb::KEY_XF86Xfer =>                 KeyCode::Xfer,
         // xkb::KEY_XF86ZoomIn =>               KeyCode::ZoomIn,
         // xkb::KEY_XF86ZoomOut =>              KeyCode::ZoomOut,
         // xkb::KEY_XF86Away =>                 KeyCode::Away,
         // xkb::KEY_XF86Messenger =>            KeyCode::Messenger,
         // xkb::KEY_XF86WebCam =>               KeyCode::WebCam,
         // xkb::KEY_XF86MailForward =>          KeyCode::MailForward,
         // xkb::KEY_XF86Pictures =>             KeyCode::Pictures,
         // xkb::KEY_XF86Music =>                KeyCode::Music,
         // xkb::KEY_XF86Battery =>              KeyCode::Battery,
         // xkb::KEY_XF86Bluetooth =>            KeyCode::Bluetooth,
         // xkb::KEY_XF86WLAN =>                 KeyCode::WLAN,
         // xkb::KEY_XF86UWB =>                  KeyCode::UWB,
         // xkb::KEY_XF86AudioForward =>         KeyCode::AudioForward,
         // xkb::KEY_XF86AudioRepeat =>          KeyCode::AudioRepeat,
         // xkb::KEY_XF86AudioRandomPlay =>      KeyCode::AudioRandomPlay,
         // xkb::KEY_XF86Subtitle =>             KeyCode::Subtitle,
         // xkb::KEY_XF86AudioCycleTrack =>      KeyCode::AudioCycleTrack,
         // xkb::KEY_XF86Time =>                 KeyCode::Time,
         // xkb::KEY_XF86Select =>               KeyCode::Select,
         // xkb::KEY_XF86View =>                 KeyCode::View,
         // xkb::KEY_XF86TopMenu =>              KeyCode::TopMenu,
         // xkb::KEY_XF86Red =>                  KeyCode::Red,
         // xkb::KEY_XF86Green =>                KeyCode::Green,
         // xkb::KEY_XF86Yellow =>               KeyCode::Yellow,
         // xkb::KEY_XF86Blue =>                 KeyCode::Blue,
         // xkb::KEY_XF86Bluetooth =>            KeyCode::Bluetooth,
         // xkb::KEY_XF86Suspend =>              KeyCode::Suspend,
         // xkb::KEY_XF86Hibernate =>            KeyCode::Hibernate,
         // xkb::KEY_XF86TouchpadToggle =>       KeyCode::TouchpadToggle,
         // xkb::KEY_XF86TouchpadOn =>           KeyCode::TouchpadOn,
         // xkb::KEY_XF86TouchpadOff =>          KeyCode::TouchpadOff,
         // xkb::KEY_XF86AudioMicMute =>         KeyCode::MicMute,
         // xkb::KEY_XF86Launch0 =>              KeyCode::Launch0, // ### Qt 6: remap properly
         // xkb::KEY_XF86Launch1 =>              KeyCode::Launch1,
         // xkb::KEY_XF86Launch2 =>              KeyCode::Launch2,
         // xkb::KEY_XF86Launch3 =>              KeyCode::Launch3,
         // xkb::KEY_XF86Launch4 =>              KeyCode::Launch4,
         // xkb::KEY_XF86Launch5 =>              KeyCode::Launch5,
         // xkb::KEY_XF86Launch6 =>              KeyCode::Launch6,
         // xkb::KEY_XF86Launch7 =>              KeyCode::Launch7,
         // xkb::KEY_XF86Launch8 =>              KeyCode::Launch8,
         // xkb::KEY_XF86Launch9 =>              KeyCode::Launch9,
         // xkb::KEY_XF86LaunchA =>              KeyCode::LaunchA,
         // xkb::KEY_XF86LaunchB =>              KeyCode::LaunchB,
         // xkb::KEY_XF86LaunchC =>              KeyCode::LaunchC,
         // xkb::KEY_XF86LaunchD =>              KeyCode::LaunchD,
         // xkb::KEY_XF86LaunchE =>              KeyCode::LaunchE,
         // xkb::KEY_XF86LaunchF =>              KeyCode::LaunchF,
            _ => {return None;}
        };
    Some(res)
}

/// struct that has fields common to the 3 different xkb events
/// (StateNotify, NewKeyboardNotify, MapNotify)
#[repr(C)]
struct xcb_xkb_generic_event_t {
    response_type: u8,
    xkb_type: u8,
    sequence: u16,
    time: xcb::Timestamp,
    device_id: u8,
}

struct XkbGenericEvent {
    base: xcb::Event<xcb_xkb_generic_event_t>,
}

impl XkbGenericEvent {
    pub fn response_type(&self) -> u8 {
        unsafe { (*self.base.ptr).response_type }
    }
    #[allow(non_snake_case)]
    pub fn xkb_type(&self) -> u8 {
        unsafe { (*self.base.ptr).xkb_type }
    }
    pub fn sequence(&self) -> u16 {
        unsafe { (*self.base.ptr).sequence }
    }
    pub fn time(&self) -> xcb::Timestamp {
        unsafe { (*self.base.ptr).time }
    }
    #[allow(non_snake_case)]
    pub fn device_id(&self) -> u8 {
        unsafe { (*self.base.ptr).device_id }
    }
}
