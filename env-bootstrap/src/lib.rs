pub mod ringlog;
pub use ringlog::setup_logger;
use std::path::{Path, PathBuf};

pub fn set_wezterm_executable() {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            std::env::set_var("WEZTERM_EXECUTABLE_DIR", dir);
        }
        std::env::set_var("WEZTERM_EXECUTABLE", exe);
    }
}

pub fn fixup_snap() {
    if std::env::var_os("SNAP").is_some() {
        // snapd sets a bunch of environment variables as a part of setup
        // These are not useful to be passed through to things spawned by us.

        // SNAP is the base path of the files in the snap
        std::env::remove_var("SNAP");

        // snapd also sets a bunch of SNAP_* environment variables
        // This list may change over time, so for simplicity, assume
        // anything in the SNAP_* namespace is set by snapd, and unset it.
        std::env::vars_os()
            .into_iter()
            .filter(|(key, _)| {
                key.to_str()
                    .filter(|key| key.starts_with("SNAP_"))
                    .is_some()
            })
            .map(|(key, _)| key)
            .for_each(|key| std::env::remove_var(key));

        // snapd has *also* set LD_LIBRARY_PATH, and things we spawn
        // *absolutely do not* want this propagated
        std::env::remove_var("LD_LIBRARY_PATH");
    }
}

pub fn fixup_appimage() {
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        let appimage = std::path::PathBuf::from(appimage);

        // We were started via an AppImage, presumably ourselves.
        // AppImage exports ARGV0 into the environment and that causes
        // everything that was indirectly spawned by us to appear to
        // be the AppImage.  eg: if you `vim foo` it shows as
        // `WezTerm.AppImage foo`, which is super confusing for everyone!
        // Let's just unset that from the environment!
        std::env::remove_var("ARGV0");

        // Since our AppImage includes multiple utilities, we want to
        // be able to use them, so add that location to the PATH!
        // WEZTERM_EXECUTABLE_DIR is set by `set_wezterm_executable`
        // which is called before `fixup_appimage`
        if let Some(dir) = std::env::var_os("WEZTERM_EXECUTABLE_DIR") {
            if let Some(path) = std::env::var_os("PATH") {
                let mut paths = std::env::split_paths(&path).collect::<Vec<_>>();
                paths.insert(0, PathBuf::from(dir));
                let new_path = std::env::join_paths(paths).expect("unable to update PATH");
                std::env::set_var("PATH", &new_path);
            }
        }

        // This AppImage feature allows redirecting HOME and XDG_CONFIG_HOME
        // to live alongside the executable for portable use:
        // https://github.com/AppImage/AppImageKit/issues/368
        // When we spawn children, we don't want them to inherit this,
        // but we do want to respect them for config loading.
        // Let's force resolution and cleanup our environment now.

        /// Given "/some/path.AppImage" produce "/some/path.AppImageSUFFIX".
        /// We only support this for "path.AppImage" that can be converted
        /// to UTF-8.  Otherwise, we return "/some/path.AppImage" unmodified.
        fn append_extra_file_name_suffix(p: &Path, suffix: &str) -> PathBuf {
            if let Some(name) = p.file_name().and_then(|o| o.to_str()) {
                p.with_file_name(format!("{}{}", name, suffix))
            } else {
                p.to_path_buf()
            }
        }

        /// Our config stuff exports these env vars to help portable apps locate
        /// the correct environment when it is launched via wezterm.
        /// However, if we are using the system wezterm to spawn a portable
        /// AppImage then we want these to not take effect.
        fn clean_wezterm_config_env() {
            std::env::remove_var("WEZTERM_CONFIG_FILE");
            std::env::remove_var("WEZTERM_CONFIG_DIR");
        }

        if config::HOME_DIR.starts_with(append_extra_file_name_suffix(&appimage, ".home")) {
            // Fixup HOME to point to the user's actual home dir
            std::env::remove_var("HOME");
            std::env::set_var(
                "HOME",
                dirs_next::home_dir().expect("can't resolve HOME dir"),
            );
            clean_wezterm_config_env();
        }

        if std::env::var("XDG_CONFIG_HOME")
            .map(|d| {
                PathBuf::from(d).starts_with(append_extra_file_name_suffix(&appimage, ".config"))
            })
            .unwrap_or_default()
        {
            std::env::remove_var("XDG_CONFIG_HOME");
            clean_wezterm_config_env();
        }
    }
}

