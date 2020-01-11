//! a tab hosting a termwiz terminal applet
//! The idea is to use these when wezterm needs to request
//! input from the user as part of eg: setting up an ssh
//! session.

use crate::config::configuration;
use crate::font::FontConfiguration;
use crate::frontend::{executor, front_end};
use crate::mux::domain::{alloc_domain_id, Domain, DomainId, DomainState};
use crate::mux::renderable::{Renderable, RenderableDimensions, StableCursorPosition};
use crate::mux::tab::{alloc_tab_id, Tab, TabId};
use crate::mux::window::WindowId;
use crate::mux::Mux;
use anyhow::{bail, Error};
use crossbeam_channel::{unbounded as channel, Receiver, Sender};
use filedescriptor::Pipe;
use portable_pty::*;
use promise::{Future, Promise};
use rangeset::RangeSet;
use std::cell::RefCell;
use std::cell::RefMut;
use std::convert::TryInto;
use std::ops::Range;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use term::color::ColorPalette;
use term::{KeyCode, KeyModifiers, Line, MouseEvent, StableRowIndex, TerminalHost};
use termwiz::input::{InputEvent, KeyEvent};
use termwiz::lineedit::*;
use termwiz::surface::{Change, SequenceNo, Surface};
use termwiz::terminal::{ScreenSize, Terminal, TerminalWaker};

struct RenderableInner {
    surface: Surface,
    something_changed: Arc<AtomicBool>,
    local_sequence: SequenceNo,
    dead: bool,
    render_rx: Receiver<Vec<Change>>,
    input_tx: Sender<InputEvent>,
}

struct RenderableState {
    inner: RefCell<RenderableInner>,
}

impl std::io::Write for RenderableState {
    fn write(&mut self, data: &[u8]) -> Result<usize, std::io::Error> {
        if let Ok(s) = std::str::from_utf8(data) {
            let paste = InputEvent::Paste(s.to_string());
            self.inner.borrow_mut().input_tx.send(paste).ok();
        }

        Ok(data.len())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}

impl Renderable for RenderableState {
    fn get_cursor_position(&self) -> StableCursorPosition {
        let surface = &self.inner.borrow().surface;
        let (x, y) = surface.cursor_position();
        let shape = surface.cursor_shape();
        StableCursorPosition {
            x,
            y: y as StableRowIndex,
            shape,
        }
    }

    fn get_lines(&mut self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
        let inner = self.inner.borrow_mut();

        // Reset the dirty bit
        inner.something_changed.store(false, Ordering::SeqCst);

        let config = configuration();

        (
            lines.start,
            inner
                .surface
                .screen_lines()
                .into_iter()
                .skip(lines.start.try_into().unwrap())
                .take((lines.end - lines.start).try_into().unwrap())
                .map(|line| {
                    let mut line = line.into_owned();
                    line.scan_and_create_hyperlinks(&config.hyperlink_rules);
                    line
                })
                .collect(),
        )
    }

    fn get_dirty_lines(&self, lines: Range<StableRowIndex>) -> RangeSet<StableRowIndex> {
        let mut inner = self.inner.borrow_mut();
        let mut set = RangeSet::new();

        loop {
            match inner.render_rx.try_recv() {
                Ok(changes) => {
                    inner.surface.add_changes(changes);
                }
                Err(err) => {
                    if err.is_disconnected() {
                        inner.dead = true;
                    }
                    break;
                }
            }
        }

        if inner.something_changed.load(Ordering::SeqCst)
            || inner.surface.has_changes(inner.local_sequence)
        {
            set.add_range(lines);
        }
        set
    }

    fn get_dimensions(&self) -> RenderableDimensions {
        let (cols, viewport_rows) = self.inner.borrow().surface.dimensions();
        RenderableDimensions {
            viewport_rows,
            cols,
            scrollback_rows: viewport_rows,
            physical_top: 0,
            scrollback_top: 0,
        }
    }
}

struct TermWizTerminalDomain {
    domain_id: DomainId,
}

impl TermWizTerminalDomain {
    pub fn new() -> Self {
        let domain_id = alloc_domain_id();
        Self { domain_id }
    }
}

impl Domain for TermWizTerminalDomain {
    fn spawn(
        &self,
        _size: PtySize,
        _command: Option<CommandBuilder>,
        _command_dir: Option<String>,
        _window: WindowId,
    ) -> anyhow::Result<Rc<dyn Tab>> {
        bail!("cannot spawn tabs in a TermWizTerminalTab");
    }

