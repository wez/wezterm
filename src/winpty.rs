use failure::Error;
use std::io::{self, Error as IoError, Result as IoResult};
extern crate winapi;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::raw::HANDLE;
use std::ptr;
use std::sync::{Arc, Mutex};
use winpty::winapi::shared::minwindef::DWORD;
use winpty::winapi::shared::winerror::{HRESULT, S_OK};
use winpty::winapi::um::fileapi::{ReadFile, WriteFile};
use winpty::winapi::um::handleapi::*;
use winpty::winapi::um::namedpipeapi::CreatePipe;
use winpty::winapi::um::processthreadsapi::*;
use winpty::winapi::um::winbase::EXTENDED_STARTUPINFO_PRESENT;
use winpty::winapi::um::winbase::STARTUPINFOEXW;
use winpty::winapi::um::wincon::COORD;

const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x00020016;

#[derive(Debug)]
pub struct Command {
    args: Vec<OsString>,
    input: Option<OwnedHandle>,
    output: Option<OwnedHandle>,
    hpc: Option<HPCON>,
}

impl Command {
    pub fn new<S: AsRef<OsStr>>(program: S) -> Self {
        Self {
            args: vec![Self::search_path(program.as_ref().to_owned())],
            input: None,
            output: None,
            hpc: None,
        }
    }

    fn search_path(exe: OsString) -> OsString {
        if let Some(path) = env::var_os("PATH") {
            let extensions = env::var_os("PATHEXT").unwrap_or(".EXE".into());
            for path in env::split_paths(&path) {
                // Check for exactly the user's string in this path dir
                let candidate = path.join(&exe);
                if fs::metadata(&candidate).is_ok() {
                    return candidate.into_os_string();
                }

                // otherwise try tacking on some extensions.
                // Note that this really replaces the extension in the
                // user specified path, so this is potentially wrong.
                for ext in env::split_paths(&extensions) {
                    // PATHEXT includes the leading `.`, but `with_extension`
                    // doesn't want that
                    let ext = ext.to_str().expect("PATHEXT entries must be utf8");
                    let path = path.join(&exe).with_extension(&ext[1..]);
                    if fs::metadata(&path).is_ok() {
                        return path.into_os_string();
                    }
                }
            }
        }

        exe
    }

    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Command {
        // FIXME: quoting!
        self.args.push(arg.as_ref().to_owned());
        self
    }

    pub fn args<I, S>(&mut self, args: I) -> &mut Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for arg in args {
            self.arg(arg);
        }
        self
    }

    pub fn env<K, V>(&mut self, key: K, val: V) -> &mut Command
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        eprintln!(
            "ignoring env {:?}={:?} for child; FIXME: implement this!",
            key.as_ref(),
            val.as_ref()
        );
        self
    }

    fn set_pty(&mut self, input: OwnedHandle, output: OwnedHandle, con: HPCON) -> &mut Command {
        self.input.replace(input);
        self.output.replace(output);
        self.hpc.replace(con);
        self
    }

    fn cmdline(&self) -> Result<Vec<u16>, Error> {
        let mut cmdline = Vec::<u16>::new();
        for (idx, arg) in self.args.iter().enumerate() {
            if idx != 0 {
                cmdline.push(' ' as u16);
            }
            ensure!(
                !arg.encode_wide().any(|c| c == 0),
                "invalid encoding for command line argument at index {}: {:?}",
                idx,
                arg
            );
            Self::append_quoted(arg, &mut cmdline);
        }
        Ok(cmdline)
    }

    // Borrowed from https://github.com/hniksic/rust-subprocess/blob/873dfed165173e52907beb87118b2c0c05d8b8a1/src/popen.rs#L1117
    // which in turn was translated from ArgvQuote at http://tinyurl.com/zmgtnls
    fn append_quoted(arg: &OsStr, cmdline: &mut Vec<u16>) {
        if !arg.is_empty()
            && !arg.encode_wide().any(|c| {
                c == ' ' as u16
                    || c == '\t' as u16
                    || c == '\n' as u16
                    || c == '\x0b' as u16
                    || c == '\"' as u16
            })
        {
            cmdline.extend(arg.encode_wide());
            return;
        }
        cmdline.push('"' as u16);

        let arg: Vec<_> = arg.encode_wide().collect();
        let mut i = 0;
        while i < arg.len() {
            let mut num_backslashes = 0;
            while i < arg.len() && arg[i] == '\\' as u16 {
                i += 1;
                num_backslashes += 1;
            }

            if i == arg.len() {
                for _ in 0..num_backslashes * 2 {
                    cmdline.push('\\' as u16);
                }
                break;
            } else if arg[i] == b'"' as u16 {
                for _ in 0..num_backslashes * 2 + 1 {
                    cmdline.push('\\' as u16);
                }
                cmdline.push(arg[i]);
            } else {
                for _ in 0..num_backslashes {
                    cmdline.push('\\' as u16);
                }
                cmdline.push(arg[i]);
            }
            i += 1;
        }
        cmdline.push('"' as u16);
    }

    pub fn spawn(&mut self) -> Result<Child, Error> {
        let mut si: STARTUPINFOEXW = unsafe { mem::zeroed() };
        si.StartupInfo.cb = mem::size_of::<STARTUPINFOEXW>() as u32;

        let mut attrs = ProcThreadAttributeList::with_capacity(1)?;
        attrs.set_pty(*self.hpc.as_ref().unwrap())?;
        si.lpAttributeList = attrs.as_mut_ptr();

        let mut pi: PROCESS_INFORMATION = unsafe { mem::zeroed() };

        let mut cmdline = self.cmdline()?;
        let res = unsafe {
            CreateProcessW(
                ptr::null(),
                cmdline.as_mut_slice().as_mut_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                EXTENDED_STARTUPINFO_PRESENT,
                ptr::null_mut(), // FIXME: env
                ptr::null_mut(),
                &mut si.StartupInfo,
                &mut pi,
            )
        };
        if res == 0 {
            bail!(
                "CreateProcessW `{:?}` failed: {}",
                OsString::from_wide(&cmdline),
                IoError::last_os_error()
            );
        }

        // Make sure we close out the thread handle so we don't leak it;
        // we do this simply by making it owned
        let _main_thread = OwnedHandle { handle: pi.hThread };
        let proc = OwnedHandle {
            handle: pi.hProcess,
        };

        Ok(Child { proc })
    }
}

