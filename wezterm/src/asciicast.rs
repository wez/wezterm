use anyhow::Context;
use chrono::serde::ts_seconds_option;
use chrono::{DateTime, Utc};
use config::ConfigHandle;
use filedescriptor::FileDescriptor;
use portable_pty::{native_pty_system, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::mem::MaybeUninit;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};
use structopt::StructOpt;
use termios::{cfmakeraw, tcsetattr, Termios, TCSAFLUSH};
#[cfg(unix)]
use unix::UnixTty as Tty;
use wezterm_term::color::ColorPalette;

/// See <https://github.com/asciinema/asciinema/blob/develop/doc/asciicast-v2.md>
/// for file format specification
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Header {
    /// Must be 2 or higher
    pub version: u32,
    /// Initial terminal width (number of columns)
    pub width: u32,
    /// Initial terminal height (number of columns)
    pub height: u32,
    /// Unix timestamp of starting time of session
    #[serde(
        default,
        with = "ts_seconds_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub timestamp: Option<DateTime<Utc>>,
    /// Duration of the whole recording in seconds
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f32>,
    /// Used to reduce terminal inactivity (delays between frames)
    /// to a maximum of this amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_time_limit: Option<f32>,
    /// Command that was recorded
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Title of the asciicast
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Map of captured environment variables
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    /// Color theme of the recorded terminal
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
}

