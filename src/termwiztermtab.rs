//! a tab hosting a termwiz terminal applet
//! The idea is to use these when wezterm needs to request
//! input from the user as part of eg: setting up an ssh
//! session.

use crate::font::FontConfiguration;
use crate::frontend::{executor, front_end};
use crate::mux::domain::{alloc_domain_id, Domain, DomainId, DomainState};
use crate::mux::renderable::Renderable;
use crate::mux::tab::{alloc_tab_id, Tab, TabId};
use crate::mux::window::WindowId;
use crate::mux::Mux;
use anyhow::{bail, Error};
use crossbeam_channel::{unbounded as channel, Receiver, Sender};
use filedescriptor::Pipe;
use portable_pty::*;
use promise::{Future, Promise};
use std::borrow::Cow;
use std::cell::RefCell;
use std::cell::RefMut;
use std::ops::Range;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use term::color::ColorPalette;
use term::selection::SelectionRange;
use term::{
    CursorPosition, KeyCode, KeyModifiers, Line, MouseEvent, TerminalHost, VisibleRowIndex,
};
use termwiz::hyperlink::Hyperlink;
use termwiz::input::{InputEvent, KeyEvent};
use termwiz::lineedit::*;
use termwiz::surface::{Change, SequenceNo, Surface};
use termwiz::terminal::{ScreenSize, Terminal, TerminalWaker};

struct RenderableInner {
    surface: Surface,
    selection_range: Arc<Mutex<Option<SelectionRange>>>,
    something_changed: Arc<AtomicBool>,
    highlight: Arc<Mutex<Option<Arc<Hyperlink>>>>,
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
    fn get_cursor_position(&self) -> CursorPosition {
        let surface = &self.inner.borrow().surface;
        let (x, y) = surface.cursor_position();
        let shape = surface.cursor_shape();
        CursorPosition {
            x,
            y: y as i64,
            shape,
        }
    }

    fn get_dirty_lines(&self) -> Vec<(usize, Cow<Line>, Range<usize>)> {
        let mut inner = self.inner.borrow_mut();
        let seq = inner.surface.current_seqno();
        inner.surface.flush_changes_older_than(seq);
        let selection = *inner.selection_range.lock().unwrap();
        inner.something_changed.store(false, Ordering::SeqCst);
        inner.local_sequence = seq;
        inner
            .surface
            .screen_lines()
            .into_iter()
            .enumerate()
            .map(|(idx, line)| {
                let r = match selection {
                    None => 0..0,
                    Some(sel) => sel.normalize().cols_for_row(idx as i32),
                };
                (idx, Cow::Owned(line.into_owned()), r)
            })
            .collect()
    }

    fn has_dirty_lines(&self) -> bool {
        let mut inner = self.inner.borrow_mut();

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

        if inner.something_changed.load(Ordering::SeqCst) {
            return true;
        }
        inner.surface.has_changes(inner.local_sequence)
    }

    fn make_all_lines_dirty(&mut self) {
        self.inner
            .borrow()
            .something_changed
            .store(true, Ordering::SeqCst);
    }

    fn clean_dirty_lines(&mut self) {}

    fn current_highlight(&self) -> Option<Arc<Hyperlink>> {
        self.inner
            .borrow()
            .highlight
            .lock()
            .unwrap()
            .as_ref()
            .cloned()
    }

    fn physical_dimensions(&self) -> (usize, usize) {
        let (cols, rows) = self.inner.borrow().surface.dimensions();
        (rows, cols)
    }

    fn get_scrollbar_info(&self) -> (VisibleRowIndex, usize) {
        let (_cols, rows) = self.physical_dimensions();
        (0, rows)
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
    fn attach(&self) -> anyhow::Result<()> {
        Ok(())
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

    fn selection_range(&self) -> Option<SelectionRange> {
        *self
            .renderable
            .borrow()
            .inner
            .borrow()
            .selection_range
            .lock()
            .unwrap()
    }

    fn selection_text(&self) -> Option<String> {
        // FIXME: grab it from the surface
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
            highlight: Arc::new(Mutex::new(None)),
            local_sequence: 0,
            dead: false,
            something_changed: Arc::new(AtomicBool::new(false)),
            selection_range: Arc::new(Mutex::new(None)),
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
