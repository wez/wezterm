//! A type-safe wrapper around the sys module, which in turn exposes
//! the API exported by winpty.dll.
//! https://github.com/rprichard/winpty/blob/master/src/include/winpty.h
#![allow(dead_code)]
use super::sys::*;
use crate::win::ownedhandle::OwnedHandle;
use bitflags::bitflags;
use failure::{bail, ensure, format_err, Error};
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::ptr;
use winapi::shared::minwindef::DWORD;
use winapi::shared::ntdef::LPCWSTR;
use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::winbase::INFINITE;
use winapi::um::winnt::HANDLE;
use winapi::um::winnt::{GENERIC_READ, GENERIC_WRITE};

bitflags! {
    pub struct AgentFlags : u64 {
        const CONERR = WINPTY_FLAG_CONERR;
        const PLAIN_OUTPUT = WINPTY_FLAG_PLAIN_OUTPUT;
        const COLOR_ESCAPES = WINPTY_FLAG_COLOR_ESCAPES;
        const ALLOW_DESKTOP_CREATE = WINPTY_FLAG_ALLOW_CURPROC_DESKTOP_CREATION;
    }
}
bitflags! {
    pub struct SpawnFlags : u64 {
        const AUTO_SHUTDOWN = WINPTY_SPAWN_FLAG_AUTO_SHUTDOWN;
        const EXIT_AFTER_SHUTDOWN = WINPTY_SPAWN_FLAG_EXIT_AFTER_SHUTDOWN;
    }
}

#[repr(u32)]
pub enum MouseMode {
    None = WINPTY_MOUSE_MODE_NONE,
    Auto = WINPTY_MOUSE_MODE_AUTO,
    Force = WINPTY_MOUSE_MODE_FORCE,
}

pub enum Timeout {
    Infinite,
    Milliseconds(DWORD),
}

pub struct WinPtyConfig {
    config: *mut winpty_config_t,
}

fn wstr_to_osstr(wstr: LPCWSTR) -> Result<OsString, Error> {
    ensure!(!wstr.is_null(), "LPCWSTR is null");
    let slice = unsafe { std::slice::from_raw_parts(wstr, libc::wcslen(wstr)) };
    Ok(OsString::from_wide(slice))
}

fn wstr_to_string(wstr: LPCWSTR) -> Result<String, Error> {
    ensure!(!wstr.is_null(), "LPCWSTR is null");
    let slice = unsafe { std::slice::from_raw_parts(wstr, libc::wcslen(wstr)) };
    String::from_utf16(slice).map_err(|e| format_err!("String::from_utf16: {}", e))
}

fn check_err<T>(err: winpty_error_ptr_t, value: T) -> Result<T, Error> {
    if err.is_null() {
        return Ok(value);
    }
    unsafe {
        let code = (WINPTY.winpty_error_code)(err);
        if code == WINPTY_ERROR_SUCCESS {
            return Ok(value);
        }

        let converted = wstr_to_string((WINPTY.winpty_error_msg)(err))?;
        (WINPTY.winpty_error_free)(err);
        bail!("winpty error code {}: {}", code, converted)
    }
}

impl WinPtyConfig {
    pub fn new(flags: AgentFlags) -> Result<Self, Error> {
        let mut err: winpty_error_ptr_t = ptr::null_mut();
        let config = unsafe { (WINPTY.winpty_config_new)(flags.bits(), &mut err) };
        let config = check_err(err, config)?;
        ensure!(
            !config.is_null(),
            "winpty_config_new returned nullptr but no error"
        );
        Ok(Self { config })
    }

    pub fn set_initial_size(&mut self, cols: c_int, rows: c_int) {
        unsafe { (WINPTY.winpty_config_set_initial_size)(self.config, cols, rows) }
    }

    pub fn set_mouse_mode(&mut self, mode: MouseMode) {
        unsafe { (WINPTY.winpty_config_set_mouse_mode)(self.config, mode as c_int) }
    }

    pub fn set_agent_timeout(&mut self, timeout: Timeout) {
        let duration = match timeout {
            Timeout::Infinite => INFINITE,
            Timeout::Milliseconds(n) => n,
        };
        unsafe { (WINPTY.winpty_config_set_agent_timeout)(self.config, duration) }
    }

    pub fn open(&self) -> Result<WinPty, Error> {
        let mut err: winpty_error_ptr_t = ptr::null_mut();
        let pty = unsafe { (WINPTY.winpty_open)(self.config, &mut err) };
        let pty = check_err(err, pty)?;
        ensure!(!pty.is_null(), "winpty_open returned nullptr but no error");
        Ok(WinPty { pty })
    }
}

impl Drop for WinPtyConfig {
    fn drop(&mut self) {
        unsafe { (WINPTY.winpty_config_free)(self.config) }
    }
}

pub struct WinPty {
    pty: *mut winpty_t,
}

