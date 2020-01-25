use crate::localtab::LocalTab;
use crate::mux::domain::{alloc_domain_id, Domain, DomainId, DomainState};
use crate::mux::tab::Tab;
use crate::mux::window::WindowId;
use crate::mux::Mux;
use crate::termwiztermtab;
use anyhow::{anyhow, bail, Context, Error};
use async_trait::async_trait;
use crossbeam::channel::{bounded, Receiver, Sender};
use portable_pty::cmdbuilder::CommandBuilder;
use portable_pty::{PtySize, PtySystem};
use promise::{Future, Promise};
use std::collections::HashSet;
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use std::rc::Rc;
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

enum UIRequest {
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

struct SshUIImpl {
    term: termwiztermtab::TermWizTerminal,
    rx: Receiver<UIRequest>,
}

impl SshUIImpl {
    fn run(&mut self) -> anyhow::Result<()> {
        let title = "🔐 wezterm: SSH authentication".to_string();
        self.term.render(&[Change::Title(title)])?;

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

struct SshUI {
    tx: Sender<UIRequest>,
}

impl SshUI {
    fn new() -> Self {
        let (tx, rx) = bounded(16);
        promise::spawn::spawn_into_main_thread(termwiztermtab::run(70, 15, move |term| {
            let mut ui = SshUIImpl { term, rx };
            ui.run()
        }));
        Self { tx }
    }

    fn output(&self, changes: Vec<Change>) {
        self.tx
            .send(UIRequest::Output(changes))
            .expect("send to SShUI failed");
    }

    fn output_str(&self, s: &str) {
        let s = s.replace("\n", "\r\n");
        self.output(vec![Change::Text(s)]);
    }

    fn input(&self, prompt: &str) -> anyhow::Result<String> {
        let mut promise = Promise::new();
        let future = promise.get_future().unwrap();

        self.tx
            .send(UIRequest::Input {
                prompt: prompt.replace("\n", "\r\n"),
                echo: true,
                respond: promise,
            })
            .expect("send to SshUI failed");

        future.wait()
    }

    fn password(&self, prompt: &str) -> anyhow::Result<String> {
        let mut promise = Promise::new();
        let future = promise.get_future().unwrap();

        self.tx
            .send(UIRequest::Input {
                prompt: prompt.replace("\n", "\r\n"),
                echo: false,
                respond: promise,
            })
            .expect("send to SshUI failed");

        future.wait()
    }

    fn close(&self) {
        self.tx
            .send(UIRequest::Close)
            .expect("send to SshUI failed");
    }
}

impl ssh2::KeyboardInteractivePrompt for SshUI {
    fn prompt<'b>(
        &mut self,
        _username: &str,
        instructions: &str,
        prompts: &[ssh2::Prompt<'b>],
    ) -> Vec<String> {
        prompts
            .iter()
            .map(|p| {
                self.output_str(&format!("{}\n", instructions));
                if p.echo {
                    self.input(&p.text)
                } else {
                    self.password(&p.text)
                }
                .unwrap_or_else(|_| String::new())
            })
            .collect()
    }
}

pub fn async_ssh_connect(remote_address: &str, username: &str) -> Future<ssh2::Session> {
    let mut promise = Promise::new();
    let future = promise.get_future().unwrap();
    let remote_address = remote_address.to_owned();
    let username = username.to_owned();
    std::thread::spawn(move || promise.result(ssh_connect(&remote_address, &username)));
    future
}

fn ssh_connect_with_ui(
    remote_address: &str,
    username: &str,
    ui: &mut SshUI,
) -> anyhow::Result<ssh2::Session> {
    let mut sess = ssh2::Session::new()?;

    let (remote_address, remote_host_name, port) = {
        let parts: Vec<&str> = remote_address.split(':').collect();

        if parts.len() == 2 {
            (remote_address.to_string(), parts[0], parts[1].parse()?)
        } else {
            (format!("{}:22", remote_address), remote_address, 22)
        }
    };

    ui.output_str(&format!("Connecting to {}\n", remote_address));

    let tcp = TcpStream::connect(&remote_address)
        .with_context(|| format!("ssh connecting to {}", remote_address))?;
    ui.output_str("Connected OK!\n");
    tcp.set_nodelay(true)?;
    sess.set_tcp_stream(tcp);
    sess.handshake()
        .with_context(|| format!("ssh handshake with {}", remote_address))?;

    if let Ok(mut known_hosts) = sess.known_hosts() {
        let varname = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let var = std::env::var_os(varname)
            .ok_or_else(|| anyhow!("environment variable {} is missing", varname))?;
        let file = Path::new(&var).join(".ssh/known_hosts");
        if file.exists() {
            known_hosts
                .read_file(&file, ssh2::KnownHostFileKind::OpenSSH)
                .with_context(|| format!("reading known_hosts file {}", file.display()))?;
        }

        let (key, key_type) = sess
            .host_key()
            .ok_or_else(|| anyhow!("failed to get ssh host key"))?;

        let fingerprint = sess
            .host_key_hash(ssh2::HashType::Sha256)
            .map(|fingerprint| {
                format!(
                    "SHA256:{}",
                    base64::encode_config(
                        fingerprint,
                        base64::Config::new(base64::CharacterSet::Standard, false)
                    )
                )
            })
            .or_else(|| {
                // Querying for the Sha256 can fail if for example we were linked
                // against libssh < 1.9, so let's fall back to Sha1 in that case.
                sess.host_key_hash(ssh2::HashType::Sha1).map(|fingerprint| {
                    let mut res = vec![];
                    write!(&mut res, "SHA1").ok();
                    for b in fingerprint {
                        write!(&mut res, ":{:02x}", *b).ok();
                    }
                    String::from_utf8(res).unwrap()
                })
            })
            .ok_or_else(|| anyhow!("failed to get host fingerprint"))?;

        use ssh2::CheckResult;
        match known_hosts.check_port(&remote_host_name, port, key) {
            CheckResult::Match => {}
            CheckResult::NotFound => {
                ui.output_str(&format!(
                    "SSH host {} is not yet trusted.\n\
                     {:?} Fingerprint: {}.\n\
                     Trust and continue connecting?\n",
                    remote_address, key_type, fingerprint
                ));

                loop {
                    let line = ui.input("Enter [Y/n]> ")?;

                    match line.as_ref() {
                        "y" | "Y" | "yes" | "YES" => break,
                        "n" | "N" | "no" | "NO" => bail!("user declined to trust host"),
                        _ => continue,
                    }
                }

                known_hosts
                    .add(remote_host_name, key, &remote_address, key_type.into())
                    .context("adding known_hosts entry in memory")?;

                known_hosts
                    .write_file(&file, ssh2::KnownHostFileKind::OpenSSH)
                    .with_context(|| format!("writing known_hosts file {}", file.display()))?;
            }
            CheckResult::Mismatch => {
                ui.output_str(&format!(
                    "🛑 host key mismatch for ssh server {}.\n\
                     Got fingerprint {} instead of expected value from known_hosts\n\
                     file {}.\n\
                     Refusing to connect.\n",
                    remote_address,
                    fingerprint,
                    file.display()
                ));
                bail!("host mismatch, man in the middle attack?!");
            }
            CheckResult::Failure => {
                ui.output_str("🛑 Failed to load and check known ssh hosts\n");
                bail!("failed to check the known hosts");
            }
        }
    }

    for _ in 0..3 {
        if sess.authenticated() {
            break;
        }

        // Re-query the auth methods on each loop as a successful method
        // may unlock a new method on a subsequent iteration (eg: password
        // auth may then unlock 2fac)
        let methods: HashSet<&str> = sess.auth_methods(&username)?.split(',').collect();
        log::trace!("ssh auth methods: {:?}", methods);

        if !sess.authenticated() && methods.contains("publickey") {
            if let Err(err) = sess.userauth_agent(&username) {
                log::info!("while attempting agent auth: {}", err);
            } else if sess.authenticated() {
                ui.output_str("publickey auth successful!\n");
            }
        }

        if !sess.authenticated() && methods.contains("password") {
            ui.output_str(&format!(
                "Password authentication for {}@{}\n",
                username, remote_address
            ));
            let pass = ui.password("Password: ")?;
            if let Err(err) = sess.userauth_password(username, &pass) {
                log::error!("while attempting password auth: {}", err);
            }
        }

        if !sess.authenticated() && methods.contains("keyboard-interactive") {
            if let Err(err) = sess.userauth_keyboard_interactive(&username, ui) {
                log::error!("while attempting keyboard-interactive auth: {}", err);
            }
        }
    }

    if !sess.authenticated() {
        bail!("unable to authenticate session");
    }

    Ok(sess)
}

pub fn ssh_connect(remote_address: &str, username: &str) -> anyhow::Result<ssh2::Session> {
    let mut ui = SshUI::new();
    let res = ssh_connect_with_ui(remote_address, username, &mut ui);
    match res {
        Ok(sess) => {
            ui.close();
            Ok(sess)
        }
        Err(err) => {
            ui.output_str(&format!("\nFailed: {}", err));
            Err(err)
        }
    }
}

pub struct RemoteSshDomain {
    pty_system: Box<dyn PtySystem>,
    id: DomainId,
    name: String,
}

impl RemoteSshDomain {
    pub fn with_pty_system(name: &str, pty_system: Box<dyn PtySystem>) -> Self {
        let id = alloc_domain_id();
        Self {
            pty_system,
            id,
            name: name.to_string(),
        }
    }
}

#[async_trait(?Send)]
impl Domain for RemoteSshDomain {
    async fn spawn(
        &self,
        size: PtySize,
        command: Option<CommandBuilder>,
        _command_dir: Option<String>,
        window: WindowId,
    ) -> Result<Rc<dyn Tab>, Error> {
        let cmd = match command {
            Some(c) => c,
            None => CommandBuilder::new_default_prog(),
        };
        let pair = self.pty_system.openpty(size)?;
        let child = pair.slave.spawn_command(cmd)?;
        log::info!("spawned: {:?}", child);

        let terminal = term::Terminal::new(
            size.rows as usize,
            size.cols as usize,
            size.pixel_width as usize,
            size.pixel_height as usize,
            std::sync::Arc::new(crate::config::TermConfig {}),
        );

        let mux = Mux::get().unwrap();
        let tab: Rc<dyn Tab> = Rc::new(LocalTab::new(terminal, child, pair.master, self.id));

        mux.add_tab(&tab)?;
        mux.add_tab_to_window(&tab, window)?;

        Ok(tab)
    }

    fn domain_id(&self) -> DomainId {
        self.id
    }

    fn domain_name(&self) -> &str {
        &self.name
    }

    async fn attach(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn detach(&self) -> anyhow::Result<()> {
        bail!("detach not implemented");
    }

    fn state(&self) -> DomainState {
        DomainState::Attached
    }
}