struct ProcThreadAttributeList {
    data: Vec<u8>,
}

impl ProcThreadAttributeList {
    pub fn with_capacity(num_attributes: DWORD) -> Result<Self, Error> {
        let mut bytes_required: usize = 0;
        unsafe {
            InitializeProcThreadAttributeList(
                ptr::null_mut(),
                num_attributes,
                0,
                &mut bytes_required,
            )
        };
        let mut data = Vec::with_capacity(bytes_required);
        // We have the right capacity, so force the vec to consider itself
        // that length.  The contents of those bytes will be maintained
        // by the win32 apis used in this impl.
        unsafe { data.set_len(bytes_required) };

        let attr_ptr = data.as_mut_slice().as_mut_ptr() as *mut _;
        let res = unsafe {
            InitializeProcThreadAttributeList(attr_ptr, num_attributes, 0, &mut bytes_required)
        };
        ensure!(
            res != 0,
            "InitializeProcThreadAttributeList failed: {}",
            IoError::last_os_error()
        );
        Ok(Self { data })
    }

    pub fn as_mut_ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.data.as_mut_slice().as_mut_ptr() as *mut _
    }

    pub fn set_pty(&mut self, con: HPCON) -> Result<(), Error> {
        let res = unsafe {
            UpdateProcThreadAttribute(
                self.as_mut_ptr(),
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
                con,
                mem::size_of::<HPCON>(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        ensure!(
            res != 0,
            "UpdateProcThreadAttribute failed: {}",
            IoError::last_os_error()
        );
        Ok(())
    }
}

impl Drop for ProcThreadAttributeList {
    fn drop(&mut self) {
        unsafe { DeleteProcThreadAttributeList(self.as_mut_ptr()) };
    }
}

#[derive(Debug)]
pub struct Child {
    proc: OwnedHandle,
}

impl Child {
    pub fn try_wait(&mut self) -> IoResult<Option<ExitStatus>> {
        let mut status: DWORD = 0;
        let res = unsafe { GetExitCodeProcess(self.proc.handle, &mut status) };
        if res != 0 {
            Ok(Some(ExitStatus { status }))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug)]
pub struct ExitStatus {
    status: DWORD,
}

type HPCON = HANDLE;

extern "system" {
    fn CreatePseudoConsole(
        size: COORD,
        hInput: HANDLE,
        hOutput: HANDLE,
        flags: DWORD,
        hpc: *mut HPCON,
    ) -> HRESULT;
    fn ResizePseudoConsole(hpc: HPCON, size: COORD) -> HRESULT;
    fn ClosePseudoConsole(hpc: HPCON);
}

struct PsuedoCon {
    con: HPCON,
}
unsafe impl Send for PsuedoCon {}
unsafe impl Sync for PsuedoCon {}
impl Drop for PsuedoCon {
    fn drop(&mut self) {
        unsafe { ClosePseudoConsole(self.con) };
    }
}
impl PsuedoCon {
    fn new(size: COORD, input: &OwnedHandle, output: &OwnedHandle) -> Result<Self, Error> {
        let mut con: HPCON = INVALID_HANDLE_VALUE;
        let result = unsafe { CreatePseudoConsole(size, input.handle, output.handle, 0, &mut con) };
        ensure!(
            result == S_OK,
            "failed to create psuedo console: HRESULT {}",
            result
        );
        Ok(Self { con })
    }
    fn resize(&self, size: COORD) -> Result<(), Error> {
        let result = unsafe { ResizePseudoConsole(self.con, size) };
        ensure!(
            result == S_OK,
            "failed to resize console to {}x{}: HRESULT: {}",
            size.X,
            size.Y,
            result
        );
        Ok(())
    }
}

#[derive(Debug)]
struct OwnedHandle {
    handle: HANDLE,
}
unsafe impl Send for OwnedHandle {}
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.handle) };
    }
}