/// If LANG isn't set in the environment, make an attempt at setting
/// it to a UTF-8 version of the current locale known to NSLocale.
#[cfg(target_os = "macos")]
pub fn set_lang_from_locale() {
    #![allow(unexpected_cfgs)] // <https://github.com/SSheldon/rust-objc/issues/125>
    use cocoa::base::id;
    use cocoa::foundation::NSString;
    use objc::runtime::Object;
    use objc::*;

    fn lang_is_set() -> bool {
        match std::env::var_os("LANG") {
            None => false,
            Some(lang) => !lang.is_empty(),
        }
    }

    if !lang_is_set() {
        unsafe fn nsstring_to_str<'a>(ns: *mut Object) -> &'a str {
            let data = NSString::UTF8String(ns as id) as *const u8;
            let len = NSString::len(ns as id);
            let bytes = std::slice::from_raw_parts(data, len);
            std::str::from_utf8_unchecked(bytes)
        }

        unsafe {
            let locale: *mut Object = msg_send![class!(NSLocale), autoupdatingCurrentLocale];
            let lang_code_obj: *mut Object = msg_send![locale, languageCode];
            let country_code_obj: *mut Object = msg_send![locale, countryCode];

            {
                let lang_code = nsstring_to_str(lang_code_obj);
                let country_code = nsstring_to_str(country_code_obj);

                let candidate = format!("{}_{}.UTF-8", lang_code, country_code);
                let candidate_cstr =
                    std::ffi::CString::new(candidate.as_bytes()).expect("make cstr from str");

                // If this looks like a working locale then export it to
                // the environment so that our child processes inherit it.
                let old = libc::setlocale(libc::LC_CTYPE, std::ptr::null());
                if !libc::setlocale(libc::LC_CTYPE, candidate_cstr.as_ptr()).is_null() {
                    std::env::set_var("LANG", &candidate);
                } else {
                    log::debug!("setlocale({}) failed, fall back to en_US.UTF-8", candidate);
                    std::env::set_var("LANG", "en_US.UTF-8");
                }
                libc::setlocale(libc::LC_CTYPE, old);
            }

            let _: () = msg_send![lang_code_obj, release];
            let _: () = msg_send![country_code_obj, release];
            let _: () = msg_send![locale, release];
        }
    }
}

fn register_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload();
        let payload = payload.downcast_ref::<&str>().unwrap_or(&"!?");
        let bt = backtrace::Backtrace::new();
        if let Some(loc) = info.location() {
            log::error!(
                "panic at {}:{}:{} - {}\n{:?}",
                loc.file(),
                loc.line(),
                loc.column(),
                payload,
                bt
            );
        } else {
            log::error!("panic - {}\n{:?}", payload, bt);
        }
        default_hook(info);
    }));
}

fn register_lua_modules() {
    for func in [
        battery::register,
        color_funcs::register,
        termwiz_funcs::register,
        logging::register,
        mux_lua::register,
        procinfo_funcs::register,
        filesystem::register,
        serde_funcs::register,
        plugin::register,
        ssh_funcs::register,
        spawn_funcs::register,
        share_data::register,
        time_funcs::register,
        url_funcs::register,
    ] {
        config::lua::add_context_setup_func(func);
    }
}

pub fn bootstrap() {
    config::assign_version_info(
        wezterm_version::wezterm_version(),
        wezterm_version::wezterm_target_triple(),
    );
    setup_logger();
    register_panic_hook();

    set_wezterm_executable();

    #[cfg(target_os = "macos")]
    set_lang_from_locale();

    fixup_appimage();
    fixup_snap();

    register_lua_modules();

    // Remove this env var to avoid weirdness with some vim configurations.
    // wezterm never sets WINDOWID and we don't want to inherit it from a
    // parent process.
    std::env::remove_var("WINDOWID");
    // Avoid vte shell integration kicking in if someone started
    // wezterm or the mux server from inside gnome terminal.
    // <https://github.com/wezterm/wezterm/issues/2237>
    std::env::remove_var("VTE_VERSION");

    // Sice folks don't like to reboot or sign out if they `chsh`,
    // SHELL may be stale. Rather than using a stale value, unset
    // it so that pty::CommandBuilder::get_shell will resolve the
    // shell from the password database instead.
    std::env::remove_var("SHELL");
}