impl Drop for WinPty {
    fn drop(&mut self) {
        unsafe { (WINPTY.winpty_free)(self.pty) }
    }
}

fn pipe_client(name: LPCWSTR, for_read: bool) -> Result<OwnedHandle, Error> {
    let handle = unsafe {
        CreateFileW(
            name,
            if for_read {
                GENERIC_READ
            } else {
                GENERIC_WRITE
            },
            0,
            ptr::null_mut(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        let err = std::io::Error::last_os_error();
        bail!("failed to open {:?}: {}", wstr_to_string(name), err);
    } else {
        Ok(OwnedHandle::new(handle))
    }
}

impl WinPty {
    pub fn agent_process(&self) -> HANDLE {
        unsafe { (WINPTY.winpty_agent_process)(self.pty) }
    }

    pub fn conin(&self) -> Result<OwnedHandle, Error> {
        pipe_client(unsafe { (WINPTY.winpty_conin_name)(self.pty) }, false)
    }

    pub fn conout(&self) -> Result<OwnedHandle, Error> {
        pipe_client(unsafe { (WINPTY.winpty_conout_name)(self.pty) }, true)
    }

    pub fn conerr(&self) -> Result<OwnedHandle, Error> {
        pipe_client(unsafe { (WINPTY.winpty_conerr_name)(self.pty) }, true)
    }

    pub fn set_size(&mut self, cols: c_int, rows: c_int) -> Result<bool, Error> {
        let mut err: winpty_error_ptr_t = ptr::null_mut();
        let result = unsafe { (WINPTY.winpty_set_size)(self.pty, cols, rows, &mut err) };
        Ok(result != 0)
    }

    pub fn spawn(&mut self, config: &SpawnConfig) -> Result<SpawnedProcess, Error> {
        let mut err: winpty_error_ptr_t = ptr::null_mut();
        let mut create_process_error: DWORD = 0;
        let mut process_handle: HANDLE = ptr::null_mut();
        let mut thread_handle: HANDLE = ptr::null_mut();

        let result = unsafe {
            (WINPTY.winpty_spawn)(
                self.pty,
                config.spawn_config,
                &mut process_handle,
                &mut thread_handle,
                &mut create_process_error,
                &mut err,
            )
        };
        let thread_handle = OwnedHandle::new(thread_handle);
        let process_handle = OwnedHandle::new(process_handle);
        let result = check_err(err, result)?;
        if result == 0 {
            let err = std::io::Error::from_raw_os_error(create_process_error as _);
            bail!("winpty_spawn failed: {}", err);
        }
        Ok(SpawnedProcess {
            thread_handle,
            process_handle,
        })
    }
}

pub struct SpawnedProcess {
    pub process_handle: OwnedHandle,
    pub thread_handle: OwnedHandle,
}

pub struct SpawnConfig {
    spawn_config: *mut winpty_spawn_config_t,
}

/// Construct a null terminated wide string from an OsStr
fn str_to_wide(s: &OsStr) -> Vec<u16> {
    let mut wide: Vec<u16> = s.encode_wide().collect();
    wide.push(0);
    wide
}

fn str_ptr(s: &Option<Vec<u16>>) -> LPCWSTR {
    match s {
        None => ptr::null(),
        Some(v) => v.as_ptr(),
    }
}

impl SpawnConfig {
    pub fn with_os_str_args(
        flags: SpawnFlags,
        appname: Option<&OsStr>,
        cmdline: Option<&OsStr>,
        cwd: Option<&OsStr>,
        env: Option<&OsStr>,
    ) -> Result<Self, Error> {
        let appname = appname.map(str_to_wide);
        let cmdline = cmdline.map(str_to_wide);
        let cwd = cwd.map(str_to_wide);
        let env = env.map(str_to_wide);
        Self::new(flags, appname, cmdline, cwd, env)
    }

    pub fn new(
        flags: SpawnFlags,
        appname: Option<Vec<u16>>,
        cmdline: Option<Vec<u16>>,
        cwd: Option<Vec<u16>>,
        env: Option<Vec<u16>>,
    ) -> Result<Self, Error> {
        let mut err: winpty_error_ptr_t = ptr::null_mut();

        let spawn_config = unsafe {
            (WINPTY.winpty_spawn_config_new)(
                flags.bits(),
                str_ptr(&appname),
                str_ptr(&cmdline),
                str_ptr(&cwd),
                str_ptr(&env),
                &mut err,
            )
        };
        let spawn_config = check_err(err, spawn_config)?;
        ensure!(
            !spawn_config.is_null(),
            "winpty_spawn_config_new returned nullptr but no error"
        );
        Ok(Self { spawn_config })
    }
}

impl Drop for SpawnConfig {
    fn drop(&mut self) {
        unsafe { (WINPTY.winpty_spawn_config_free)(self.spawn_config) }
    }
}
