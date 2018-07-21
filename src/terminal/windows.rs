use failure::Error;
use istty::IsTty;
use std::io::{stdin, stdout, Error as IOError, Read, Result as IOResult, Stdin, Stdout, Write};
use std::mem;
use std::ops::{Deref, DerefMut};
use std::os::windows::io::AsRawHandle;
use winapi::um::consoleapi;
use winapi::um::wincon::{
    GetConsoleScreenBufferInfo, SetConsoleScreenBufferSize, CONSOLE_SCREEN_BUFFER_INFO, COORD,
    DISABLE_NEWLINE_AUTO_RETURN, ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT,
    ENABLE_VIRTUAL_TERMINAL_INPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
};

use terminal::{cast, ScreenSize, Terminal, BUF_SIZE};

pub trait ConsoleInputHandle {
    fn set_input_mode(&mut self, mode: u32) -> Result<(), Error>;
    fn get_input_mode(&mut self) -> Result<u32, Error>;
}

pub trait ConsoleOutputHandle {
    fn set_output_mode(&mut self, mode: u32) -> Result<(), Error>;
    fn get_output_mode(&mut self) -> Result<u32, Error>;
}

struct InputHandle {
    handle: Stdin,
}

impl Deref for InputHandle {
    type Target = Stdin;

    fn deref(&self) -> &Stdin {
        &self.handle
    }
}

impl DerefMut for InputHandle {
    fn deref_mut(&mut self) -> &mut Stdin {
        &mut self.handle
    }
}

impl ConsoleInputHandle for InputHandle {
    fn set_input_mode(&mut self, mode: u32) -> Result<(), Error> {
        if unsafe { consoleapi::SetConsoleMode(self.handle.as_raw_handle(), mode) } == 0 {
            bail!("SetConsoleMode failed: {}", IOError::last_os_error());
        }
        Ok(())
    }

    fn get_input_mode(&mut self) -> Result<u32, Error> {
        let mut mode = 0;
        if unsafe { consoleapi::GetConsoleMode(self.handle.as_raw_handle(), &mut mode) } == 0 {
            bail!("GetConsoleMode failed: {}", IOError::last_os_error());
        }
        Ok(mode)
    }
}

struct OutputHandle {
    handle: Stdout,
}

impl Deref for OutputHandle {
    type Target = Stdout;

    fn deref(&self) -> &Stdout {
        &self.handle
    }
}

impl DerefMut for OutputHandle {
    fn deref_mut(&mut self) -> &mut Stdout {
        &mut self.handle
    }
}

impl ConsoleOutputHandle for OutputHandle {
    fn set_output_mode(&mut self, mode: u32) -> Result<(), Error> {
        if unsafe { consoleapi::SetConsoleMode(self.handle.as_raw_handle(), mode) } == 0 {
            bail!("SetConsoleMode failed: {}", IOError::last_os_error());
        }
        Ok(())
    }

    fn get_output_mode(&mut self) -> Result<u32, Error> {
        let mut mode = 0;
        if unsafe { consoleapi::GetConsoleMode(self.handle.as_raw_handle(), &mut mode) } == 0 {
            bail!("GetConsoleMode failed: {}", IOError::last_os_error());
        }
        Ok(mode)
    }
}

pub struct WindowsTerminal {
    input_handle: InputHandle,
    output_handle: OutputHandle,
    write_buffer: Vec<u8>,
    saved_input_mode: u32,
    saved_output_mode: u32,
}

impl Drop for WindowsTerminal {
    fn drop(&mut self) {
        self.input_handle
            .set_input_mode(self.saved_input_mode)
            .expect("failed to restore console input mode");
        self.output_handle
            .set_output_mode(self.saved_output_mode)
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

        let mut input_handle = InputHandle { handle: read };
        let mut output_handle = OutputHandle { handle: write };

        let saved_input_mode = input_handle.get_input_mode()?;
        let saved_output_mode = output_handle.get_output_mode()?;

        Ok(Self {
            input_handle,
            output_handle,
            saved_input_mode,
            saved_output_mode,
            write_buffer: Vec::with_capacity(BUF_SIZE),
        })
    }

    pub fn enable_virtual_terminal_processing(&mut self) -> Result<(), Error> {
        let mode = self.output_handle.get_output_mode()?;
        self.output_handle.set_output_mode(
            mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING | DISABLE_NEWLINE_AUTO_RETURN,
        )?;

        let mode = self.input_handle.get_input_mode()?;
        self.input_handle
            .set_input_mode(mode | ENABLE_VIRTUAL_TERMINAL_INPUT)?;
        Ok(())
    }
}

impl Read for WindowsTerminal {
    fn read(&mut self, buf: &mut [u8]) -> IOResult<usize> {
        self.input_handle.read(buf)
    }
}

impl Write for WindowsTerminal {
    fn write(&mut self, buf: &[u8]) -> IOResult<usize> {
        if self.write_buffer.len() + buf.len() > self.write_buffer.capacity() {
            self.flush()?;
        }
        if buf.len() >= self.write_buffer.capacity() {
            self.output_handle.write(buf)
        } else {
            self.write_buffer.write(buf)
        }
    }

    fn flush(&mut self) -> IOResult<()> {
        if self.write_buffer.len() > 0 {
            self.output_handle.write(&self.write_buffer)?;
            self.write_buffer.clear();
        }
        self.output_handle.flush()
    }
}

impl Terminal for WindowsTerminal {
    fn set_raw_mode(&mut self) -> Result<(), Error> {
        let mode = self.input_handle.get_input_mode()?;

        self.input_handle.set_input_mode(
            mode & !(ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT),
        )
    }

    fn get_screen_size(&mut self) -> Result<ScreenSize, Error> {
        let mut info: CONSOLE_SCREEN_BUFFER_INFO = unsafe { mem::zeroed() };
        let handle = self.output_handle.handle.as_raw_handle();
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
        let handle = self.output_handle.handle.as_raw_handle();
        if unsafe { SetConsoleScreenBufferSize(handle, size) } != 1 {
            bail!(
                "failed to SetConsoleScreenBufferSize: {}",
                IOError::last_os_error()
            );
        }
        Ok(())
    }

    fn get_console_input_handle(&mut self) -> &mut ConsoleInputHandle {
        &mut self.input_handle
    }

    fn get_console_output_handle(&mut self) -> &mut ConsoleOutputHandle {
        &mut self.output_handle
    }
}