    fn domain_id(&self) -> DomainId {
        self.domain_id
    }

    fn domain_name(&self) -> &str {
        "TermWizTerminalDomain"
    }
    fn attach(&self) -> promise::Future<()> {
        promise::Future::ok(())
    }

    fn detach(&self) -> anyhow::Result<()> {
        bail!("detach not implemented for TermWizTerminalDomain");
    }

    fn state(&self) -> DomainState {
        DomainState::Attached
    }
}

pub struct TermWizTerminalTab {
    tab_id: TabId,
    domain_id: DomainId,
    renderable: RefCell<RenderableState>,
    reader: Pipe,
}

impl Drop for TermWizTerminalTab {
    fn drop(&mut self) {
        log::error!("Dropping TermWizTerminalTab");
    }
}

impl TermWizTerminalTab {
    fn new(domain_id: DomainId, inner: RenderableInner) -> Self {
        let tab_id = alloc_tab_id();
        let renderable = RefCell::new(RenderableState {
            inner: RefCell::new(inner),
        });
        let reader = Pipe::new().expect("Pipe::new failed");
        Self {
            tab_id,
            domain_id,
            renderable,
            reader,
        }
    }
}

impl Tab for TermWizTerminalTab {
    fn tab_id(&self) -> TabId {
        self.tab_id
    }

    fn renderer(&self) -> RefMut<dyn Renderable> {
        self.renderable.borrow_mut()
    }

    fn get_title(&self) -> String {
        let renderable = self.renderable.borrow();
        let surface = &renderable.inner.borrow().surface;
        surface.title().to_string()
    }

    fn send_paste(&self, text: &str) -> anyhow::Result<()> {
        let paste = InputEvent::Paste(text.to_string());
        self.renderable
            .borrow_mut()
            .inner
            .borrow_mut()
            .input_tx
            .send(paste)?;
        Ok(())
    }

    fn reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
        Ok(Box::new(self.reader.read.try_clone()?))
    }

    fn writer(&self) -> RefMut<dyn std::io::Write> {
        self.renderable.borrow_mut()
    }

    fn resize(&self, size: PtySize) -> anyhow::Result<()> {
        self.renderable
            .borrow()
            .inner
            .borrow_mut()
            .surface
            .resize(size.cols as usize, size.rows as usize);
        Ok(())
    }

    fn key_down(&self, key: KeyCode, modifiers: KeyModifiers) -> anyhow::Result<()> {
        let event = InputEvent::Key(KeyEvent { key, modifiers });
        self.renderable
            .borrow_mut()
            .inner
            .borrow_mut()
            .input_tx
            .send(event)?;
        Ok(())
    }

    fn mouse_event(&self, _event: MouseEvent, _host: &mut dyn TerminalHost) -> anyhow::Result<()> {
        // FIXME: send mouse events through
        Ok(())
    }

    fn advance_bytes(&self, _buf: &[u8], _host: &mut dyn TerminalHost) {
        panic!("advance_bytes is undefed for TermWizTerminalTab");
    }

    fn is_dead(&self) -> bool {
        self.renderable.borrow().inner.borrow().dead
    }

    fn palette(&self) -> ColorPalette {
        Default::default()
    }

    fn domain_id(&self) -> DomainId {
        self.domain_id
    }

    fn is_mouse_grabbed(&self) -> bool {
        true
    }

    fn get_current_working_dir(&self) -> Option<String> {
        None
    }
}

pub struct TermWizTerminal {
    render_tx: Sender<Vec<Change>>,
    input_rx: Receiver<InputEvent>,
    screen_size: ScreenSize,
}

