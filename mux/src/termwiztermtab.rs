//! a tab hosting a termwiz terminal applet
//! The idea is to use these when wezterm needs to request
//! input from the user as part of eg: setting up an ssh
//! session.

use crate::domain::{alloc_domain_id, Domain, DomainId, DomainState};
use crate::pane::{
    alloc_pane_id, CachePolicy, CloseReason, ForEachPaneLogicalLine, LogicalLine, Pane, PaneId,
    WithPaneLines,
};
use crate::renderable::*;
use crate::tab::Tab;
use crate::window::WindowId;
use crate::Mux;
use anyhow::bail;
use async_trait::async_trait;
use config::keyassignment::ScrollbackEraseMode;
use crossbeam::channel::{unbounded as channel, Receiver, Sender};
use filedescriptor::{FileDescriptor, Pipe};
use parking_lot::{MappedMutexGuard, Mutex, MutexGuard};
use portable_pty::*;
use rangeset::RangeSet;
use std::io::{BufWriter, Write};
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;
use termwiz::input::{InputEvent, KeyEvent, Modifiers, MouseEvent as TermWizMouseEvent};
use termwiz::render::terminfo::TerminfoRenderer;
use termwiz::surface::{Change, Line, SequenceNo};
use termwiz::terminal::{ScreenSize, TerminalWaker};
use termwiz::Context;
use url::Url;
use wezterm_term::color::ColorPalette;
use wezterm_term::{
    KeyCode, KeyModifiers, MouseEvent, StableRowIndex, TerminalConfiguration, TerminalSize,
};

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
    async fn spawn_pane(
        &self,
        _size: TerminalSize,
        _command: Option<CommandBuilder>,
        _command_dir: Option<String>,
    ) -> anyhow::Result<Arc<dyn Pane>> {
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
    async fn attach(&self, _window_id: Option<WindowId>) -> anyhow::Result<()> {
        Ok(())
    }

    fn detachable(&self) -> bool {
        false
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
    terminal: Mutex<wezterm_term::Terminal>,
    input_tx: Sender<InputEvent>,
    dead: Mutex<bool>,
    writer: Mutex<Vec<u8>>,
    render_rx: FileDescriptor,
}

impl TermWizTerminalPane {
    fn new(
        domain_id: DomainId,
        size: TerminalSize,
        input_tx: Sender<InputEvent>,
        render_rx: FileDescriptor,
        term_config: Option<Arc<dyn TerminalConfiguration + Send + Sync>>,
    ) -> Self {
        let pane_id = alloc_pane_id();

        let terminal = Mutex::new(wezterm_term::Terminal::new(
            size,
            term_config.unwrap_or_else(|| Arc::new(config::TermConfig::new())),
            "WezTerm",
            config::wezterm_version(),
            Box::new(Vec::new()), // FIXME: connect to something?
        ));

        Self {
            pane_id,
            domain_id,
            terminal,
            writer: Mutex::new(Vec::new()),
            render_rx,
            input_tx,
            dead: Mutex::new(false),
        }
    }
}

impl Pane for TermWizTerminalPane {
    fn pane_id(&self) -> PaneId {
        self.pane_id
    }

    fn get_cursor_position(&self) -> StableCursorPosition {
        terminal_get_cursor_position(&mut self.terminal.lock())
    }

    fn get_current_seqno(&self) -> SequenceNo {
        self.terminal.lock().current_seqno()
    }

    fn get_changed_since(
        &self,
        lines: Range<StableRowIndex>,
        seqno: SequenceNo,
    ) -> RangeSet<StableRowIndex> {
        terminal_get_dirty_lines(&mut self.terminal.lock(), lines, seqno)
    }

    fn for_each_logical_line_in_stable_range_mut(
        &self,
        lines: Range<StableRowIndex>,
        for_line: &mut dyn ForEachPaneLogicalLine,
    ) {
        terminal_for_each_logical_line_in_stable_range_mut(
            &mut self.terminal.lock(),
            lines,
            for_line,
        );
    }

    fn get_logical_lines(&self, lines: Range<StableRowIndex>) -> Vec<LogicalLine> {
        crate::pane::impl_get_logical_lines_via_get_lines(self, lines)
    }

    fn with_lines_mut(&self, lines: Range<StableRowIndex>, with_lines: &mut dyn WithPaneLines) {
        terminal_with_lines_mut(&mut self.terminal.lock(), lines, with_lines)
    }

    fn get_lines(&self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
        terminal_get_lines(&mut self.terminal.lock(), lines)
    }

    fn get_dimensions(&self) -> RenderableDimensions {
        terminal_get_dimensions(&mut self.terminal.lock())
    }

    fn get_title(&self) -> String {
        self.terminal.lock().get_title().to_string()
    }

    fn can_close_without_prompting(&self, _reason: CloseReason) -> bool {
        true
    }

    fn send_paste(&self, text: &str) -> anyhow::Result<()> {
        let paste = InputEvent::Paste(text.to_string());
        self.input_tx.send(paste)?;
        Ok(())
    }

    fn reader(&self) -> anyhow::Result<Option<Box<dyn std::io::Read + Send>>> {
        Ok(Some(Box::new(self.render_rx.try_clone()?)))
    }

    fn writer(&self) -> MappedMutexGuard<dyn std::io::Write> {
        MutexGuard::map(self.writer.lock(), |writer| {
            let w: &mut dyn std::io::Write = writer;
            w
        })
    }

    fn resize(&self, size: TerminalSize) -> anyhow::Result<()> {
        self.input_tx.send(InputEvent::Resized {
            rows: size.rows as usize,
            cols: size.cols as usize,
        })?;

        self.terminal.lock().resize(size);

        Ok(())
    }

    fn key_down(&self, key: KeyCode, modifiers: KeyModifiers) -> anyhow::Result<()> {
        let event = InputEvent::Key(KeyEvent {
            key,
            modifiers: modifiers.remove_positional_mods(),
        });
        if let Err(e) = self.input_tx.send(event) {
            *self.dead.lock() = true;
            return Err(e.into());
        }
        Ok(())
    }

    fn key_up(&self, _key: KeyCode, _modifiers: KeyModifiers) -> anyhow::Result<()> {
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
            MouseButton::WheelLeft(_) => Buttons::HORZ_WHEEL | Buttons::WHEEL_POSITIVE,
            MouseButton::WheelRight(_) => Buttons::HORZ_WHEEL,
            MouseButton::None => Buttons::NONE,
        };

        let event = InputEvent::Mouse(TermWizMouseEvent {
            x: event.x as u16,
            y: event.y as u16,
            mouse_buttons,
            modifiers: event.modifiers,
        });
        if let Err(e) = self.input_tx.send(event) {
            *self.dead.lock() = true;
            return Err(e.into());
        }
        Ok(())
    }

    fn set_config(&self, config: Arc<dyn TerminalConfiguration>) {
        self.terminal.lock().set_config(config);
    }

    fn get_config(&self) -> Option<Arc<dyn TerminalConfiguration>> {
        Some(self.terminal.lock().get_config())
    }

    fn perform_actions(&self, actions: Vec<termwiz::escape::Action>) {
        self.terminal.lock().perform_actions(actions)
    }

    fn kill(&self) {
        *self.dead.lock() = true;
    }

    fn is_dead(&self) -> bool {
        *self.dead.lock()
    }

    fn palette(&self) -> ColorPalette {
        self.terminal.lock().palette()
    }

    fn domain_id(&self) -> DomainId {
        self.domain_id
    }

    fn is_mouse_grabbed(&self) -> bool {
        self.terminal.lock().is_mouse_grabbed()
    }

    fn is_alt_screen_active(&self) -> bool {
        self.terminal.lock().is_alt_screen_active()
    }

    fn get_current_working_dir(&self, _policy: CachePolicy) -> Option<Url> {
        self.terminal.lock().get_current_dir().cloned()
    }

    fn erase_scrollback(&self, erase_mode: ScrollbackEraseMode) {
        match erase_mode {
            ScrollbackEraseMode::ScrollbackOnly => {
                self.terminal.lock().erase_scrollback();
            }
            ScrollbackEraseMode::ScrollbackAndViewport => {
                self.terminal.lock().erase_scrollback_and_viewport();
            }
        }
    }
}

