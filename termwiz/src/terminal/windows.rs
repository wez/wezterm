use crate::istty::IsTty;
use anyhow::{anyhow, bail, Error};
use filedescriptor::{FileDescriptor, OwnedHandle};
use std::cmp::{max, min};
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::{stdin, stdout, Error as IoError, Read, Result as IoResult, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::sync::Arc;
use std::time::Duration;
use std::{mem, ptr};
use winapi::shared::winerror::WAIT_TIMEOUT;
use winapi::um::consoleapi;
use winapi::um::synchapi::{CreateEventW, SetEvent, WaitForMultipleObjects};
use winapi::um::winbase::{INFINITE, WAIT_FAILED, WAIT_OBJECT_0};
use winapi::um::wincon::{
    FillConsoleOutputAttribute, FillConsoleOutputCharacterW, GetConsoleScreenBufferInfo,
    ScrollConsoleScreenBufferW, SetConsoleCursorPosition, SetConsoleScreenBufferSize,
    SetConsoleTextAttribute, SetConsoleWindowInfo, CHAR_INFO, CONSOLE_SCREEN_BUFFER_INFO, COORD,
    DISABLE_NEWLINE_AUTO_RETURN, ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_MOUSE_INPUT,
    ENABLE_PROCESSED_INPUT, ENABLE_VIRTUAL_TERMINAL_INPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
    ENABLE_WINDOW_INPUT, INPUT_RECORD, SMALL_RECT,
};

use crate::caps::Capabilities;
use crate::input::{InputEvent, InputParser};
use crate::render::windows::WindowsConsoleRenderer;
use crate::surface::Change;
use crate::terminal::{cast, ScreenSize, Terminal};

const BUF_SIZE: usize = 128;

pub trait ConsoleInputHandle {
    fn set_input_mode(&mut self, mode: u32) -> Result<(), Error>;
    fn get_input_mode(&mut self) -> Result<u32, Error>;
    fn get_number_of_input_events(&mut self) -> Result<usize, Error>;
    fn read_console_input(&mut self, num_events: usize) -> Result<Vec<INPUT_RECORD>, Error>;
}

pub trait ConsoleOutputHandle {
    fn set_output_mode(&mut self, mode: u32) -> Result<(), Error>;
    fn get_output_mode(&mut self) -> Result<u32, Error>;
    fn fill_char(&mut self, text: char, x: i16, y: i16, len: u32) -> Result<u32, Error>;
    fn fill_attr(&mut self, attr: u16, x: i16, y: i16, len: u32) -> Result<u32, Error>;
    fn set_attr(&mut self, attr: u16) -> Result<(), Error>;
    fn set_cursor_position(&mut self, x: i16, y: i16) -> Result<(), Error>;
    fn get_buffer_info(&mut self) -> Result<CONSOLE_SCREEN_BUFFER_INFO, Error>;
    fn set_viewport(&mut self, left: i16, top: i16, right: i16, bottom: i16) -> Result<(), Error>;
    fn scroll_region(
        &mut self,
        left: i16,
        top: i16,
        right: i16,
        bottom: i16,
        dx: i16,
        dy: i16,
        attr: u16,
    ) -> Result<(), Error>;
}

struct InputHandle {
    handle: FileDescriptor,
}

impl Read for InputHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        self.handle.read(buf)
    }
}

impl ConsoleInputHandle for InputHandle {
    fn set_input_mode(&mut self, mode: u32) -> Result<(), Error> {
        if unsafe { consoleapi::SetConsoleMode(self.handle.as_raw_handle() as *mut _, mode) } == 0 {
            bail!("SetConsoleMode failed: {}", IoError::last_os_error());
        }
        Ok(())
    }

    fn get_input_mode(&mut self) -> Result<u32, Error> {
        let mut mode = 0;
        if unsafe { consoleapi::GetConsoleMode(self.handle.as_raw_handle() as *mut _, &mut mode) }
            == 0
        {
            bail!("GetConsoleMode failed: {}", IoError::last_os_error());
        }
        Ok(mode)
    }

    fn get_number_of_input_events(&mut self) -> Result<usize, Error> {
        let mut num = 0;
        if unsafe {
            consoleapi::GetNumberOfConsoleInputEvents(
                self.handle.as_raw_handle() as *mut _,
                &mut num,
            )
        } == 0
        {
            bail!(
                "GetNumberOfConsoleInputEvents failed: {}",
                IoError::last_os_error()
            );
        }
        Ok(num as usize)
    }

