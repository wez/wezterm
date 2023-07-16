//! An abstraction over a terminal device

use crate::caps::Capabilities;
use crate::input::InputEvent;
use crate::surface::Change;
use crate::{format_err, Result};
use num_traits::NumCast;
use std::fmt::Display;
use std::io::{Read, Write};
use std::time::Duration;

#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

pub mod buffered;

#[cfg(unix)]
pub use self::unix::{UnixTerminal, UnixTerminalWaker as TerminalWaker};
#[cfg(windows)]
pub use self::windows::{WindowsTerminal, WindowsTerminalWaker as TerminalWaker};

/// Represents the size of the terminal screen.
/// The number of rows and columns of character cells are expressed.
/// Some implementations populate the size of those cells in pixels.
// On Windows, GetConsoleFontSize() can return the size of a cell in
// logical units and we can probably use this to populate xpixel, ypixel.
// GetConsoleScreenBufferInfo() can return the rows and cols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenSize {
    /// The number of rows of text
    pub rows: usize,
    /// The number of columns per row
    pub cols: usize,
    /// The width of a cell in pixels.  Some implementations never
    /// set this to anything other than zero.
    pub xpixel: usize,
    /// The height of a cell in pixels.  Some implementations never
    /// set this to anything other than zero.
    pub ypixel: usize,
}

