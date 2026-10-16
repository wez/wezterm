use super::WinChild;
use crate::cmdbuilder::CommandBuilder;
use crate::win::procthreadattr::ProcThreadAttributeList;
use anyhow::{bail, ensure, Error};
use filedescriptor::{FileDescriptor, OwnedHandle};
use lazy_static::lazy_static;
use shared_library::shared_library;
use std::ffi::OsString;
use std::io::Error as IoError;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::{mem, ptr};
use windows_sys::core::HRESULT;
use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE, S_OK};
use windows_sys::Win32::System::Console::COORD;
use windows_sys::Win32::System::Threading::{
    CreateProcessW, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, PROCESS_INFORMATION,
    STARTF_USESTDHANDLES, STARTUPINFOEXW,
};

pub type HPCON = HANDLE;

pub const PSEUDOCONSOLE_INHERIT_CURSOR: u32 = 0x1;
pub const PSEUDOCONSOLE_RESIZE_QUIRK: u32 = 0x2;
pub const PSEUDOCONSOLE_WIN32_INPUT_MODE: u32 = 0x4;
pub const PSEUDOCONSOLE_PASSTHROUGH_MODE: u32 = 0x8;

shared_library!(ConPtyFuncs,
    pub fn CreatePseudoConsole(
        size: COORD,
        hInput: HANDLE,
        hOutput: HANDLE,
        flags: u32,
        hpc: *mut HPCON
    ) -> HRESULT,
    pub fn ResizePseudoConsole(hpc: HPCON, size: COORD) -> HRESULT,
    pub fn ClosePseudoConsole(hpc: HPCON),
);

struct ConPtySelection {
    funcs: Arc<ConPtyFuncs>,
    passthrough: bool,
}

fn load_default_conpty() -> ConPtySelection {
    // If the kernel doesn't export these functions then their system is
    // too old and we cannot run.
    let kernel = ConPtyFuncs::open(Path::new("kernel32.dll")).expect(
        "this system does not support conpty.  Windows 10 October 2018 or newer is required",
    );

    // We prefer to use a sideloaded conpty.dll and openconsole.exe host deployed
    // alongside the application.  We check for this after checking for kernel
    // support so that we don't try to proceed and do something crazy.
    if let Ok(sideloaded) = ConPtyFuncs::open(Path::new("conpty.dll")) {
        ConPtySelection {
            funcs: Arc::new(sideloaded),
            passthrough: false,
        }
    } else {
        ConPtySelection {
            funcs: Arc::new(kernel),
            passthrough: false,
        }
    }
}

lazy_static! {
    static ref DEFAULT_CONPTY: ConPtySelection = load_default_conpty();
    static ref CONFIGURED_CONPTY: Mutex<Option<ConPtySelection>> = Mutex::new(None);
}

/// Select a redistributable ConPTY DLL for subsequently created pseudo consoles.
///
/// The matching `OpenConsole.exe` must be in the same directory. Existing
/// pseudo consoles retain the library they were created with, so applications
/// may safely change this setting while other sessions are running.
///
/// The redistributable is opened with `PSEUDOCONSOLE_PASSTHROUGH_MODE`. The
/// inbox and automatically sideloaded ConPTY libraries keep the historical
/// flags.
pub fn set_conpty_path(path: PathBuf) -> Result<(), shared_library::LoadingError> {
    let funcs = Arc::new(ConPtyFuncs::open(&path)?);
    *CONFIGURED_CONPTY.lock().unwrap() = Some(ConPtySelection {
        funcs,
        passthrough: true,
    });
    Ok(())
}

/// Use the default system or application-local ConPTY for new pseudo consoles.
pub fn clear_conpty_path() {
    *CONFIGURED_CONPTY.lock().unwrap() = None;
}

fn selected_conpty() -> ConPtySelection {
    if let Some(configured) = CONFIGURED_CONPTY.lock().unwrap().as_ref() {
        return ConPtySelection {
            funcs: configured.funcs.clone(),
            passthrough: configured.passthrough,
        };
    }
    ConPtySelection {
        funcs: DEFAULT_CONPTY.funcs.clone(),
        passthrough: DEFAULT_CONPTY.passthrough,
    }
}