pub struct TermWizTerminal {
    render_tx: TermWizTerminalRenderTty,
    input_rx: Receiver<InputEvent>,
    renderer: TerminfoRenderer,
    grab_mouse: bool,
}

impl TermWizTerminal {
    pub fn no_grab_mouse_in_raw_mode(&mut self) {
        self.grab_mouse = false;
    }
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
        if self.grab_mouse {
            decset!(AnyEventMouse);
            decset!(SGRMouse);
        }
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
            match i {
                // Urgh, we get normalized-to-lowercase CTRL-c,
                // but eg: termwiz and other terminal input expect
                // to get CTRL-C instead.  Adjust for that here.
                Some(InputEvent::Key(KeyEvent {
                    key: KeyCode::Char(c),
                    modifiers: Modifiers::CTRL,
                })) if c.is_ascii_lowercase() => Some(InputEvent::Key(KeyEvent {
                    key: KeyCode::Char(c.to_ascii_uppercase()),
                    modifiers: Modifiers::CTRL,
                })),
                i @ _ => i,
            }
        })
    }

    fn waker(&self) -> TerminalWaker {
        // TODO: TerminalWaker assumes that we're a SystemTerminal but that
        // isn't the case here.
        panic!("TermWizTerminal::waker called!?");
    }
}