impl ScreenSize {
    /// This is a helper function to facilitate the implementation of
    /// Terminal::probe_screen_size. It will emit a series of terminal
    /// escape sequences intended to probe the terminal and determine
    /// the ScreenSize.
    /// `input` and `output` are the corresponding reading and writable
    /// handles to the terminal.
    pub fn probe<I: Read, O: Write>(mut input: I, mut output: O) -> Result<Self> {
        use crate::escape::csi::{Device, Window};
        use crate::escape::parser::Parser;
        use crate::escape::{Action, DeviceControlMode, Esc, EscCode, CSI};

        let xt_version = CSI::Device(Box::new(Device::RequestTerminalNameAndVersion));
        let query_cells = CSI::Window(Box::new(Window::ReportTextAreaSizeCells));
        let query_pixels = CSI::Window(Box::new(Window::ReportCellSizePixels));
        let dev_attributes = CSI::Device(Box::new(Device::RequestPrimaryDeviceAttributes));

        // some tmux versions have their rows/cols swapped in ReportTextAreaSizeCells,
        // so we need to figure out the specific tmux version we're talking to
        write!(output, "{xt_version}{dev_attributes}")?;
        output.flush()?;
        let mut term = vec![];
        let mut parser = Parser::new();
        let mut done = false;

        while !done {
            let mut byte = [0u8];
            input.read(&mut byte)?;

            parser.parse(&byte, |action| {
                // print!("{action:?}\r\n");
                match action {
                    Action::Esc(Esc::Code(EscCode::StringTerminator)) => {}
                    Action::DeviceControl(dev) => {
                        if let DeviceControlMode::Data(b) = dev {
                            term.push(b);
                        }
                    }
                    _ => {
                        done = true;
                    }
                }
            });
        }

        /*
        print!(
            "probed terminal version: {}\r\n",
            String::from_utf8_lossy(&term)
        );
        */

        let is_tmux = term.starts_with(b"tmux ");
        let swapped_cols_rows = if is_tmux {
            let version = &term[5..];
            match version {
                b"3.2" | b"3.2a" | b"3.3" | b"3.3a" => true,
                _ => false,
            }
        } else {
            false
        };

        write!(output, "{query_cells}{query_pixels}")?;

        // tmux refuses to directly support responding to 14t or 16t queries
        // for pixel dimensions, so we need to jump through to the outer
        // terminal and see what it says
        if is_tmux {
            let tmux_begin = "\u{1b}Ptmux;\u{1b}";
            let tmux_end = "\u{1b}\\";
            write!(output, "{tmux_begin}{query_pixels}{tmux_end}")?;
            output.flush()?;
            // I really wanted to avoid a delay here, but tmux will re-order the
            // response to dev_attributes before it sends the response for the
            // passthru of query_pixels if we don't delay. The delay is potentially
            // imperfect for things like a laggy ssh connection. The consequence
            // of the timing being wrong is that we won't be able to reason about
            // the pixel dimensions, which is "OK", but that was kinda the whole
            // point of probing this way vs. termios.
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        write!(output, "{dev_attributes}")?;
        output.flush()?;

        let mut parser = Parser::new();
        let mut done = false;
        let mut size = ScreenSize {
            rows: 0,
            cols: 0,
            xpixel: 0,
            ypixel: 0,
        };

        while !done {
            let mut byte = [0u8];
            input.read(&mut byte)?;

            parser.parse(&byte, |action| {
                // print!("{action:?}\r\n");
                match action {
                    Action::CSI(csi) => match csi {
                        CSI::Window(win) => match *win {
                            Window::ResizeWindowCells { width, height } => {
                                let width = width.unwrap_or(1);
                                let height = height.unwrap_or(1);
                                if width > 0 && height > 0 {
                                    let width = width as usize;
                                    let height = height as usize;
                                    if swapped_cols_rows {
                                        size.rows = width;
                                        size.cols = height;
                                    } else {
                                        size.rows = height;
                                        size.cols = width;
                                    }
                                }
                            }
                            Window::ReportCellSizePixelsResponse { width, height } => {
                                let width = width.unwrap_or(1);
                                let height = height.unwrap_or(1);
                                if width > 0 && height > 0 {
                                    let width = width as usize;
                                    let height = height as usize;
                                    size.xpixel = width;
                                    size.ypixel = height;
                                }
                            }
                            _ => {
                                done = true;
                            }
                        },
                        _ => {
                            done = true;
                        }
                    },
                    _ => {
                        done = true;
                    }
                }
            });
        }

        Ok(size)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocking {
    DoNotWait,
    Wait,
}

/// `Terminal` abstracts over some basic terminal capabilities.
/// If the `set_raw_mode` or `set_cooked_mode` functions are used in
/// any combination, the implementation is required to restore the
/// terminal mode that was in effect when it was created.
pub trait Terminal {
    /// Raw mode disables input line buffering, allowing data to be
    /// read as the user presses keys, disables local echo, so keys
    /// pressed by the user do not implicitly render to the terminal
    /// output, and disables canonicalization of unix newlines to CRLF.
    fn set_raw_mode(&mut self) -> Result<()>;
    fn set_cooked_mode(&mut self) -> Result<()>;

    /// Enter the alternate screen.  The alternate screen will be left
    /// automatically when the `Terminal` is dropped.
    fn enter_alternate_screen(&mut self) -> Result<()>;

    /// Exit the alternate screen.
    fn exit_alternate_screen(&mut self) -> Result<()>;

    /// Queries the current screen size, returning width, height.
    fn get_screen_size(&mut self) -> Result<ScreenSize>;

    /// Like get_screen_size but uses escape sequences to interrogate
    /// the terminal rather than relying on the termios/kernel interface
    /// You should delegate this to `ScreenSize::probe(&mut self.read, &mut self.write)`
    /// to implement this method.
    fn probe_screen_size(&mut self) -> Result<ScreenSize>;

    /// Sets the current screen size
    fn set_screen_size(&mut self, size: ScreenSize) -> Result<()>;

    /// Render a series of changes to the terminal output
    fn render(&mut self, changes: &[Change]) -> Result<()>;

    /// Flush any buffered output
    fn flush(&mut self) -> Result<()>;

    /// Check for a parsed input event.
    /// `wait` indicates the behavior in the case that no input is
    /// immediately available.  If wait is `None` then `poll_input`
    /// will not return until an event is available.  If wait is
    /// `Some(duration)` then `poll_input` will wait up to the given
    /// duration for an event before returning with a value of
    /// `Ok(None)`.  If wait is `Some(Duration::ZERO)` then the
    /// poll is non-blocking.
    ///
    /// The possible values returned as `InputEvent`s depend on the
    /// mode of the terminal.  Most values are not returned unless
    /// the terminal is set to raw mode.
    fn poll_input(&mut self, wait: Option<Duration>) -> Result<Option<InputEvent>>;

    fn waker(&self) -> TerminalWaker;
}

/// `SystemTerminal` is a concrete implementation of `Terminal`.
/// Ideally you wouldn't reference `SystemTerminal` in consuming
/// code.  This type is exposed for convenience if you are doing
/// something unusual and want easier access to the constructors.
#[cfg(unix)]
pub type SystemTerminal = UnixTerminal;
#[cfg(windows)]
pub type SystemTerminal = WindowsTerminal;

/// Construct a new instance of Terminal.
/// The terminal will have a renderer that is influenced by the configuration
/// in the provided `Capabilities` instance.
/// The terminal will explicitly open `/dev/tty` on Unix systems and
/// `CONIN$` and `CONOUT$` on Windows systems, so that it should yield a
/// functioning console with minimal headaches.
/// If you have a more advanced use case you will want to look to the
/// constructors for `UnixTerminal` and `WindowsTerminal` and call whichever
/// one is most suitable for your needs.
pub fn new_terminal(caps: Capabilities) -> Result<impl Terminal> {
    SystemTerminal::new(caps)
}

pub(crate) fn cast<T: NumCast + Display + Copy, U: NumCast>(n: T) -> Result<U> {
    num_traits::cast(n).ok_or_else(|| format_err!("{} is out of bounds for this system", n))
}
