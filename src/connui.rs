use crate::termwiztermtab;
use anyhow::{anyhow, bail, Context as _};
use crossbeam::channel::{bounded, Receiver, Sender};
use promise::Promise;
use std::time::Duration;
use termwiz::cell::unicode_column_width;
use termwiz::lineedit::*;
use termwiz::surface::Change;
use termwiz::terminal::*;

#[derive(Default)]
struct PasswordPromptHost {
    history: BasicHistory,
}
impl LineEditorHost for PasswordPromptHost {
    fn history(&mut self) -> &mut dyn History {
        &mut self.history
    }

    // Rewrite the input so that we can obscure the password
    // characters when output to the terminal widget
    fn highlight_line(&self, line: &str, cursor_position: usize) -> (Vec<OutputElement>, usize) {
        let placeholder = "🔑";
        let grapheme_count = unicode_column_width(line);
        let mut output = vec![];
        for _ in 0..grapheme_count {
            output.push(OutputElement::Text(placeholder.to_string()));
        }
        (output, unicode_column_width(placeholder) * cursor_position)
    }
}

pub enum UIRequest {
    /// Display something
    Output(Vec<Change>),
    /// Request input
    Input {
        prompt: String,
        echo: bool,
        respond: Promise<String>,
    },
    Close,
}

struct ConnectionUIImpl {
    term: termwiztermtab::TermWizTerminal,
    rx: Receiver<UIRequest>,
}

impl ConnectionUIImpl {
    fn run(&mut self) -> anyhow::Result<()> {
        loop {
            match self.rx.recv_timeout(Duration::from_millis(200)) {
                Ok(UIRequest::Close) => break,
                Ok(UIRequest::Output(changes)) => self.term.render(&changes)?,
                Ok(UIRequest::Input {
                    prompt,
                    echo: true,
                    mut respond,
                }) => {
                    respond.result(self.input_prompt(&prompt));
                }
                Ok(UIRequest::Input {
                    prompt,
                    echo: false,
                    mut respond,
                }) => {
                    respond.result(self.password_prompt(&prompt));
                }
                Err(err) if err.is_timeout() => {}
                Err(err) => bail!("recv_timeout: {}", err),
            }
        }
        Ok(())
    }

    fn password_prompt(&mut self, prompt: &str) -> anyhow::Result<String> {
        let mut editor = LineEditor::new(&mut self.term);
        editor.set_prompt(prompt);

        let mut host = PasswordPromptHost::default();
        if let Some(line) = editor.read_line(&mut host)? {
            Ok(line)
        } else {
            bail!("password entry was cancelled");
        }
    }

    fn input_prompt(&mut self, prompt: &str) -> anyhow::Result<String> {
        let mut editor = LineEditor::new(&mut self.term);
        editor.set_prompt(prompt);

        let mut host = NopLineEditorHost::default();
        if let Some(line) = editor.read_line(&mut host)? {
            Ok(line)
        } else {
            bail!("prompt cancelled");
        }
    }
}

struct HeadlessImpl {
    rx: Receiver<UIRequest>,
}

impl HeadlessImpl {
    fn run(&mut self) -> anyhow::Result<()> {
        loop {
            match self.rx.recv_timeout(Duration::from_millis(200)) {
                Ok(UIRequest::Close) => break,
                Ok(UIRequest::Output(changes)) => {
                    log::trace!("Output: {:?}", changes);
                }
                Ok(UIRequest::Input { mut respond, .. }) => {
                    respond.result(Err(anyhow!("Input requested from headless context")));
                }
                Err(err) if err.is_timeout() => {}
                Err(err) => bail!("recv_timeout: {}", err),
            }
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct ConnectionUI {
    tx: Sender<UIRequest>,
}

impl ConnectionUI {
    pub fn new() -> Self {
        let (tx, rx) = bounded(16);
        promise::spawn::spawn_into_main_thread(termwiztermtab::run(80, 24, move |term| {
            let mut ui = ConnectionUIImpl { term, rx };
            if let Err(e) = ui.run() {
                log::error!("while running ConnectionUI loop: {:?}", e);
            }
            std::thread::sleep(Duration::new(10, 0));
            Ok(())
        }));
        Self { tx }
    }

    pub fn new_headless() -> Self {
        let (tx, rx) = bounded(16);
        std::thread::spawn(move || {
            let mut ui = HeadlessImpl { rx };
            ui.run()
        });
        Self { tx }
    }

    pub fn title(&self, title: &str) {
        self.output(vec![Change::Title(title.to_string())]);
    }

    pub fn output(&self, changes: Vec<Change>) {
        self.tx.send(UIRequest::Output(changes)).ok();
    }

    pub fn output_str(&self, s: &str) {
        let s = s.replace("\n", "\r\n");
        self.output(vec![Change::Text(s)]);
    }

    /// Crack a multi-line prompt into an optional preamble and the prompt
    /// text on the final line.  This is needed because the line editor
    /// is only designed for a single line prompt; a multi-line prompt
    /// messes up the cursor positioning.
    fn split_multi_line_prompt(s: &str) -> (Option<String>, String) {
        let text = s.replace("\n", "\r\n");
        let bits: Vec<&str> = text.rsplitn(2, "\r\n").collect();

        if bits.len() == 2 {
            (Some(format!("{}\r\n", bits[1])), bits[0].to_owned())
        } else {
            (None, text)
        }
    }

    pub fn input(&self, prompt: &str) -> anyhow::Result<String> {
        let mut promise = Promise::new();
        let future = promise.get_future().unwrap();

        let (preamble, prompt) = Self::split_multi_line_prompt(prompt);
        if let Some(preamble) = preamble {
            self.output(vec![Change::Text(preamble)]);
        }

        self.tx
            .send(UIRequest::Input {
                prompt,
                echo: true,
                respond: promise,
            })
            .context("send to ConnectionUI failed")?;

        future.wait()
    }

    pub fn password(&self, prompt: &str) -> anyhow::Result<String> {
        let mut promise = Promise::new();
        let future = promise.get_future().unwrap();

        let (preamble, prompt) = Self::split_multi_line_prompt(prompt);
        if let Some(preamble) = preamble {
            self.output(vec![Change::Text(preamble)]);
        }

        self.tx
            .send(UIRequest::Input {
                prompt,
                echo: false,
                respond: promise,
            })
            .context("send to ConnectionUI failed")?;

        future.wait()
    }

    pub fn close(&self) {
        self.tx.send(UIRequest::Close).ok();
    }
}