    fn read_console_input(&mut self, num_events: usize) -> Result<Vec<INPUT_RECORD>, Error> {
        let mut res = Vec::with_capacity(num_events);
        let empty_record: INPUT_RECORD = unsafe { mem::zeroed() };
        res.resize(num_events, empty_record);

        let mut num = 0;

        if unsafe {
            consoleapi::ReadConsoleInputW(
                self.handle.as_raw_handle() as *mut _,
                res.as_mut_ptr(),
                num_events as u32,
                &mut num,
            )
        } == 0
        {
            bail!("ReadConsoleInput failed: {}", IoError::last_os_error());
        }

        unsafe { res.set_len(num as usize) };
        Ok(res)
    }
}

struct OutputHandle {
    handle: FileDescriptor,
    write_buffer: Vec<u8>,
}

impl OutputHandle {
    fn new(handle: FileDescriptor) -> Self {
        Self {
            handle,
            write_buffer: Vec::with_capacity(BUF_SIZE),
        }
    }
}

struct EventHandle {
    handle: OwnedHandle,
}

impl EventHandle {
    fn new() -> IoResult<Self> {
        let handle = unsafe { CreateEventW(ptr::null_mut(), 0, 0, ptr::null_mut()) };
        if handle.is_null() {
            Err(IoError::last_os_error())
        } else {
            Ok(Self {
                handle: unsafe { OwnedHandle::from_raw_handle(handle as *mut _) },
            })
        }
    }

