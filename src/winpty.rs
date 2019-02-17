use failure::Error;
use std::io;
use std::io::Error as IoError;
extern crate winapi;
use std::os::windows::raw::HANDLE;
use std::ptr;
use std::sync::{Arc, Mutex};
use winpty::winapi::shared::minwindef::DWORD;
use winpty::winapi::shared::winerror::{HRESULT, S_OK};
use winpty::winapi::um::fileapi::{ReadFile, WriteFile};
use winpty::winapi::um::handleapi::*;
use winpty::winapi::um::namedpipeapi::CreatePipe;
use winpty::winapi::um::wincon::COORD;

pub use std::process::{Child, Command};

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

struct OwnedHandle {
    handle: HANDLE,
}
unsafe impl Send for OwnedHandle {}
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.handle) };
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
        bail!("spawn_command not implemented")
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