impl Header {
    fn new(config: &ConfigHandle, size: PtySize) -> Self {
        let mut env = HashMap::new();
        env.insert("TERM".to_string(), config.term.to_string());
        if let Ok(shell) = std::env::var("SHELL") {
            env.insert("SHELL".to_string(), shell.to_string());
        }

        let palette: ColorPalette = config.resolved_palette.clone().into();
        let ansi_colors: Vec<String> = palette.colors.0[0..16]
            .iter()
            .map(|c| c.to_rgb_string())
            .collect();

        let theme = Theme {
            fg: palette.foreground.to_rgb_string(),
            bg: palette.background.to_rgb_string(),
            palette: ansi_colors.join(":"),
        };

        Header {
            version: 2,
            height: size.rows.into(),
            width: size.cols.into(),
            env,
            theme: Some(theme),
            ..Default::default()
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Theme {
    /// Normal text color
    pub fg: String,
    /// Normal background color
    pub bg: String,
    /// List of 8 or 16 colors separated by a colon character
    pub palette: String,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Event(pub f32, pub String, pub String);

impl Event {
    fn log_output<W: Write>(mut w: W, elapsed: f32, output: &str) -> std::io::Result<()> {
        let event = Event(elapsed, "o".to_string(), output.to_string());
        writeln!(w, "{}", serde_json::to_string(&event)?)
    }
}

mod unix {
    use super::*;
    use std::os::unix::io::AsRawFd;

    pub struct UnixTty {
        tty: FileDescriptor,
        termios: Termios,
    }

    fn get_termios(fd: &FileDescriptor) -> anyhow::Result<Termios> {
        Termios::from_fd(fd.as_raw_fd()).context("get_termios failed")
    }

    fn set_termios(
        fd: &FileDescriptor,
        termios: &Termios,
        mode: libc::c_int,
    ) -> anyhow::Result<()> {
        tcsetattr(fd.as_raw_fd(), mode, termios).context("set_termios failed")
    }

    impl UnixTty {
        pub fn new() -> anyhow::Result<Self> {
            let tty = FileDescriptor::new(
                std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open("/dev/tty")?,
            );
            let termios = get_termios(&tty)?;

            Ok(Self { tty, termios })
        }

        pub fn set_raw(&mut self) -> anyhow::Result<()> {
            let mut termios = get_termios(&self.tty)?;
            cfmakeraw(&mut termios);
            set_termios(&self.tty, &termios, TCSAFLUSH)
        }

        pub fn set_cooked(&mut self) -> anyhow::Result<()> {
            set_termios(&self.tty, &self.termios, TCSAFLUSH)
        }

        pub fn get_size(&self) -> anyhow::Result<PtySize> {
            let mut size = MaybeUninit::<libc::winsize>::uninit();
            if unsafe { libc::ioctl(self.tty.as_raw_fd(), libc::TIOCGWINSZ as _, &mut size) } != 0 {
                anyhow::bail!(
                    "failed to ioctl(TIOCGWINSZ): {:#}",
                    std::io::Error::last_os_error()
                );
            }

            let size = unsafe { size.assume_init() };

            Ok(PtySize {
                rows: size.ws_row.into(),
                cols: size.ws_col.into(),
                pixel_width: size.ws_xpixel.into(),
                pixel_height: size.ws_ypixel.into(),
            })
        }

        pub fn reader(&self) -> anyhow::Result<FileDescriptor> {
            Ok(self.tty.try_clone()?)
        }

        pub fn write_all(&mut self, data: &[u8]) -> anyhow::Result<()> {
            Ok(self.tty.write_all(data)?)
        }
    }

    impl Drop for UnixTty {
        fn drop(&mut self) {
            let _ = self.set_cooked();
        }
    }
}

#[derive(Debug)]
enum Message {
    /// Input from the user
    Stdin(Vec<u8>),
    /// Output from the child tty
    Stdout(Vec<u8>),
    /// Child process terminated
    Terminated(portable_pty::ExitStatus),
}

#[derive(Debug, StructOpt, Clone)]
pub struct RecordCommand {
    #[structopt(parse(from_os_str))]
    prog: Vec<OsString>,
}

impl RecordCommand {
    pub fn run(&self, config: ConfigHandle) -> anyhow::Result<()> {
        let mut tty = Tty::new()?;
        let size = tty.get_size()?;

        let header = Header::new(&config, size);

        let (cast_file, cast_file_name) = tempfile::Builder::new()
            .prefix("wezterm-recording-")
            .suffix(".cast")
            .tempfile()?
            .keep()?;
        let mut cast_file = BufWriter::new(cast_file);
        writeln!(cast_file, "{}", serde_json::to_string(&header)?)?;

        let pty_system = native_pty_system();
        let mut pair = pty_system.openpty(size)?;

        let prog = self.prog.iter().map(|s| s.as_os_str()).collect::<Vec<_>>();
        let cmd = config.build_prog(
            if self.prog.is_empty() {
                None
            } else {
                Some(prog)
            },
            config.default_prog.as_ref(),
            config.default_cwd.as_ref(),
        )?;

        let mut child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);
        let mut child_output = pair.master.try_clone_reader()?;

        tty.set_raw()?;

        let (tx, rx) = channel();

        {
            let tx = tx.clone();
            std::thread::spawn(move || -> anyhow::Result<()> {
                let mut buf = [0u8; 8192];
                loop {
                    let size = child_output.read(&mut buf)?;
                    if size == 0 {
                        break;
                    }
                    tx.send(Message::Stdout(buf[0..size].to_vec()))?;
                }
                Ok(())
            });
        }

        {
            let mut stdin = tty.reader()?;
            let tx = tx.clone();
            std::thread::spawn(move || -> anyhow::Result<()> {
                let mut buf = [0u8; 8192];
                loop {
                    let size = stdin.read(&mut buf)?;
                    if size == 0 {
                        break;
                    }
                    tx.send(Message::Stdin(buf[0..size].to_vec()))?;
                }
                Ok(())
            });
        }

        {
            let tx = tx.clone();
            std::thread::spawn(move || -> anyhow::Result<()> {
                let status = child.wait()?;
                tx.send(Message::Terminated(status))?;
                Ok(())
            });
        }

        let mut child_status = None;
        let first_output = Instant::now();
        let mut buffer = vec![];

        for msg in rx {
            match msg {
                Message::Stdin(data) => {
                    pair.master.write_all(&data)?;
                }
                Message::Stdout(mut data) => {
                    let elapsed = first_output.elapsed().as_secs_f32();
                    tty.write_all(&data)?;

                    // The end of the data may be an incomplete utf8 sequence
                    // that straddles the buffer boundary.  JSON requires strings
                    // to be utf-8 so we need to send the currently-valid portions
                    // through to the .cast file and buffer up the remainder
                    buffer.append(&mut data);
                    match std::str::from_utf8(&buffer) {
                        Ok(valid) => {
                            Event::log_output(&mut cast_file, elapsed, valid)?;
                            buffer.clear();
                        }
                        Err(error) => {
                            let valid_len = error.valid_up_to();
                            Event::log_output(&mut cast_file, elapsed, unsafe {
                                std::str::from_utf8_unchecked(&buffer[0..valid_len])
                            })?;

                            buffer.drain(0..valid_len);

                            if let Some(invalid_sequence_length) = error.error_len() {
                                // Invalid sequence: skip it
                                buffer.drain(0..invalid_sequence_length);
                            }
                        }
                    }
                }
                Message::Terminated(status) => {
                    child_status.replace(status);
                    break;
                }
            }
        }

        tty.set_cooked()?;
        eprintln!("Child status: {:?}", child_status);
        cast_file.flush()?;
        eprintln!("*** Finished recording to {}", cast_file_name.display());

        Ok(())
    }
}

#[derive(Debug, StructOpt, Clone)]
pub struct PlayCommand {
    cast_file: PathBuf,
}

impl PlayCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let mut cast_file = BufReader::new(
            std::fs::File::open(&self.cast_file)
                .with_context(|| format!("reading cast file {}", self.cast_file.display()))?,
        );
        let mut header_line = String::new();
        cast_file
            .read_line(&mut header_line)
            .context("reading Header line")?;

        let header: Header = serde_json::from_str(&header_line).context("parsing Header")?;

        let mut tty = Tty::new()?;
        let size = tty.get_size()?;
        if u32::from(size.cols) < header.width || u32::from(size.rows) < header.height {
            anyhow::bail!(
                "{} was recorded with width={} and height={}
                 but the current screen dimensions {}x{} are
                 too small to display it",
                self.cast_file.display(),
                header.width,
                header.height,
                size.cols,
                size.rows
            );
        }

        tty.set_raw()?;

        let start = Instant::now();

        for line in cast_file.lines() {
            let line = line?;
            let event: Event = serde_json::from_str(&line)?;
            if event.1 != "o" {
                continue;
            }
            let target = start + Duration::from_secs_f32(event.0);
            let duration = target.saturating_duration_since(Instant::now());
            std::thread::sleep(duration);

            tty.write_all(&event.2.as_bytes())?;
        }

        tty.set_cooked()?;

        Ok(())
    }
}
