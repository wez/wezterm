use libc::c_uint;
use scopeguard::defer;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use x11::xlib;
use xcb;
use xkbcommon::xkb::{self};

#[derive(Debug, Clone, Copy)]
pub struct ModifierIndex {
    pub idx: xkb::ModIndex,
}

impl Default for ModifierIndex {
    fn default() -> Self {
        return Self {
            idx: xkb::MOD_INVALID,
        };
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct ModifierMap {
    pub ctrl: ModifierIndex,
    pub meta: ModifierIndex,
    pub alt: ModifierIndex,
    pub shift: ModifierIndex,
    pub caps_lock: ModifierIndex,
    pub num_lock: ModifierIndex,
    pub supr: ModifierIndex,
    pub hyper: ModifierIndex,
}

struct KeyProbingAlgorithm<'a> {
    failed: bool,

    try_shift: bool,
    shift_keycode: u32,

    shift: u32,
    ctrl: u32,
    alt: u32,
    meta: u32,
    caps_lock: u32,
    num_lock: u32,
    supr: u32,
    hyper: u32,

    state: &'a mut xkb::State,
}

impl<'a> KeyProbingAlgorithm<'a> {
    fn new(state: &'a mut xkb::State) -> KeyProbingAlgorithm<'a> {
        return KeyProbingAlgorithm {
            failed: false,
            try_shift: false,
            shift_keycode: 0,

            shift: 0,
            ctrl: 0,
            alt: 0,
            meta: 0,
            caps_lock: 0,
            num_lock: 0,
            supr: 0,
            hyper: 0,

            state,
        };
    }
}

impl<'a> KeyProbingAlgorithm<'a> {
    /// Algorithm for mapping virtual modifiers to real modifiers:
    ///   1. create new state
    ///   2. for each key in keymap
    ///      a. send key down to state
    ///      b. if it affected exactly one bit in modifier map
    ///         i) get keysym
    ///         ii) if keysym matches one of the known modifiers, save it for that modifier
    ///         iii) if modifier is latched, send key up and key down to toggle again
    ///      c. send key up to reset the state
    ///   3. if shift key found in step 2, run step 2 with all shift+key for each key
    ///   4. if shift, control, alt and super are not all found, declare failure
    ///   5. if failure, use static mapping from xkbcommon-names.h
    ///
    /// Step 3 is needed because many popular keymaps map meta to alt+shift.
    ///
    /// We could do better by constructing a system of linear equations, but it should not be
    /// needed in any sane system. We could also use this algorithm with X11, but X11
    /// provides XkbVirtualModsToReal which is guaranteed to be accurate, while this
    /// algorithm is only a heuristic.
    ///
    /// We don't touch level3 or level5 modifiers.
    ///
    fn visit_key(&mut self, key: xkb::Keycode) {
        log::debug!("Key: {:?}", key);

        if self.failed {
            return;
        }

        if self.try_shift {
            if key == self.shift_keycode {
                return;
            }

            self.state
                .update_key(self.shift_keycode, xkb::KeyDirection::Down);
        }

        let changed_type = self.state.update_key(key, xkb::KeyDirection::Down);

        if changed_type
            & (xkb::STATE_MODS_DEPRESSED | xkb::STATE_MODS_LATCHED | xkb::STATE_MODS_LOCKED)
            != 0
        {
            let mods = self.state.serialize_mods(if self.try_shift {
                xkb::STATE_MODS_EFFECTIVE
            } else {
                xkb::STATE_MODS_DEPRESSED | xkb::STATE_MODS_LATCHED | xkb::STATE_MODS_LOCKED
            });

            let keysyms = self.state.key_get_syms(key);
            log::debug!("Keysym: {:x?}", keysyms);

            macro_rules! assign_left_right {
                ($key_l:expr, $key_r:expr, $mod:ident) => {
                    if keysyms[0] == $key_l || keysyms[0] == $key_r {
                        if self.$mod == 0 {
                            log::debug!(
                                "Keycode {:?} triggered keysym '{:x?}' for {:?}' modifier.",
                                key,
                                keysyms[0],
                                stringify!($mod)
                            );
                            self.$mod = mods;
                        } else if self.$mod != mods {
                            log::debug!(
                                "Keycode {:?} triggered again keysym '{:x?}' for {:?}' modifier.",
                                key,
                                keysyms[0],
                                stringify!($mod)
                            );
                            self.failed = true;
                        }
                    }
                };
            }

            macro_rules! assign {
                ($key:expr, $mod:ident) => {
                    if keysyms[0] == $key && self.$mod == 0 {
                        log::debug!(
                            "Key {:?} triggered keysym '{:?}' for '{:?}' modifier.",
                            key,
                            keysyms[0],
                            stringify!($mod)
                        );
                        self.$mod = mods;
                    }
                };
            }

            // We can handle exactly one keysym with exactly one bit set in the implementation
            // below; with a lot more gymnastics, we could set up an 8x8 linear system and solve
            // for each modifier in case there are some modifiers that are only present in
            // combination with others, but it is not worth the effort.
            if keysyms.len() == 1 && mods.is_power_of_two() {
                assign_left_right!(xkb::KEY_Shift_L, xkb::KEY_Shift_R, shift);
                assign_left_right!(xkb::KEY_Control_L, xkb::KEY_Control_R, ctrl);
                assign!(xkb::KEY_Caps_Lock, caps_lock);
                assign!(xkb::KEY_Shift_Lock, num_lock);
                assign_left_right!(xkb::KEY_Alt_L, xkb::KEY_Alt_R, alt);
                assign_left_right!(xkb::KEY_Meta_L, xkb::KEY_Meta_R, meta);
                assign_left_right!(xkb::KEY_Super_L, xkb::KEY_Super_R, supr);
                assign_left_right!(xkb::KEY_Hyper_L, xkb::KEY_Hyper_R, hyper);
            }

            if self.shift_keycode == 0
                && (keysyms[0] == xkb::KEY_Shift_L || keysyms[0] == xkb::KEY_Shift_R)
            {
                log::debug!("Found shift keycode.");
                self.shift_keycode = key;
            }

            // If this is a lock, then up and down to remove lock state
            if changed_type & xkb::STATE_MODS_LOCKED != 0 {
                log::debug!("Found lock self.state. Set up/down to remove lock self.state.");
                self.state.update_key(key, xkb::KeyDirection::Up);
                self.state.update_key(key, xkb::KeyDirection::Down);
            }
        }

        self.state.update_key(key, xkb::KeyDirection::Up);

        if self.try_shift {
            self.state
                .update_key(self.shift_keycode, xkb::KeyDirection::Up);
        }
    }