pub struct PseudoCon {
    con: HPCON,
    funcs: Arc<ConPtyFuncs>,
}

unsafe impl Send for PseudoCon {}
unsafe impl Sync for PseudoCon {}

impl Drop for PseudoCon {
    fn drop(&mut self) {
        unsafe { (self.funcs.ClosePseudoConsole)(self.con) };
    }
}

impl PseudoCon {
    pub fn new(size: COORD, input: FileDescriptor, output: FileDescriptor) -> Result<Self, Error> {
        let mut con: HPCON = INVALID_HANDLE_VALUE;
        let selected = selected_conpty();
        let flags = PSEUDOCONSOLE_INHERIT_CURSOR
            | PSEUDOCONSOLE_RESIZE_QUIRK
            | PSEUDOCONSOLE_WIN32_INPUT_MODE
            | if selected.passthrough {
                PSEUDOCONSOLE_PASSTHROUGH_MODE
            } else {
                0
            };
        let result = unsafe {
            (selected.funcs.CreatePseudoConsole)(
                size,
                input.as_raw_handle(),
                output.as_raw_handle(),
                flags,
                &mut con,
            )
        };
        ensure!(
            result == S_OK,
            "failed to create pseudo console: HRESULT {}",
            result
        );
        Ok(Self {
            con,
            funcs: selected.funcs,
        })
    }

    pub fn resize(&self, size: COORD) -> Result<(), Error> {
        let result = unsafe { (self.funcs.ResizePseudoConsole)(self.con, size) };
        ensure!(
            result == S_OK,
            "failed to resize console to {}x{}: HRESULT: {}",
            size.X,
            size.Y,
            result
        );
        Ok(())
    }

    pub fn spawn_command(&self, cmd: CommandBuilder) -> anyhow::Result<WinChild> {
        let mut si: STARTUPINFOEXW = unsafe { mem::zeroed() };
        si.StartupInfo.cb = mem::size_of::<STARTUPINFOEXW>() as u32;
        // Explicitly set the stdio handles as invalid handles otherwise
        // we can end up with a weird state where the spawned process can
        // inherit the explicitly redirected output handles from its parent.
        // For example, when daemonizing wezterm-mux-server, the stdio handles
        // are redirected to a log file and the spawned process would end up
        // writing its output there instead of to the pty we just created.
        si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        si.StartupInfo.hStdInput = INVALID_HANDLE_VALUE;
        si.StartupInfo.hStdOutput = INVALID_HANDLE_VALUE;
        si.StartupInfo.hStdError = INVALID_HANDLE_VALUE;

        let mut attrs = ProcThreadAttributeList::with_capacity(1)?;
        attrs.set_pty(self.con)?;
        si.lpAttributeList = attrs.as_mut_ptr();

        let mut pi: PROCESS_INFORMATION = unsafe { mem::zeroed() };

        let (mut exe, mut cmdline) = cmd.cmdline()?;
        let cmd_os = OsString::from_wide(&cmdline);

        let cwd = cmd.current_directory();

        let res = unsafe {
            CreateProcessW(
                exe.as_mut_slice().as_mut_ptr(),
                cmdline.as_mut_slice().as_mut_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
                cmd.environment_block().as_mut_slice().as_mut_ptr() as *mut _,
                cwd.as_ref()
                    .map(|c| c.as_slice().as_ptr())
                    .unwrap_or(ptr::null()),
                &mut si.StartupInfo,
                &mut pi,
            )
        };
        if res == 0 {
            let err = IoError::last_os_error();
            let msg = format!(
                "CreateProcessW `{:?}` in cwd `{:?}` failed: {}",
                cmd_os,
                cwd.as_ref().map(|c| OsString::from_wide(c)),
                err
            );
            log::error!("{}", msg);
            bail!("{}", msg);
        }

        // Make sure we close out the thread handle so we don't leak it;
        // we do this simply by making it owned
        let _main_thread = unsafe { OwnedHandle::from_raw_handle(pi.hThread as _) };
        let proc = unsafe { OwnedHandle::from_raw_handle(pi.hProcess as _) };

        Ok(WinChild {
            proc: Mutex::new(proc),
        })
    }
}
