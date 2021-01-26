//! a tab hosting a termwiz terminal applet
//! The idea is to use these when wezterm needs to request
//! input from the user as part of eg: setting up an ssh
//! session.

use crate::domain::{alloc_domain_id, Domain, DomainId, DomainState};
use crate::pane::{alloc_pane_id, Pane, PaneId};
use crate::renderable::*;
use crate::tab::{SplitDirection, Tab, TabId};
use crate::window::WindowId;
use crate::Mux;
use anyhow::bail;
use async_trait::async_trait;
use config::keyassignment::ScrollbackEraseMode;
use crossbeam::channel::{unbounded as channel, Receiver, Sender};
use filedescriptor::{FileDescriptor, Pipe};
use portable_pty::*;
use rangeset::RangeSet;
use std::cell::RefCell;
use std::cell::RefMut;
use std::io::BufWriter;
use std::io::Write;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use termwiz::caps::{Capabilities, ColorLevel, ProbeHints};
use termwiz::input::{InputEvent, KeyEvent, MouseEvent as TermWizMouseEvent};
use termwiz::render::terminfo::TerminfoRenderer;
use termwiz::surface::Change;
use termwiz::surface::Line;
use termwiz::terminal::{ScreenSize, TerminalWaker};
use termwiz::Context;
use url::Url;
use wezterm_term::color::ColorPalette;
use wezterm_term::{KeyCode, KeyModifiers, MouseEvent, StableRowIndex};

struct TermWizTerminalDomain {
    domain_id: DomainId,
}

impl TermWizTerminalDomain {
    pub fn new() -> Self {
        let domain_id = alloc_domain_id();
        Self { domain_id }
    }
}

#[async_trait(?Send)]
impl Domain for TermWizTerminalDomain {
    async fn spawn(
        &self,
        _size: PtySize,
        _command: Option<CommandBuilder>,
        _command_dir: Option<String>,
        _window: WindowId,
    ) -> anyhow::Result<Rc<Tab>> {
        bail!("cannot spawn tabs in a TermWizTerminalPane");
    }
    async fn split_pane(
        &self,
        _command: Option<CommandBuilder>,
        _command_dir: Option<String>,
        _tab: TabId,
        _pane_id: PaneId,
        _split_direction: SplitDirection,
    ) -> anyhow::Result<Rc<dyn Pane>> {
        bail!("cannot spawn panes in a TermWizTerminalPane");
    }

    fn spawnable(&self) -> bool {
        false
    }

    fn domain_id(&self) -> DomainId {
        self.domain_id
    }

    fn domain_name(&self) -> &str {
        "TermWizTerminalDomain"
    }
    async fn attach(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn detach(&self) -> anyhow::Result<()> {
        bail!("detach not implemented for TermWizTerminalDomain");
    }

    fn state(&self) -> DomainState {
        DomainState::Attached
    }
}

pub struct TermWizTerminalPane {
    pane_id: PaneId,
    domain_id: DomainId,
    terminal: RefCell<wezterm_term::Terminal>,
    input_tx: Sender<InputEvent>,
    dead: RefCell<bool>,
    writer: RefCell<Vec<u8>>,
    render_rx: FileDescriptor,
}

impl TermWizTerminalPane {
    fn new(
        domain_id: DomainId,
        size: PtySize,
        input_tx: Sender<InputEvent>,
        render_rx: FileDescriptor,
    ) -> Self {
        let pane_id = alloc_pane_id();

        let terminal = RefCell::new(wezterm_term::Terminal::new(
            crate::pty_size_to_terminal_size(size),
            std::sync::Arc::new(config::TermConfig {}),
            "WezTerm",
            config::wezterm_version(),
            Box::new(Vec::new()), // FIXME: connect to something?
        ));

        Self {
            pane_id,
            domain_id,
            terminal,
            writer: RefCell::new(Vec::new()),
            render_rx,
            input_tx,
            dead: RefCell::new(false),
        }
    }
}

impl Pane for TermWizTerminalPane {
    fn pane_id(&self) -> PaneId {
        self.pane_id
    }

