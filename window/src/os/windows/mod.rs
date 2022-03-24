pub mod connection;
pub mod event;
mod extra_constants;
mod keycodes;
mod wgl;
pub mod window;

pub use self::window::*;
pub use connection::*;
pub use event::*;

/// Convert a rust string to a windows wide string
pub fn wide_string(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Returns true if we are running in an RDP session.
/// See <https://docs.microsoft.com/en-us/windows/win32/termserv/detecting-the-terminal-services-environment>
pub fn is_running_in_rdp_session() -> bool {
    use winapi::shared::minwindef::DWORD;
    use winapi::um::processthreadsapi::{GetCurrentProcessId, ProcessIdToSessionId};
    use winapi::um::winuser::{GetSystemMetrics, SM_REMOTESESSION};
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    if unsafe { GetSystemMetrics(SM_REMOTESESSION) } != 0 {
        return true;
    }

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let terminal_server =
        match hklm.open_subkey("SYSTEM\\CurrentControlSet\\Control\\Terminal Server\\") {
            Ok(k) => k,
            Err(_) => return false,
        };

    let glass_session_id: DWORD = match terminal_server.get_value("GlassSessionId") {
        Ok(sess) => sess,
        Err(_) => return false,
    };

    unsafe {
        let mut current_session = 0;
        if ProcessIdToSessionId(GetCurrentProcessId(), &mut current_session) != 0 {
            // If we're not the glass session then we're a remote session
            current_session != glass_session_id
        } else {
            false
        }
    }
}