    fn set(&self) -> IoResult<()> {
        let ok = unsafe { SetEvent(self.handle.as_raw_handle() as *mut _) };
        if ok == 0 {
            Err(IoError::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl Write for OutputHandle {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        if self.write_buffer.len() + buf.len() > self.write_buffer.capacity() {
            self.flush()?;
        }
        if buf.len() >= self.write_buffer.capacity() {
            self.handle.write(buf)
        } else {
            self.write_buffer.write(buf)
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        if self.write_buffer.len() > 0 {
            self.handle.write(&self.write_buffer)?;
            self.write_buffer.clear();
        }
        Ok(())
    }
}

impl ConsoleOutputHandle for OutputHandle {
    fn set_output_mode(&mut self, mode: u32) -> Result<(), Error> {
        if unsafe { consoleapi::SetConsoleMode(self.handle.as_raw_handle() as *mut _, mode) } == 0 {
            bail!("SetConsoleMode failed: {}", IoError::last_os_error());
        }
        Ok(())
    }

    fn get_output_mode(&mut self) -> Result<u32, Error> {
        let mut mode = 0;
        if unsafe { consoleapi::GetConsoleMode(self.handle.as_raw_handle() as *mut _, &mut mode) }
            == 0
        {
            bail!("GetConsoleMode failed: {}", IoError::last_os_error());
        }
        Ok(mode)
    }

    fn fill_char(&mut self, text: char, x: i16, y: i16, len: u32) -> Result<u32, Error> {
        let mut wrote = 0;
        if unsafe {
            FillConsoleOutputCharacterW(
                self.handle.as_raw_handle() as *mut _,
                text as u16,
                len,
                COORD { X: x, Y: y },
                &mut wrote,
            )
        } == 0
        {
            bail!(
                "FillConsoleOutputCharacterW failed: {}",
                IoError::last_os_error()
            );
        }
        Ok(wrote)
    }

    fn fill_attr(&mut self, attr: u16, x: i16, y: i16, len: u32) -> Result<u32, Error> {
        let mut wrote = 0;
        if unsafe {
            FillConsoleOutputAttribute(
                self.handle.as_raw_handle() as *mut _,
                attr,
                len,
                COORD { X: x, Y: y },
                &mut wrote,
            )
        } == 0
        {
            bail!(
                "FillConsoleOutputAttribute failed: {}",
                IoError::last_os_error()
            );
        }
        Ok(wrote)
    }

    fn set_attr(&mut self, attr: u16) -> Result<(), Error> {
        if unsafe { SetConsoleTextAttribute(self.handle.as_raw_handle() as *mut _, attr) } == 0 {
            bail!(
                "SetConsoleTextAttribute failed: {}",
                IoError::last_os_error()
            );
        }
        Ok(())
    }

    fn set_cursor_position(&mut self, x: i16, y: i16) -> Result<(), Error> {
        if unsafe {
            SetConsoleCursorPosition(self.handle.as_raw_handle() as *mut _, COORD { X: x, Y: y })
        } == 0
        {
            bail!(
                "SetConsoleCursorPosition failed: {}",
                IoError::last_os_error()
            );
        }
        Ok(())
    }

    fn get_buffer_info(&mut self) -> Result<CONSOLE_SCREEN_BUFFER_INFO, Error> {
        let mut info: CONSOLE_SCREEN_BUFFER_INFO = unsafe { mem::zeroed() };
        let ok = unsafe {
            GetConsoleScreenBufferInfo(self.handle.as_raw_handle() as *mut _, &mut info as *mut _)
        };
        if ok == 0 {
            bail!(
                "GetConsoleScreenBufferInfo failed: {}",
                IoError::last_os_error()
            );
        }
        Ok(info)
    }

    fn set_viewport(&mut self, left: i16, top: i16, right: i16, bottom: i16) -> Result<(), Error> {
        let rect = SMALL_RECT {
            Left: left,
            Top: top,
            Right: right,
            Bottom: bottom,
        };
        if unsafe { SetConsoleWindowInfo(self.handle.as_raw_handle() as *mut _, 1, &rect) } == 0 {
            bail!("SetConsoleWindowInfo failed: {}", IoError::last_os_error());
        }
        Ok(())
    }

    fn scroll_region(
        &mut self,
        left: i16,
        top: i16,
        right: i16,
        bottom: i16,
        dx: i16,
        dy: i16,
        attr: u16,
    ) -> Result<(), Error> {
        let scroll_rect = SMALL_RECT {
            Left: max(left, left - dx),
            Top: max(top, top - dy),
            Right: min(right, right - dx),
            Bottom: min(bottom, bottom - dy),
        };
        let clip_rect = SMALL_RECT {
            Left: left,
            Top: top,
            Right: right,
            Bottom: bottom,
        };
        let fill = unsafe {
            let mut fill = CHAR_INFO {
                Char: mem::zeroed(),
                Attributes: attr,
            };
            *fill.Char.UnicodeChar_mut() = ' ' as u16;
            fill
        };
        if unsafe {
            ScrollConsoleScreenBufferW(
                self.handle.as_raw_handle() as *mut _,
                &scroll_rect,
                &clip_rect,
                COORD {
                    X: max(left, left + dx),
                    Y: max(left, top + dy),
                },
                &fill,
            )
        } == 0
        {
            bail!(
                "ScrollConsoleScreenBufferW failed: {}",
                IoError::last_os_error()
            );
        }
        Ok(())
    }
}

pub struct WindowsTerminal {
    input_handle: InputHandle,
    output_handle: OutputHandle,
    waker_handle: Arc<EventHandle>,
    saved_input_mode: u32,
    saved_output_mode: u32,
    renderer: WindowsConsoleRenderer,
    input_parser: InputParser,
    input_queue: VecDeque<InputEvent>,
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
    /// Attempt to create an instance from the stdin and stdout of the
    /// process.  This will fail unless both are associated with a tty.
    /// Note that this will duplicate the underlying file descriptors
    /// and will no longer participate in the stdin/stdout locking
    /// provided by the rust standard library.
    pub fn new_from_stdio(caps: Capabilities) -> Result<Self, Error> {
        Self::new_with(caps, stdin(), stdout())
    }

    /// Create an instance using the provided capabilities, read and write
    /// handles. The read and write handles must be tty handles of this
    /// will return an error.
    pub fn new_with<A: Read + IsTty + AsRawHandle, B: Write + IsTty + AsRawHandle>(
        caps: Capabilities,
        read: A,
        write: B,
    ) -> Result<Self, Error> {
        if !read.is_tty() || !write.is_tty() {
            bail!("stdin and stdout must both be tty handles");
        }

        let mut input_handle = InputHandle {
            handle: FileDescriptor::dup(&read)?,
        };
        let mut output_handle = OutputHandle::new(FileDescriptor::dup(&write)?);
        let waker_handle = Arc::new(EventHandle::new()?);

        let saved_input_mode = input_handle.get_input_mode()?;
        let saved_output_mode = output_handle.get_output_mode()?;
        let renderer = WindowsConsoleRenderer::new(caps);
        let input_parser = InputParser::new();

        Ok(Self {
            input_handle,
            output_handle,
            waker_handle,
            saved_input_mode,
            saved_output_mode,
            renderer,
            input_parser,
            input_queue: VecDeque::new(),
        })
    }

    /// Attempt to explicitly open handles to a console device (CONIN$,
    /// CONOUT$). This should yield the terminal already associated with
    /// the process, even if stdio streams have been redirected.
    pub fn new(caps: Capabilities) -> Result<Self, Error> {
        let read = OpenOptions::new().read(true).write(true).open("CONIN$")?;
        let write = OpenOptions::new().read(true).write(true).open("CONOUT$")?;
        Self::new_with(caps, read, write)
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

#[derive(Clone)]
pub struct WindowsTerminalWaker {
    handle: Arc<EventHandle>,
}

impl WindowsTerminalWaker {
    pub fn wake(&self) -> IoResult<()> {
        self.handle.set()?;
        Ok(())
    }
}

impl Terminal for WindowsTerminal {
    fn set_raw_mode(&mut self) -> Result<(), Error> {
        let mode = self.input_handle.get_input_mode()?;

        self.input_handle.set_input_mode(
            (mode & !(ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT))
                | ENABLE_MOUSE_INPUT
                | ENABLE_WINDOW_INPUT,
        )
    }

    fn set_cooked_mode(&mut self) -> anyhow::Result<()> {
        let mode = self.input_handle.get_input_mode()?;

        self.input_handle.set_input_mode(
            (mode & !(ENABLE_WINDOW_INPUT | ENABLE_WINDOW_INPUT))
                | ENABLE_ECHO_INPUT
                | ENABLE_LINE_INPUT
                | ENABLE_PROCESSED_INPUT,
        )
    }

    fn enter_alternate_screen(&mut self) -> Result<(), Error> {
        // TODO: Implement using CreateConsoleScreenBuffer and
        // SetConsoleActiveScreenBuffer.
        Ok(())
    }

    fn exit_alternate_screen(&mut self) -> Result<(), Error> {
        // TODO: Implement using SetConsoleActiveScreenBuffer.
        Ok(())
    }

    fn get_screen_size(&mut self) -> Result<ScreenSize, Error> {
        let info = self.output_handle.get_buffer_info()?;

        // NOTE: the default console behavior is different from unix style
        // terminals wrt. handling printing in the last column position.
        // We under report the width by one to make it easier to have similar
        // semantics to unix style terminals.

        let visible_width = 0 + (info.srWindow.Right - info.srWindow.Left);
        let visible_height = 1 + (info.srWindow.Bottom - info.srWindow.Top);

        Ok(ScreenSize {
            rows: cast(visible_height)?,
            cols: cast(visible_width)?,
            xpixel: 0,
            ypixel: 0,
        })
    }

    fn set_screen_size(&mut self, size: ScreenSize) -> Result<(), Error> {
        // FIXME: take into account the visible window size here;
        // this probably changes the size of everything including scrollback
        let size = COORD {
            // See the note in get_screen_size() for info on the +1.
            X: cast(size.cols + 1)?,
            Y: cast(size.rows)?,
        };
        let handle = self.output_handle.handle.as_raw_handle();
        if unsafe { SetConsoleScreenBufferSize(handle as *mut _, size) } != 1 {
            bail!(
                "failed to SetConsoleScreenBufferSize: {}",
                IoError::last_os_error()
            );
        }
        Ok(())
    }

    fn render(&mut self, changes: &[Change]) -> Result<(), Error> {
        self.renderer
            .render_to(changes, &mut self.input_handle, &mut self.output_handle)
    }
    fn flush(&mut self) -> Result<(), Error> {
        self.output_handle
            .flush()
            .map_err(|e| anyhow!("flush failed: {}", e))
    }

    fn poll_input(&mut self, wait: Option<Duration>) -> Result<Option<InputEvent>, Error> {
        loop {
            if let Some(event) = self.input_queue.pop_front() {
                return Ok(Some(event));
            }

            let mut pending = self.input_handle.get_number_of_input_events()?;

            if pending == 0 {
                let mut handles = [
                    self.input_handle.handle.as_raw_handle() as *mut _,
                    self.waker_handle.handle.as_raw_handle() as *mut _,
                ];
                let result = unsafe {
                    WaitForMultipleObjects(
                        2,
                        handles.as_mut_ptr(),
                        0,
                        wait.map(|wait| wait.as_millis() as u32).unwrap_or(INFINITE),
                    )
                };
                if result == WAIT_OBJECT_0 + 0 {
                    pending = 1;
                } else if result == WAIT_OBJECT_0 + 1 {
                    return Ok(Some(InputEvent::Wake));
                } else if result == WAIT_FAILED {
                    bail!(
                        "failed to WaitForMultipleObjects: {}",
                        IoError::last_os_error()
                    );
                } else if result == WAIT_TIMEOUT {
                    return Ok(None);
                } else {
                    return Ok(None);
                }
            }

            let records = self.input_handle.read_console_input(pending)?;

            let input_queue = &mut self.input_queue;
            self.input_parser
                .decode_input_records(&records, &mut |evt| input_queue.push_back(evt));
        }
    }

    fn waker(&self) -> WindowsTerminalWaker {
        WindowsTerminalWaker {
            handle: self.waker_handle.clone(),
        }
    }
}