impl OwnedHandle {
    fn try_clone(&self) -> Result<Self, IoError> {
        let proc = unsafe { GetCurrentProcess() };
        let mut duped = INVALID_HANDLE_VALUE;
        let ok = unsafe {
            DuplicateHandle(
                proc,
                self.handle as *mut _,
                proc,
                &mut duped,
                0,
                0,
                winapi::um::winnt::DUPLICATE_SAME_ACCESS,
            )
        };
        if ok == 0 {
            Err(IoError::last_os_error())
        } else {
            Ok(OwnedHandle {
                handle: duped as *mut _,
            })
        }
    }
}

struct Inner {
    con: PsuedoCon,
    readable: OwnedHandle,
    writable: OwnedHandle,
    size: winsize,
}

impl Inner {
    pub fn resize(
        &mut self,
        num_rows: u16,
        num_cols: u16,
        pixel_width: u16,
        pixel_height: u16,
    ) -> Result<(), Error> {
        self.con.resize(COORD {
            X: num_cols as i16,
            Y: num_rows as i16,
        })?;
        self.size = winsize {
            ws_row: num_rows,
            ws_col: num_cols,
            ws_xpixel: pixel_width,
            ws_ypixel: pixel_height,
        };
        Ok(())
    }
}

#[derive(Clone)]
pub struct MasterPty {
    inner: Arc<Mutex<Inner>>,
}

pub struct SlavePty {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct winsize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

impl MasterPty {
    pub fn resize(
        &self,
        num_rows: u16,
        num_cols: u16,
        pixel_width: u16,
        pixel_height: u16,
    ) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();
        inner.resize(num_rows, num_cols, pixel_width, pixel_height)
    }

    pub fn get_size(&self) -> Result<winsize, Error> {
        let inner = self.inner.lock().unwrap();
        Ok(inner.size.clone())
    }

    pub fn try_clone(&self) -> Result<Self, Error> {
        Ok(Self {
            inner: self.inner.clone(),
        })
    }

    pub fn clear_nonblocking(&self) -> Result<(), Error> {
        unimplemented!();
    }
}

impl io::Write for MasterPty {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        let mut num_wrote = 0;
        let ok = unsafe {
            WriteFile(
                self.inner.lock().unwrap().writable.handle as *mut _,
                buf.as_ptr() as *const _,
                buf.len() as u32,
                &mut num_wrote,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(IoError::last_os_error())
        } else {
            Ok(num_wrote as usize)
        }
    }
    fn flush(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}

impl io::Read for MasterPty {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        let mut num_read = 0;
        let ok = unsafe {
            ReadFile(
                self.inner.lock().unwrap().readable.handle as *mut _,
                buf.as_mut_ptr() as *mut _,
                buf.len() as u32,
                &mut num_read,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(IoError::last_os_error())
        } else {
            Ok(num_read as usize)
        }
    }
}

impl SlavePty {
    pub fn spawn_command(self, mut cmd: Command) -> Result<Child, Error> {
        let inner = self.inner.lock().unwrap();
        cmd.set_pty(
            inner.writable.try_clone()?,
            inner.readable.try_clone()?,
            inner.con.con,
        );

        cmd.spawn()
    }
}

fn pipe() -> Result<(OwnedHandle, OwnedHandle), Error> {
    let mut read: HANDLE = INVALID_HANDLE_VALUE;
    let mut write: HANDLE = INVALID_HANDLE_VALUE;
    if unsafe { CreatePipe(&mut read, &mut write, ptr::null_mut(), 0) } == 0 {
        bail!("CreatePipe failed: {}", IoError::last_os_error());
    }
    Ok((OwnedHandle { handle: read }, OwnedHandle { handle: write }))
}

pub fn openpty(
    num_rows: u16,
    num_cols: u16,
    pixel_width: u16,
    pixel_height: u16,
) -> Result<(MasterPty, SlavePty), Error> {
    let (stdin_read, stdin_write) = pipe()?;
    let (stdout_read, stdout_write) = pipe()?;

    let con = PsuedoCon::new(
        COORD {
            X: num_cols as i16,
            Y: num_rows as i16,
        },
        &stdin_read,
        &stdout_write,
    )?;

    let size = winsize {
        ws_row: num_rows,
        ws_col: num_cols,
        ws_xpixel: pixel_width,
        ws_ypixel: pixel_height,
    };

    let master = MasterPty {
        inner: Arc::new(Mutex::new(Inner {
            con,
            readable: stdout_read,
            writable: stdin_write,
            size,
        })),
    };

    let slave = SlavePty {
        inner: master.inner.clone(),
    };

    Ok((master, slave))
}
