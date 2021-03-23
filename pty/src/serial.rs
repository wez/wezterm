//! This module implements a serial port based tty.
//! This is a bit different from the other implementations in that
//! we cannot explicitly spawn a process into the serial connection,
//! so we can only use a `CommandBuilder::new_default_prog` with the
//! `openpty` method.
//! On most (all?) systems, attempting to open multiple instances of
//! the same serial port will fail.
use crate::{Child, CommandBuilder, ExitStatus, MasterPty, PtyPair, PtySize, PtySystem, SlavePty};
use anyhow::{ensure, Context};
use filedescriptor::FileDescriptor;
use serial::{
    BaudRate, CharSize, FlowControl, Parity, PortSettings, SerialPort, StopBits, SystemPort,
};
use std::ffi::{OsStr, OsString};
use std::io::Result as IoResult;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Handle = Arc<Mutex<SystemPort>>;

pub struct SerialTty {
    port: OsString,
    baud: BaudRate,
    char_size: CharSize,
    parity: Parity,
    stop_bits: StopBits,
    flow_control: FlowControl,
}

impl SerialTty {
    pub fn new<T: AsRef<OsStr> + ?Sized>(port: &T) -> Self {
        Self {
            port: port.as_ref().to_owned(),
            baud: BaudRate::Baud9600,
            char_size: CharSize::Bits8,
            parity: Parity::ParityNone,
            stop_bits: StopBits::Stop1,
            flow_control: FlowControl::FlowSoftware,
        }
    }

    pub fn set_baud_rate(&mut self, baud: BaudRate) {
        self.baud = baud;
    }

    pub fn set_char_size(&mut self, char_size: CharSize) {
        self.char_size = char_size;
    }

    pub fn set_parity(&mut self, parity: Parity) {
        self.parity = parity;
    }

    pub fn set_stop_bits(&mut self, stop_bits: StopBits) {
        self.stop_bits = stop_bits;
    }

    pub fn set_flow_control(&mut self, flow_control: FlowControl) {
        self.flow_control = flow_control;
    }
}

impl PtySystem for SerialTty {
    fn openpty(&self, _size: PtySize) -> anyhow::Result<PtyPair> {
        let mut port = serial::open(&self.port)
            .with_context(|| format!("openpty on serial port {:?}", self.port))?;

        let settings = PortSettings {
            baud_rate: self.baud,
            char_size: self.char_size,
            parity: self.parity,
            stop_bits: self.stop_bits,
            flow_control: self.flow_control,
        };
        log::debug!("serial settings: {:#?}", settings);
        port.configure(&settings)?;

        // The timeout needs to be rather short because, at least on Windows,
        // a read with a long timeout will block a concurrent write from
        // happening.  In wezterm we tend to have a thread looping on read
        // while writes happen occasionally from the gui thread, and if we
        // make this timeout too long we can block the gui thread.
        port.set_timeout(Duration::from_millis(50))?;

        let port: Handle = Arc::new(Mutex::new(port));

        Ok(PtyPair {
            slave: Box::new(Slave {
                port: Arc::clone(&port),
            }),
            master: Box::new(Master { port }),
        })
    }
}

struct Slave {
    port: Handle,
}

impl SlavePty for Slave {
    fn spawn_command(&self, cmd: CommandBuilder) -> anyhow::Result<Box<dyn Child + Send + Sync>> {
        ensure!(
            cmd.is_default_prog(),
            "can only use default prog commands with serial tty implementations"
        );
        Ok(Box::new(SerialChild {
            _port: Arc::clone(&self.port),
        }))
    }
}

/// There isn't really a child process on the end of the serial connection,
/// so all of the Child trait impls are NOP
struct SerialChild {
    _port: Handle,
}

// An anemic impl of Debug to satisfy some indirect trait bounds
impl std::fmt::Debug for SerialChild {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        fmt.debug_struct("SerialChild").finish()
    }
}

impl Child for SerialChild {
    fn try_wait(&mut self) -> IoResult<Option<ExitStatus>> {
        Ok(None)
    }

    fn kill(&mut self) -> IoResult<()> {
        Ok(())
    }

    fn wait(&mut self) -> IoResult<ExitStatus> {
        Err(std::io::Error::new(
            std::io::ErrorKind::WouldBlock,
            "cannot wait for a serial connection to die",
        ))
    }

    fn process_id(&self) -> Option<u32> {
        None
    }
}

struct Master {
    port: Handle,
}

impl Write for Master {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.port.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.port.lock().unwrap().flush()
    }
}

impl MasterPty for Master {
    fn resize(&self, _size: PtySize) -> anyhow::Result<()> {
        // Serial ports have no concept of size
        Ok(())
    }

    fn get_size(&self) -> anyhow::Result<PtySize> {
        // Serial ports have no concept of size
        Ok(PtySize::default())
    }

    fn try_clone_reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
        // We rely on the fact that SystemPort implements the traits
        // that expose the underlying file descriptor, and that direct
        // reads from that return the raw data that we want
        let fd = FileDescriptor::dup(&*self.port.lock().unwrap())?;
        Ok(Box::new(Reader { fd }))
    }

    fn try_clone_writer(&self) -> anyhow::Result<Box<dyn std::io::Write + Send>> {
        let port = Arc::clone(&self.port);
        Ok(Box::new(Master { port }))
    }

    #[cfg(unix)]
    fn process_group_leader(&self) -> Option<libc::pid_t> {
        // N/A: there is no local process
        None
    }
}

struct Reader {
    fd: FileDescriptor,
}

impl Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        loop {
            match self.fd.read(buf) {
                Ok(size) => {
                    if size == 0 {
                        // Read timeout, but we expect to mostly hit this.
                        // It just means that there was no data available
                        // right now.
                        continue;
                    }
                    return Ok(size);
                }
                Err(e) => {
                    log::error!("serial read error: {}", e);
                    return Err(e);
                }
            }
        }
    }
}
