use xkbcommon::xkb::{self};

#[derive(Debug, Clone, Copy)]
pub struct ModifierIndex {
    pub idx: xkb::ModIndex,
    pub mask: u32,
}

impl Default for ModifierIndex {
    fn default() -> Self {
        return Self {
            idx: xkb::MOD_INVALID,
            mask: 0,
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

impl ModifierMap {
    fn new() -> Self {
        Self {
            ctrl: ModifierIndex::new(),
            shift: ModifierIndex::new(),
            alt: ModifierIndex::new(),
            meta: ModifierIndex::new(),
            caps_lock: ModifierIndex::new(),
            num_lock: ModifierIndex::new(),
            supr: ModifierIndex::new(),
            hyper: ModifierIndex::new(),
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
struct Algorithm {
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
}

impl Algorithm {
    fn new() -> Algorithm {
        return Algorithm {
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
        };
    }
}

// Algorithm for mapping virtual modifiers to real modifiers:
//   1. create new state
//   2. for each key in keymap
//      a. send key down to state
//      b. if it affected exactly one bit in modifier map
//         i) get keysym
//         ii) if keysym matches one of the known modifiers, save it for that modifier
//         iii) if modifier is latched, send key up and key down to toggle again
//      c. send key up to reset the state
//   3. if shift key found in step 2, run step 2 with all shift+key for each key
//   4. if shift, control, alt and super are not all found, declare failure
//   5. if failure, use static mapping from xkbcommon-names.h
//
// Step 3 is needed because many popular keymaps map meta to alt+shift.
//
// We could do better by constructing a system of linear equations, but it should not be
// needed in any sane system. We could also use this algorithm with X11, but X11
// provides XkbVirtualModsToReal which is guaranteed to be accurate, while this
// algorithm is only a heuristic.
//
// We don't touch level3 or level5 modifiers.
//
fn get_mapper_algo<'a>(
    algo: &'a mut Algorithm,
    state: &'a mut xkb::State,
) -> impl FnMut(&xkb::Keymap, xkb::Keycode) + 'a {
    return move |_: &xkb::Keymap, key: xkb::Keycode| {
        log::debug!("Key: {:?}", key);

        if algo.failed {
            return;
        }

        if algo.try_shift {
            if key == algo.shift_keycode {
                return;
            }

            state.update_key(algo.shift_keycode, xkb::KeyDirection::Down);
        }

        let changed_type = state.update_key(key, xkb::KeyDirection::Down);

        if changed_type
            & (xkb::STATE_MODS_DEPRESSED | xkb::STATE_MODS_LATCHED | xkb::STATE_MODS_LOCKED)
            != 0
        {
            let mods = state.serialize_mods(if algo.try_shift {
                xkb::STATE_MODS_EFFECTIVE
            } else {
                xkb::STATE_MODS_DEPRESSED | xkb::STATE_MODS_LATCHED | xkb::STATE_MODS_LOCKED
            });

            let keysyms = state.key_get_syms(key);
            log::debug!("Keysym: {:x?}", keysyms);

            macro_rules! S2 {
                ($key_l:expr, $key_r:expr, $mod:ident) => {
                    if keysyms[0] == $key_l || keysyms[0] == $key_r {
                        if algo.$mod == 0 {
                            log::debug!(
                                "Keycode {:?} triggered keysym '{:x?}' for {:?}' modifier.",
                                key,
                                keysyms[0],
                                stringify!($mod)
                            );
                            algo.$mod = mods;
                        } else if algo.$mod != mods {
                            log::debug!(
                                "Keycode {:?} triggered again keysym '{:x?}' for {:?}' modifier.",
                                key,
                                keysyms[0],
                                stringify!($mod)
                            );
                            algo.failed = true;
                        }
                    }
                };
            }

            macro_rules! S1 {
                ($key:expr, $mod:ident) => {
                    if keysyms[0] == $key && algo.$mod == 0 {
                        log::debug!(
                            "Key {:?} triggered keysym '{:?}' for '{:?}' modifier.",
                            key,
                            keysyms[0],
                            stringify!($mod)
                        );
                        algo.$mod = mods;
                    }
                };
            }

            // We can handle exactly one keysym with exactly one bit set in the implementation
            // below; with a lot more gymnastics, we could set up an 8x8 linear system and solve
            // for each modifier in case there are some modifiers that are only present in
            // combination with others, but it is not worth the effort.
            if keysyms.len() == 1 && mods.is_power_of_two() {
                S2!(xkb::keysyms::KEY_Shift_L, xkb::KEY_Shift_R, shift);
                S2!(xkb::KEY_Control_L, xkb::KEY_Control_R, ctrl);
                S1!(xkb::KEY_Caps_Lock, caps_lock);
                S1!(xkb::KEY_Shift_Lock, num_lock);
                S2!(xkb::KEY_Alt_L, xkb::KEY_Alt_R, alt);
                S2!(xkb::KEY_Meta_L, xkb::KEY_Meta_R, meta);
                S2!(xkb::KEY_Super_L, xkb::KEY_Super_R, supr);
                S2!(xkb::KEY_Hyper_L, xkb::KEY_Hyper_R, hyper);
            }

            if algo.shift_keycode == 0
                && (keysyms[0] == xkb::keysyms::KEY_Shift_L
                    || keysyms[0] == xkb::keysyms::KEY_Shift_R)
            {
                log::debug!("Found shift keycode.");
                algo.shift_keycode = key;
            }

            // If this is a lock, then up and down to remove lock state
            if changed_type & xkb::STATE_MODS_LOCKED != 0 {
                log::debug!("Found lock state. Set up/down to remove lock state.");
                state.update_key(key, xkb::KeyDirection::Up);
                state.update_key(key, xkb::KeyDirection::Down);
            }
        }

        state.update_key(key, xkb::KeyDirection::Up);

        if algo.try_shift {
            state.update_key(algo.shift_keycode, xkb::KeyDirection::Up);
        }

        return;
    };
}

fn init_modifier_table_fallback(keymap: &xkb::Keymap) -> ModifierMap {
    let mut mod_map = ModifierMap::new();

    macro_rules! assign {
        ($mod:ident, $n:expr) => {
            let idx = keymap.mod_get_index($n);
            mod_map.$mod = ModifierIndex {
                idx: idx,
                mask: 1 << idx,
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

#[cfg(feature = "x11")]
fn init_modifier_table(keymap: &xkb::Keymap) -> ModifierMap {
    // TODO: This implementation needs to be done with
    // https://github.com/kovidgoyal/kitty/blob/0248edbdb98cc3ae80d98bf5ad17fbf497a24a43/glfw/xkb_glfw.c#L321    return init_modifier_table_fallback(keymap);
}

#[cfg(feature = "wayland")]
pub fn init_modifier_table(keymap: &xkb::Keymap) -> ModifierMap {
    // Implementation taken from
    // https://github.com/kovidgoyal/kitty/blob/0248edbdb98cc3ae80d98bf5ad17fbf497a24a43/glfw/xkb_glfw.c#L523
    //
    // This is a client side hack for wayland, once
    // https://github.com/xkbcommon/libxkbcommon/pull/36
    // is implemented or some other solution exists
    // this code iterates through key presses to find the
    // activated modifiers.
    // Check: https://github.com/wez/wezterm/issues/4626

    log::info!("Detect modifiers on Wayland [with key press iterations].");

    let mut algo = Algorithm::new();
    let mut state: xkb::State = xkb::State::new(keymap);
    let mut mod_map = ModifierMap::new();

    keymap.key_for_each(get_mapper_algo(&mut algo, &mut state));

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
        keymap.key_for_each(get_mapper_algo(&mut algo, &mut state));
    }

    // We must have found a least those 4 modifiers.
    if !algo.failed && (algo.ctrl == 0 || algo.shift == 0 || algo.alt == 0 || algo.supr == 0) {
        log::debug!("Some of essential modifiers (ctrl, shift, alt, supr) have not been found.");
        algo.failed = true;
    }

    if !algo.failed {
        let mut shifted: u32 = 1;
        let mut used_bits: u32 = 0;

        macro_rules! assign {
            ($mod:ident, $i:ident) => {
                if mod_map.$mod.idx == xkb::MOD_INVALID
                    && (used_bits & shifted == 0)
                    && algo.$mod == shifted
                {
                    mod_map.$mod = ModifierIndex {
                        idx: $i,
                        mask: shifted,
                    };
                    used_bits |= shifted;
                }
            };
        }

        for i in 0..32 {
            assign!(ctrl, i);
            assign!(shift, i);
            assign!(alt, i);
            assign!(meta, i);
            assign!(caps_lock, i);
            assign!(num_lock, i);
            assign!(supr, i);
            assign!(hyper, i);
            shifted <<= 1;
        }
    } else {
        log::warn!("Detect modifiers on Wayland failed. Using default mapping.");
        mod_map = init_modifier_table_fallback(keymap);
    }

    log::info!(
        "Modifier map:\n
          - ctrl: {:?}
          - shift: {:?}
          - alt: {:?}
          - meta: {:?}
          - caps_lock: {:?}
          - num_lock: {:?}
          - super: {:?}
          - hyper: {:?}",
        mod_map.ctrl,
        mod_map.shift,
        mod_map.alt,
        mod_map.meta,
        mod_map.caps_lock,
        mod_map.num_lock,
        mod_map.supr,
        mod_map.hyper,
    );

    return mod_map;
}