    fn get_cursor_position(&self) -> StableCursorPosition {
        terminal_get_cursor_position(&mut self.terminal.borrow_mut())
    }

    fn get_dirty_lines(&self, lines: Range<StableRowIndex>) -> RangeSet<StableRowIndex> {
        terminal_get_dirty_lines(&mut self.terminal.borrow_mut(), lines)
    }

    fn get_lines(&self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
        terminal_get_lines(&mut self.terminal.borrow_mut(), lines)
    }

    fn get_dimensions(&self) -> RenderableDimensions {
        terminal_get_dimensions(&mut self.terminal.borrow_mut())
    }

    fn get_title(&self) -> String {
        self.terminal.borrow_mut().get_title().to_string()
    }

    fn send_paste(&self, text: &str) -> anyhow::Result<()> {
        let paste = InputEvent::Paste(text.to_string());
        self.input_tx.send(paste)?;
        Ok(())
    }

    fn reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
        Ok(Box::new(self.render_rx.try_clone()?))
    }

    fn writer(&self) -> RefMut<dyn std::io::Write> {
        self.writer.borrow_mut()
    }

    fn resize(&self, size: PtySize) -> anyhow::Result<()> {
        self.input_tx.send(InputEvent::Resized {
            rows: size.rows as usize,
            cols: size.cols as usize,
        })?;

        self.terminal.borrow_mut().resize(
            size.rows as usize,
            size.cols as usize,
            size.pixel_width as usize,
            size.pixel_height as usize,
        );

        Ok(())
    }

    fn key_down(&self, key: KeyCode, modifiers: KeyModifiers) -> anyhow::Result<()> {
        let event = InputEvent::Key(KeyEvent { key, modifiers });
        if let Err(e) = self.input_tx.send(event) {
            *self.dead.borrow_mut() = true;
            return Err(e.into());
        }
        Ok(())
    }

    fn mouse_event(&self, event: MouseEvent) -> anyhow::Result<()> {
        use termwiz::input::MouseButtons as Buttons;
        use wezterm_term::input::MouseButton;

        let mouse_buttons = match event.button {
            MouseButton::Left => Buttons::LEFT,
            MouseButton::Middle => Buttons::MIDDLE,
            MouseButton::Right => Buttons::RIGHT,
            MouseButton::WheelUp(_) => Buttons::VERT_WHEEL | Buttons::WHEEL_POSITIVE,
            MouseButton::WheelDown(_) => Buttons::VERT_WHEEL,
            MouseButton::None => Buttons::NONE,
        };

        let event = InputEvent::Mouse(TermWizMouseEvent {
            x: event.x as u16,
            y: event.y as u16,
            mouse_buttons,
            modifiers: event.modifiers,
        });
        if let Err(e) = self.input_tx.send(event) {
            *self.dead.borrow_mut() = true;
            return Err(e.into());
        }
        Ok(())
    }

    fn advance_bytes(&self, buf: &[u8]) {
        self.terminal.borrow_mut().advance_bytes(buf)
    }

    fn is_dead(&self) -> bool {
        *self.dead.borrow()
    }

    fn palette(&self) -> ColorPalette {
        self.terminal.borrow().palette()
    }

    fn domain_id(&self) -> DomainId {
        self.domain_id
    }

    fn is_mouse_grabbed(&self) -> bool {
        self.terminal.borrow().is_mouse_grabbed()
    }

    fn is_alt_screen_active(&self) -> bool {
        self.terminal.borrow().is_alt_screen_active()
    }

    fn get_current_working_dir(&self) -> Option<Url> {
        self.terminal.borrow().get_current_dir().cloned()
    }

    fn erase_scrollback(&self, erase_mode: ScrollbackEraseMode) {
        match erase_mode {
            ScrollbackEraseMode::ScrollbackOnly => {
                self.terminal.borrow_mut().erase_scrollback();
            }
            ScrollbackEraseMode::ScrollbackAndViewport => {
                self.terminal.borrow_mut().erase_scrollback_and_viewport();
            }
        }
    }
}