impl TermWizTerminal {
    fn do_input_poll(&mut self, wait: Option<Duration>) -> anyhow::Result<Option<InputEvent>> {
        if let Some(timeout) = wait {
            match self.input_rx.recv_timeout(timeout) {
                Ok(input) => Ok(Some(input)),
                Err(err) => {
                    if err.is_timeout() {
                        Ok(None)
                    } else {
                        Err(err.into())
                    }
                }
            }
        } else {
            let input = self.input_rx.recv()?;
            Ok(Some(input))
        }
    }
}

impl termwiz::terminal::Terminal for TermWizTerminal {
    fn set_raw_mode(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn set_cooked_mode(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn enter_alternate_screen(&mut self) -> anyhow::Result<()> {
        bail!("TermWizTerminalTab has no alt screen");
    }

    fn exit_alternate_screen(&mut self) -> anyhow::Result<()> {
        bail!("TermWizTerminalTab has no alt screen");
    }

    fn get_screen_size(&mut self) -> anyhow::Result<ScreenSize> {
        Ok(self.screen_size)
    }

    fn set_screen_size(&mut self, _size: ScreenSize) -> anyhow::Result<()> {
        bail!("TermWizTerminalTab cannot set screen size");
    }

    fn render(&mut self, changes: &[Change]) -> anyhow::Result<()> {
        self.render_tx.send(changes.to_vec())?;
        Ok(())
    }

    fn flush(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn poll_input(&mut self, wait: Option<Duration>) -> anyhow::Result<Option<InputEvent>> {
        self.do_input_poll(wait).map(|i| {
            if let Some(InputEvent::Resized { cols, rows }) = i.as_ref() {
                self.screen_size.cols = *cols;
                self.screen_size.rows = *rows;
            }
            i
        })
    }

    fn waker(&self) -> TerminalWaker {
        // TODO: TerminalWaker assumes that we're a SystemTerminal but that
        // isn't the case here.
        panic!("TermWizTerminal::waker called!?");
    }
}

/// This function spawns a thread and constructs a GUI window with an
/// associated termwiz Terminal object to execute the provided function.
/// The function is expected to run in a loop to manage input and output
/// from the terminal window.
/// When it completes its loop it will fulfil a promise and yield
/// the return value from the function.
pub fn run<T: Send + 'static, F: Send + 'static + Fn(TermWizTerminal) -> anyhow::Result<T>>(
    width: usize,
    height: usize,
    f: F,
) -> Future<T> {
    let (render_tx, render_rx) = channel();
    let (input_tx, input_rx) = channel();

    let tw_term = TermWizTerminal {
        render_tx,
        input_rx,
        screen_size: ScreenSize {
            cols: width,
            rows: height,
            xpixel: 0,
            ypixel: 0,
        },
    };

    let mut promise = Promise::new();
    let future = promise.get_future().expect("just made the promise");

    Future::with_executor(executor(), move || {
        let mux = Mux::get().unwrap();

        // TODO: make a singleton
        let domain: Arc<dyn Domain> = Arc::new(TermWizTerminalDomain::new());
        mux.add_domain(&domain);

        let window_id = mux.new_empty_window();

        let inner = RenderableInner {
            surface: Surface::new(width, height),
            local_sequence: 0,
            dead: false,
            something_changed: Arc::new(AtomicBool::new(false)),
            input_tx,
            render_rx,
        };

        let tab: Rc<dyn Tab> = Rc::new(TermWizTerminalTab::new(domain.domain_id(), inner));

        mux.add_tab(&tab)?;
        mux.add_tab_to_window(&tab, window_id)?;

        let fontconfig = Rc::new(FontConfiguration::new());

        let gui = front_end().unwrap();
        gui.spawn_new_window(&fontconfig, &tab, window_id)?;

        Ok(())
    });

    std::thread::spawn(move || {
        promise.result(f(tw_term));
    });

    future
}

pub fn message_box_ok(message: &str) {
    let title = "wezterm";
    let message = message.to_string();

    run(60, 10, move |mut term| {
        term.render(&[
            Change::Title(title.to_string()),
            Change::Text(message.to_string()),
        ])
        .map_err(Error::msg)?;

        let mut editor = LineEditor::new(term);
        editor.set_prompt("press enter to continue.");

        let mut host = NopLineEditorHost::default();
        editor.read_line(&mut host).ok();
        Ok(())
    })
    .wait()
    .ok();
}
