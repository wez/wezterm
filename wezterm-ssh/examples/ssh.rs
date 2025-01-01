//! This is a really basic ssh client example intended
//! to test the guts of the ssh handling, rather than
//! to be a full fledged replacement for ssh.
use anyhow::Context;
use clap::Parser;
use portable_pty::{Child, MasterPty, PtySize};
use std::io::{Read, Write};
use termwiz::cell::unicode_column_width;
use termwiz::lineedit::*;
use wezterm_ssh::{Config, Session, SessionEvent};

#[derive(Default)]
struct PasswordPromptHost {
    history: BasicHistory,
    echo: bool,
}
impl LineEditorHost for PasswordPromptHost {
    fn history(&mut self) -> &mut dyn History {
        &mut self.history
    }

    fn highlight_line(&self, line: &str, cursor_position: usize) -> (Vec<OutputElement>, usize) {
        if self.echo {
            (vec![OutputElement::Text(line.to_string())], cursor_position)
        } else {
            // Rewrite the input so that we can obscure the password
            // characters when output to the terminal widget
            let placeholder = "🔑";
            let grapheme_count = unicode_column_width(line, None);
            let mut output = vec![];
            for _ in 0..grapheme_count {
                output.push(OutputElement::Text(placeholder.to_string()));
            }
            (
                output,
                unicode_column_width(placeholder, None) * cursor_position,
            )
        }
    }
}

#[derive(Debug, Parser, Default, Clone)]
struct Opt {
    #[clap(long = "user", short = 'l')]
    pub user: Option<String>,
    pub destination: String,
    pub cmd: Vec<String>,
}

fn main() {
    env_logger::init();
    let opts = Opt::parse();

    let mut config = Config::new();
    config.add_default_config_files();

    let mut config = config.for_host(&opts.destination);
    if let Some(user) = opts.user.as_ref() {
        config.insert("user".to_string(), user.to_string());
    }

    let res = smol::block_on(async move {
        let (session, events) = Session::connect(config.clone())?;

        while let Ok(event) = events.recv().await {
            match event {
                SessionEvent::Banner(banner) => {
                    if let Some(banner) = banner {
                        log::trace!("{}", banner);
                    }
                }
                SessionEvent::HostVerify(verify) => {
                    eprintln!("{}", verify.message);
                    let mut terminal = line_editor_terminal()?;
                    let mut editor = LineEditor::new(&mut terminal);
                    let mut host = PasswordPromptHost::default();
                    host.echo = true;
                    editor.set_prompt("Enter [y/n]> ");
                    let ok = if let Some(line) = editor.read_line(&mut host)? {
                        match line.as_ref() {
                            "y" | "Y" | "yes" | "YES" => true,
                            "n" | "N" | "no" | "NO" | _ => false,
                        }
                    } else {
                        false
                    };
                    verify.answer(ok).await.context("send verify response")?;
                }
                SessionEvent::Authenticate(auth) => {
                    if !auth.username.is_empty() {
                        eprintln!("Authentication for {}", auth.username);
                    }
                    if !auth.instructions.is_empty() {
                        eprintln!("{}", auth.instructions);
                    }
                    let mut terminal = line_editor_terminal()?;
                    let mut editor = LineEditor::new(&mut terminal);
                    let mut host = PasswordPromptHost::default();
                    let mut answers = vec![];
                    for prompt in &auth.prompts {
                        let mut prompt_lines = prompt.prompt.split('\n').collect::<Vec<_>>();
                        editor.set_prompt(prompt_lines.pop().unwrap());
                        host.echo = prompt.echo;
                        for line in &prompt_lines {
                            eprintln!("{}", line);
                        }
                        if let Some(line) = editor.read_line(&mut host)? {
                            answers.push(line);
                        } else {
                            anyhow::bail!("Authentication was cancelled");
                        }
                    }
                    auth.answer(answers).await?;
                }
                SessionEvent::HostVerificationFailed(failed) => {
                    anyhow::bail!("{}", failed);
                }
                SessionEvent::Error(err) => {
                    anyhow::bail!("{}", err);
                }
                SessionEvent::Authenticated => break,
            }
        }

        let command_line = shell_words::join(&opts.cmd);
        let command_line = if command_line.is_empty() {
            None
        } else {
            Some(command_line.as_str())
        };

        let (pty, mut child) = session
            .request_pty("xterm-256color", PtySize::default(), command_line, None)
            .await?;

        let mut reader = pty.try_clone_reader()?;
        let stdout = std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let mut stdout = std::io::stdout();
            while let Ok(len) = reader.read(&mut buf) {
                if len == 0 {
                    break;
                }
                if stdout.write_all(&buf[0..len]).is_err() {
                    break;
                }
            }
        });

        // Need to separate out the writer so that we can drop
        // the pty which would otherwise keep the ssh session
        // thread alive
        let mut writer = pty.take_writer()?;
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let mut stdin = std::io::stdin();
            while let Ok(len) = stdin.read(&mut buf) {
                if len == 0 {
                    break;
                }
                if writer.write_all(&buf[0..len]).is_err() {
                    break;
                }
            }
        });

        let status = child.wait()?;
        let _ = stdout.join();
        if !status.success() {
            std::process::exit(1);
        }
        Ok(())
    });

    if let Err(err) = res {
        eprintln!("{:#}", err);
        std::process::exit(1);
    }
}