pub struct TermWizTerminal {
    render_tx: TermWizTerminalRenderTty,
    input_rx: Receiver<InputEvent>,
    renderer: TerminfoRenderer,
}

struct TermWizTerminalRenderTty {
    render_tx: BufWriter<FileDescriptor>,
    screen_size: ScreenSize,
}

impl std::io::Write for TermWizTerminalRenderTty {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.render_tx.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.render_tx.flush()
    }
}

impl termwiz::render::RenderTty for TermWizTerminalRenderTty {
    fn get_size_in_cells(&mut self) -> termwiz::Result<(usize, usize)> {
        Ok((self.screen_size.cols, self.screen_size.rows))
    }
}

impl TermWizTerminal {
    fn do_input_poll(&mut self, wait: Option<Duration>) -> termwiz::Result<Option<InputEvent>> {
        if let Some(timeout) = wait {
            match self.input_rx.recv_timeout(timeout) {
                Ok(input) => Ok(Some(input)),
                Err(err) => {
                    if err.is_timeout() {
                        Ok(None)
                    } else {
                        Err(err).context("receive from channel")
                    }
                }
            }
        } else {
            let input = self.input_rx.recv().context("receive from channel")?;
            Ok(Some(input))
        }
    }
}

impl termwiz::terminal::Terminal for TermWizTerminal {
    fn set_raw_mode(&mut self) -> termwiz::Result<()> {
        use termwiz::escape::csi::{DecPrivateMode, DecPrivateModeCode, Mode, CSI};

        macro_rules! decset {
            ($variant:ident) => {
                write!(
                    self.render_tx,
                    "{}",
                    CSI::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
                        DecPrivateModeCode::$variant
                    )))
                )?;
            };
        }

        decset!(BracketedPaste);
        decset!(AnyEventMouse);
        decset!(SGRMouse);
        self.flush()?;

        Ok(())
    }

    fn set_cooked_mode(&mut self) -> termwiz::Result<()> {
        Ok(())
    }

    fn enter_alternate_screen(&mut self) -> termwiz::Result<()> {
        termwiz::bail!("TermWizTerminalPane has no alt screen");
    }

    fn exit_alternate_screen(&mut self) -> termwiz::Result<()> {
        termwiz::bail!("TermWizTerminalPane has no alt screen");
    }

    fn get_screen_size(&mut self) -> termwiz::Result<ScreenSize> {
        Ok(self.render_tx.screen_size)
    }

    fn set_screen_size(&mut self, _size: ScreenSize) -> termwiz::Result<()> {
        termwiz::bail!("TermWizTerminalPane cannot set screen size");
    }

    fn render(&mut self, changes: &[Change]) -> termwiz::Result<()> {
        self.renderer.render_to(changes, &mut self.render_tx)?;
        Ok(())
    }

    fn flush(&mut self) -> termwiz::Result<()> {
        self.render_tx.render_tx.flush()?;
        Ok(())
    }

    fn poll_input(&mut self, wait: Option<Duration>) -> termwiz::Result<Option<InputEvent>> {
        self.do_input_poll(wait).map(|i| {
            if let Some(InputEvent::Resized { cols, rows }) = i.as_ref() {
                self.render_tx.screen_size.cols = *cols;
                self.render_tx.screen_size.rows = *rows;
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

pub fn allocate(size: PtySize) -> (TermWizTerminal, Rc<dyn Pane>) {
    let render_pipe = Pipe::new().expect("Pipe creation not to fail");

    let (input_tx, input_rx) = channel();

    let renderer = new_wezterm_terminfo_renderer();

    let tw_term = TermWizTerminal {
        render_tx: TermWizTerminalRenderTty {
            render_tx: BufWriter::new(render_pipe.write),
            screen_size: ScreenSize {
                cols: size.cols as usize,
                rows: size.rows as usize,
                xpixel: (size.pixel_width / size.cols) as usize,
                ypixel: (size.pixel_height / size.rows) as usize,
            },
        },
        input_rx,
        renderer,
    };

    let domain_id = 0;
    let pane = TermWizTerminalPane::new(domain_id, size, input_tx, render_pipe.read);

    // Add the tab to the mux so that the output is processed
    let pane: Rc<dyn Pane> = Rc::new(pane);

    let mux = Mux::get().unwrap();
    mux.add_pane(&pane).expect("to be able to add pane to mux");

    (tw_term, pane)
}

fn new_wezterm_terminfo_renderer() -> TerminfoRenderer {
    let data = include_bytes!("../../termwiz/data/xterm-256color");
    let db = terminfo::Database::from_buffer(&data[..]).unwrap();

    TerminfoRenderer::new(
        Capabilities::new_with_hints(
            ProbeHints::new_from_env()
                .term(Some("xterm-256color".into()))
                .terminfo_db(Some(db))
                .color_level(Some(ColorLevel::TrueColor))
                .colorterm(None)
                .colorterm_bce(None)
                .term_program(Some("WezTerm".into()))
                .term_program_version(Some(config::wezterm_version().into())),
        )
        .expect("cannot fail to make internal Capabilities"),
    )
}

/// This function spawns a thread and constructs a GUI window with an
/// associated termwiz Terminal object to execute the provided function.
/// The function is expected to run in a loop to manage input and output
/// from the terminal window.
/// When it completes its loop it will fulfil a promise and yield
/// the return value from the function.
pub async fn run<
    T: Send + 'static,
    F: Send + 'static + FnOnce(TermWizTerminal) -> anyhow::Result<T>,
>(
    size: PtySize,
    f: F,
) -> anyhow::Result<T> {
    let render_pipe = Pipe::new().expect("Pipe creation not to fail");
    let render_rx = render_pipe.read;
    let (input_tx, input_rx) = channel();

    let renderer = new_wezterm_terminfo_renderer();

    let tw_term = TermWizTerminal {
        render_tx: TermWizTerminalRenderTty {
            render_tx: BufWriter::new(render_pipe.write),
            screen_size: ScreenSize {
                cols: size.cols as usize,
                rows: size.rows as usize,
                xpixel: (size.pixel_width / size.cols) as usize,
                ypixel: (size.pixel_height / size.rows) as usize,
            },
        },
        input_rx,
        renderer,
    };

    async fn register_tab(
        input_tx: Sender<InputEvent>,
        render_rx: FileDescriptor,
        size: PtySize,
    ) -> anyhow::Result<WindowId> {
        let mux = Mux::get().unwrap();

        // TODO: make a singleton
        let domain: Arc<dyn Domain> = Arc::new(TermWizTerminalDomain::new());
        mux.add_domain(&domain);

        let window_id = mux.new_empty_window();

        let pane = TermWizTerminalPane::new(domain.domain_id(), size, input_tx, render_rx);
        let pane: Rc<dyn Pane> = Rc::new(pane);

        let tab = Rc::new(Tab::new(&size));
        tab.assign_pane(&pane);

        mux.add_tab_and_active_pane(&tab)?;
        mux.add_tab_to_window(&tab, *window_id)?;

        Ok(*window_id)
    }

    let window_id: WindowId = promise::spawn::spawn_into_main_thread(async move {
        register_tab(input_tx, render_rx, size).await
    })
    .await?;

    let result = promise::spawn::spawn_into_new_thread(move || f(tw_term)).await;

    // Since we're typically called with an outstanding Activity token active,
    // the dead status of the tab will be ignored until after the activity
    // resolves.  In the case of SSH where (currently!) several prompts may
    // be shown in succession, we don't want to leave lingering dead windows
    // on the screen so let's ask the mux to kill off our window now.
    promise::spawn::spawn_into_main_thread(async move {
        let mux = Mux::get().unwrap();
        mux.kill_window(window_id);
    })
    .detach();

    result
}