pub fn allocate(
    size: TerminalSize,
    config: Arc<dyn TerminalConfiguration + Send + Sync>,
) -> (TermWizTerminal, Arc<dyn Pane>) {
    let render_pipe = Pipe::new().expect("Pipe creation not to fail");

    let (input_tx, input_rx) = channel();

    let renderer = termwiz_funcs::new_wezterm_terminfo_renderer();

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
        grab_mouse: true,
    };

    let domain_id = 0;
    let pane = TermWizTerminalPane::new(domain_id, size, input_tx, render_pipe.read, Some(config));

    // Add the tab to the mux so that the output is processed
    let pane: Arc<dyn Pane> = Arc::new(pane);

    let mux = Mux::get();
    mux.add_pane(&pane).expect("to be able to add pane to mux");

    (tw_term, pane)
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
    size: TerminalSize,
    window_id: Option<WindowId>,
    f: F,
    term_config: Option<Arc<dyn TerminalConfiguration + Send + Sync>>,
) -> anyhow::Result<T> {
    let render_pipe = Pipe::new().expect("Pipe creation not to fail");
    let render_rx = render_pipe.read;
    let (input_tx, input_rx) = channel();
    let should_close_window = window_id.is_none();

    let renderer = termwiz_funcs::new_wezterm_terminfo_renderer();

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
        grab_mouse: true,
    };

    async fn register_tab(
        input_tx: Sender<InputEvent>,
        render_rx: FileDescriptor,
        size: TerminalSize,
        window_id: Option<WindowId>,
        term_config: Option<Arc<dyn TerminalConfiguration + Send + Sync>>,
    ) -> anyhow::Result<(PaneId, WindowId)> {
        let mux = Mux::get();

        // TODO: make a singleton
        let domain: Arc<dyn Domain> = Arc::new(TermWizTerminalDomain::new());
        mux.add_domain(&domain);

        let window_builder;
        let window_id = match window_id {
            Some(id) => id,
            None => {
                window_builder = mux.new_empty_window(None, None);
                *window_builder
            }
        };

        let pane =
            TermWizTerminalPane::new(domain.domain_id(), size, input_tx, render_rx, term_config);
        let pane: Arc<dyn Pane> = Arc::new(pane);

        let tab = Arc::new(Tab::new(&size));
        tab.assign_pane(&pane);

        mux.add_tab_and_active_pane(&tab)?;
        mux.add_tab_to_window(&tab, window_id)?;

        let mut window = mux
            .get_window_mut(window_id)
            .ok_or_else(|| anyhow::anyhow!("invalid window id {}", window_id))?;
        let tab_idx = window.len().saturating_sub(1);
        window.save_and_then_set_active(tab_idx);

        Ok((pane.pane_id(), window_id))
    }

    let (pane_id, window_id) = promise::spawn::spawn_into_main_thread(async move {
        register_tab(input_tx, render_rx, size, window_id, term_config).await
    })
    .await?;

    let result = promise::spawn::spawn_into_new_thread(move || f(tw_term)).await;

    // Since we're typically called with an outstanding Activity token active,
    // the dead status of the tab will be ignored until after the activity
    // resolves.  In the case of SSH where (currently!) several prompts may
    // be shown in succession, we don't want to leave lingering dead windows
    // on the screen so let's ask the mux to kill off our window now.
    promise::spawn::spawn_into_main_thread(async move {
        let mux = Mux::get();
        if should_close_window {
            mux.kill_window(window_id);
        } else if let Some(pane) = mux.get_pane(pane_id) {
            pane.kill();
            mux.remove_pane(pane.pane_id());
        }
    })
    .detach();

    result
}
