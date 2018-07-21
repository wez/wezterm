use failure::Error;
use istty::IsTty;
use std::io::{stdin, stdout};
use std::io::{Error as IOError, Read, Result as IOResult, Write};
use std::mem;
use std::os::windows::io::{AsRawHandle, RawHandle};
use winapi::um::consoleapi;
use winapi::um::wincon::{
    GetConsoleScreenBufferInfo, SetConsoleScreenBufferSize, CONSOLE_SCREEN_BUFFER_INFO, COORD,
    DISABLE_NEWLINE_AUTO_RETURN, ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT,
    ENABLE_VIRTUAL_TERMINAL_INPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
};

use terminal::{cast, Handle, ScreenSize, Terminal, BUF_SIZE};

impl Handle {
    fn writable_handle(&self) -> RawHandle {
        match self {
            Handle::File(f) => f.as_raw_handle(),
            Handle::Stdio { stdout, .. } => stdout.as_raw_handle(),
        }
    }

    fn readable_handle(&self) -> RawHandle {
        match self {
            Handle::File(f) => f.as_raw_handle(),
            Handle::Stdio { stdin, .. } => stdin.as_raw_handle(),
        }
    }

    fn enable_virtual_terminal_processing(&self) -> Result<(), Error> {
        let mode = self.get_console_output_mode()?;
        self.set_console_output_mode(
            mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING | DISABLE_NEWLINE_AUTO_RETURN,
        )?;

        let mode = self.get_console_input_mode()?;
        self.set_console_output_mode(mode | ENABLE_VIRTUAL_TERMINAL_INPUT)?;
        Ok(())
    }

    fn get_console_input_mode(&self) -> Result<u32, Error> {
        let mut mode = 0;
        let handle = self.readable_handle();
        if unsafe { consoleapi::GetConsoleMode(handle, &mut mode) } == 0 {
            bail!("GetConsoleMode failed: {}", IOError::last_os_error());
        }
        Ok(mode)
    }

    fn set_console_input_mode(&self, mode: u32) -> Result<(), Error> {
        let handle = self.readable_handle();
        if unsafe { consoleapi::SetConsoleMode(handle, mode) } == 0 {
            bail!("SetConsoleMode failed: {}", IOError::last_os_error());
        }
        Ok(())
    }

    fn get_console_output_mode(&self) -> Result<u32, Error> {
        let mut mode = 0;
        let handle = self.writable_handle();
        if unsafe { consoleapi::GetConsoleMode(handle, &mut mode) } == 0 {
            bail!("GetConsoleMode failed: {}", IOError::last_os_error());
        }
        Ok(mode)
    }

    fn set_console_output_mode(&self, mode: u32) -> Result<(), Error> {
        let handle = self.writable_handle();
        if unsafe { consoleapi::SetConsoleMode(handle, mode) } == 0 {
            bail!("SetConsoleMode failed: {}", IOError::last_os_error());
        }
        Ok(())
    }
}

pub struct WindowsTerminal {
    handle: Handle,
    write_buffer: Vec<u8>,
    saved_input_mode: u32,
    saved_output_mode: u32,
}

impl Drop for WindowsTerminal {
    fn drop(&mut self) {
        self.handle
            .set_console_input_mode(self.saved_input_mode)
            .expect("failed to restore console input mode");
        self.handle
            .set_console_output_mode(self.saved_output_mode)
            .expect("failed to restore console output mode");
    }
}

impl WindowsTerminal {
    pub fn new() -> Result<Self, Error> {
        let read = stdin();
        let write = stdout();

        if !read.is_tty() || !write.is_tty() {
            bail!("stdin and stdout must both be tty handles");
        }

        let handle = Handle::Stdio {
            stdin: read,
            stdout: write,
        };

        let saved_input_mode = handle.get_console_input_mode()?;
        let saved_output_mode = handle.get_console_output_mode()?;

        handle.enable_virtual_terminal_processing()?;

        Ok(Self {
            handle,
            saved_input_mode,
            saved_output_mode,
            write_buffer: Vec::with_capacity(BUF_SIZE),
        })
    }
}

impl Read for WindowsTerminal {
    fn read(&mut self, buf: &mut [u8]) -> IOResult<usize> {
        self.handle.read(buf)
    }
}

impl Write for WindowsTerminal {
    fn write(&mut self, buf: &[u8]) -> IOResult<usize> {
        if self.write_buffer.len() + buf.len() > self.write_buffer.capacity() {
            self.flush()?;
        }
        if buf.len() >= self.write_buffer.capacity() {
            self.handle.write(buf)
        } else {
            self.write_buffer.write(buf)
        }
    }

    fn flush(&mut self) -> IOResult<()> {
        if self.write_buffer.len() > 0 {
            self.handle.write(&self.write_buffer)?;
            self.write_buffer.clear();
        }
        self.handle.flush()
    }
}

impl Terminal for WindowsTerminal {
    fn set_raw_mode(&mut self) -> Result<(), Error> {
        let mode = self.handle.get_console_input_mode()?;

        self.handle.set_console_input_mode(
            mode & !(ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT),
        )
    }

    fn get_screen_size(&mut self) -> Result<ScreenSize, Error> {
        let mut info: CONSOLE_SCREEN_BUFFER_INFO = unsafe { mem::zeroed() };
        let handle = self.handle.writable_handle();
        let ok = unsafe { GetConsoleScreenBufferInfo(handle, &mut info as *mut _) };
        if ok != 1 {
            bail!(
                "failed to GetConsoleScreenBufferInfo: {}",
                IOError::last_os_error()
            );
        }

        Ok(ScreenSize {
            rows: cast(info.dwSize.Y)?,
            cols: cast(info.dwSize.X)?,
            xpixel: 0,
            ypixel: 0,
        })
    }

    fn set_screen_size(&mut self, size: ScreenSize) -> Result<(), Error> {
        let size = COORD {
            X: cast(size.cols)?,
            Y: cast(size.rows)?,
        };
        let handle = self.handle.writable_handle();
        if unsafe { SetConsoleScreenBufferSize(handle, size) } != 1 {
            bail!(
                "failed to SetConsoleScreenBufferSize: {}",
                IOError::last_os_error()
            );
        }
        Ok(())
    }
}
