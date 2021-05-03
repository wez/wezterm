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

        if config::CONFIG_DIR.starts_with(append_extra_file_name_suffix(&appimage, ".config")) {
            std::env::remove_var("XDG_CONFIG_HOME");
            clean_wezterm_config_env();
        }
    }
}

/// If LANG isn't set in the environment, make an attempt at setting
/// it to a UTF-8 version of the current locale known to NSLocale.
#[cfg(target_os = "macos")]
pub fn set_lang_from_locale() {
    use cocoa::base::id;
    use cocoa::foundation::NSString;
    use objc::runtime::Object;
    use objc::*;

    if std::env::var_os("LANG").is_none() {
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
                let candidate_cstr = std::ffi::CString::new(candidate.as_bytes().clone())
                    .expect("make cstr from str");

                // If this looks like a working locale then export it to
                // the environment so that our child processes inherit it.
                let old = libc::setlocale(libc::LC_CTYPE, std::ptr::null());
                if !libc::setlocale(libc::LC_CTYPE, candidate_cstr.as_ptr()).is_null() {
                    std::env::set_var("LANG", &candidate);
                }
                libc::setlocale(libc::LC_CTYPE, old);
            }

            let _: () = msg_send![lang_code_obj, release];
            let _: () = msg_send![country_code_obj, release];
            let _: () = msg_send![locale, release];
        }
    }
}

pub fn bootstrap() {
    set_wezterm_executable();

    #[cfg(target_os = "macos")]
    set_lang_from_locale();

    fixup_appimage();

    setup_logger();
}
