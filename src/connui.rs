use crate::termwiztermtab;
use anyhow::bail;
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

        std::thread::sleep(Duration::new(2, 0));

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

pub struct ConnectionUI {
    tx: Sender<UIRequest>,
}

impl ConnectionUI {
    pub fn new() -> Self {
        let (tx, rx) = bounded(16);
        promise::spawn::spawn_into_main_thread(termwiztermtab::run(70, 15, move |term| {
            let mut ui = ConnectionUIImpl { term, rx };
            ui.run()
        }));
        Self { tx }
    }

    pub fn title(&self, title: &str) {
        self.output(vec![Change::Title(title.to_string())]);
    }

    pub fn output(&self, changes: Vec<Change>) {
        self.tx
            .send(UIRequest::Output(changes))
            .expect("send to SShUI failed");
    }

    pub fn output_str(&self, s: &str) {
        let s = s.replace("\n", "\r\n");
        self.output(vec![Change::Text(s)]);
    }

    pub fn input(&self, prompt: &str) -> anyhow::Result<String> {
        let mut promise = Promise::new();
        let future = promise.get_future().unwrap();

        self.tx
            .send(UIRequest::Input {
                prompt: prompt.replace("\n", "\r\n"),
                echo: true,
                respond: promise,
            })
            .expect("send to ConnectionUI failed");

        future.wait()
    }

    pub fn password(&self, prompt: &str) -> anyhow::Result<String> {
        let mut promise = Promise::new();
        let future = promise.get_future().unwrap();

        self.tx
            .send(UIRequest::Input {
                prompt: prompt.replace("\n", "\r\n"),
                echo: false,
                respond: promise,
            })
            .expect("send to ConnectionUI failed");

        future.wait()
    }

    pub fn close(&self) {
        self.tx
            .send(UIRequest::Close)
            .expect("send to ConnectionUI failed");
    }
}