    fn visit_keymap(&mut self, keymap: &xkb::Keymap) {
        keymap.key_for_each(|_, key| {
            self.visit_key(key);
        });
    }
}

/// This function initializes wezterm internal modifiers depending
/// on a default mapping.
///
/// This function simply queries the index for the xkb modifiers
/// `Control`, `Lock`, `Shift`, `Mod1`, `Mod2`, `Mod4`
/// and treats them as default (assumption) to
/// `Ctrl`, `Caps_Lock`, `Shift`, `Alt`, `Num_Lock`, `Super`
///
/// Modifiers `Hyper` and `Meta` are not detected.
fn init_modifier_table_fallback(keymap: &xkb::Keymap) -> ModifierMap {
    let mut mod_map = ModifierMap::default();

    macro_rules! assign {
        ($mod:ident, $n:expr) => {
            mod_map.$mod = ModifierIndex {
                idx: keymap.mod_get_index($n),
            };
        };
    }

    assign!(ctrl, xkb::MOD_NAME_CTRL);
    assign!(shift, xkb::MOD_NAME_SHIFT);
    assign!(alt, xkb::MOD_NAME_ALT);
    assign!(caps_lock, xkb::MOD_NAME_CAPS);
    assign!(num_lock, xkb::MOD_NAME_NUM);
    assign!(supr, xkb::MOD_NAME_LOGO);

    return mod_map;
}

/// This function initializes wezterm internal modifiers
/// by looking up virtual modifiers (e.g. run `xmodmap -pm`)
/// over the functionality exposed by X11.
/// This function tries to initialize `xkb` modifier indices for
/// all modifiers
/// `Ctrl`, `Shift`, `Alt`, `Num_Lock`, `Caps_Lock`, `Super`,
/// `Hyper`, `Meta`.
/// If not successful it uses [init_modifier_table_fallback](init_modifier_table_fallback)
/// to initialize the default.
pub fn init_modifier_table_x11(connection: &xcb::Connection, keymap: &xkb::Keymap) -> ModifierMap {
    // TODO: This implementation needs to be done with
    // https://github.com/kovidgoyal/kitty/blob/0248edbdb98cc3ae80d98bf5ad17fbf497a24a43/glfw/xkb_glfw.c#L321

    let mut mod_map = ModifierMap::default();

    // Keeping track of used real modifiers (bits).
    let mut used_real_mod_bits: u32 = 0;

    // Shift, Ctrl, CapsLock are special they cannot be identified reliably on X11
    macro_rules! assign {
        ($mod:ident, $n:expr) => {{
            mod_map.$mod = ModifierIndex {
                idx: keymap.mod_get_index($n),
            };
            used_real_mod_bits |= 1 << mod_map.$mod.idx;
        }};
    }
    assign!(ctrl, xkb::MOD_NAME_CTRL);
    assign!(shift, xkb::MOD_NAME_SHIFT);
    assign!(caps_lock, xkb::MOD_NAME_CAPS);

    // Mask variables for all modifiers not yet set.
    // The value stored here represents the same as `used_real_mod_bits`
    // Each bit is a real modifier (only 8 modifiers).
    let mut alt: u32 = 0;
    let mut supr: u32 = 0;
    let mut num_lock: u32 = 0;
    let mut meta: u32 = 0;
    let mut hyper: u32 = 0;

    let success = || -> bool {
        let xkb_ptr = unsafe {
            xlib::XkbGetMap(
                connection.get_raw_dpy(),
                (xcb::xkb::MapPart::VIRTUAL_MODS | xcb::xkb::MapPart::VIRTUAL_MOD_MAP).bits(),
                xcb::xkb::Id::UseCoreKbd as c_uint,
            )
        };

        if xkb_ptr.is_null() {
            log::warn!("Could not initialize moidifiers from virtual modifiers table.");
            return false;
        }

        defer! {
            unsafe { xlib::XkbFreeKeyboard(xkb_ptr, 0, xlib::True); }
        }

        let status = unsafe {
            xlib::XkbGetNames(
                connection.get_raw_dpy(),
                xcb::xkb::NameDetail::VIRTUAL_MOD_NAMES.bits(),
                xkb_ptr,
            )
        };

        if status != xlib::Success as c_int {
            return false;
        }

        defer! {
            unsafe {
                xlib::XkbFreeNames(
                xkb_ptr,
                xcb::xkb::NameDetail::VIRTUAL_MOD_NAMES.bits() as c_uint,
                xlib::True,
            )};
        }

        // Iterate over all virtual modifiers and get its map to the real modifiers
        // (variable `mask`) and check if the virtual modifier name
        // matches a wezterm internal modifier we handle.
        for i in 0..xlib::XkbNumVirtualMods {
            let atom = unsafe { (*(*xkb_ptr).names).vmods[i] };

            if atom == 0 {
                continue;
            }

            let mut mask: c_uint = 0;
            let mask_ptr: *mut c_uint = &mut mask;

            if unsafe { xlib::XkbVirtualModsToReal(xkb_ptr, (1 << i) as c_uint, mask_ptr) }
                == xlib::Success as c_int
            {
                log::debug!("Could not map virtual modifier index '{i}' to real modifier mask.");
                continue;
            }

            if used_real_mod_bits & mask != 0 {
                // The variable `mask` (multiple bits can be set, which means
                // this virtual modifier maps to multiple real modifiers) and overlaps
                // already used bits.
                continue;
            }

            let name: *mut c_char = unsafe { xlib::XGetAtomName(connection.get_raw_dpy(), atom) };
            let virtual_modifier_name = match unsafe { CStr::from_ptr(name).to_str() } {
                Ok(n) => n,
                Err(_) => continue,
            };

            match virtual_modifier_name {
                // Note that the order matters here; earlier is higher priority.
                "Alt" => {
                    alt = mask;
                    used_real_mod_bits |= mask;
                }
                "Super" => {
                    supr = mask;
                    used_real_mod_bits |= mask;
                }
                "NumLock" => {
                    num_lock = mask;
                    used_real_mod_bits |= mask;
                }
                "Meta" => {
                    meta = mask;
                    used_real_mod_bits |= mask;
                }
                "Hyper" => {
                    hyper = mask;
                    used_real_mod_bits |= mask;
                }
                _ => (),
            }
        }

        return true;
    }();

    if !success || used_real_mod_bits == 0 {
        return init_modifier_table_fallback(keymap);
    }

    macro_rules! assign {
        ($mod:ident, $i:ident) => {
            if $mod & ($i << 1) != 0 {
                mod_map.$mod = ModifierIndex { idx: $i };
            }
        };
    }

    // We could assign the bit index also above in the match statement, but
    // this is more efficient and we can do it together by traversing
    // the used bits.
    let mut idx = 0;
    while used_real_mod_bits != 0 {
        assign!(alt, idx);
        assign!(supr, idx);
        assign!(hyper, idx);
        assign!(meta, idx);
        assign!(num_lock, idx);

        idx += 1;
        used_real_mod_bits >>= 1;
    }

    return mod_map;
}

/// This function initializes wezterm internal modifiers
/// by probing the keyboard state for each keypress.
///
/// This is a workaround because under Wayland the code in
/// [init_modifier_table_x11](init_modifier_table_x11)
/// does not work.
///
/// This function tries to initialize `xkb` modifier indices for
/// all modifiers
/// `Ctrl`, `Shift`, `Alt`, `Num_Lock`, `Caps_Lock`, `Super`,
/// `Hyper`, `Meta` and if it fails it uses the fallback method
/// [init_modifier_table_fallback](init_modifier_table_fallback).
///
/// Implementation is taken from Kitty:
/// https://github.com/kovidgoyal/kitty/blob/0248edbdb98cc3ae80d98bf5ad17fbf497a24a43/glfw/xkb_glfw.c#L523
pub fn init_modifier_table_wayland(keymap: &xkb::Keymap) -> ModifierMap {
    //
    // This is a client side hack for wayland, once
    // https://github.com/xkbcommon/libxkbcommon/pull/36
    // or https://github.com/xkbcommon/libxkbcommon/pull/353
    // is implemented or some other solution exists
    // this code iterates through key presses to find the
    // activated modifiers.
    // Check: https://github.com/wez/wezterm/issues/4626

    log::info!("Detect modifiers on Wayland [with key press iterations].");

    let mut state: xkb::State = xkb::State::new(keymap);
    let mut algo = KeyProbingAlgorithm::new(&mut state);
    let mut mod_map = ModifierMap::default();

    algo.visit_keymap(&keymap);

    if algo.shift_keycode == 0 {
        algo.failed = true;
        log::debug!("Did not found shift keycode.")
    }

    if (algo.ctrl == 0
        || algo.alt == 0
        || algo.meta == 0
        || algo.shift == 0
        || algo.supr == 0
        || algo.hyper == 0)
        && !algo.failed
    {
        algo.try_shift = true;

        log::debug!("Detect modifiers on Wayland [with Shift+key press iterations].");
        algo.visit_keymap(&keymap);
    }

    // We must have found a least those 4 modifiers.
    if !algo.failed && (algo.ctrl == 0 || algo.shift == 0 || algo.alt == 0 || algo.supr == 0) {
        log::debug!("Some of essential modifiers (ctrl, shift, alt, supr) have not been found.");
        algo.failed = true;
    }

    if !algo.failed {
        let mut shifted: u32 = 1;
        let mut used_real_mod_bits: u32 = 0;

        // Algorithm can detect the same index for multiple modifiers
        // but we only want one assigned to each of our modifiers.
        macro_rules! try_assign_unique_index {
            ($mod:ident, $i:ident) => {
                if mod_map.$mod.idx == xkb::MOD_INVALID
                    && (used_real_mod_bits & shifted == 0)
                    && algo.$mod == shifted
                {
                    mod_map.$mod = ModifierIndex { idx: $i };
                    used_real_mod_bits |= shifted;
                }
            };
        }

        for i in 0..32 {
            try_assign_unique_index!(ctrl, i);
            try_assign_unique_index!(shift, i);
            try_assign_unique_index!(alt, i);
            try_assign_unique_index!(meta, i);
            try_assign_unique_index!(caps_lock, i);
            try_assign_unique_index!(num_lock, i);
            try_assign_unique_index!(supr, i);
            try_assign_unique_index!(hyper, i);
            shifted <<= 1;
        }
    } else {
        log::warn!("Detect modifiers on Wayland failed. Using default mapping.");
        mod_map = init_modifier_table_fallback(keymap);
    }

    log::info!("Modifier map {mod_map:#?}");

    return mod_map;
}
