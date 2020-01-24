use crate::localtab::LocalTab;
use crate::mux::domain::{alloc_domain_id, Domain, DomainId, DomainState};
use crate::mux::tab::Tab;
use crate::mux::window::WindowId;
use crate::mux::Mux;
use crate::termwiztermtab;
use anyhow::{anyhow, bail, Context, Error};
use async_trait::async_trait;
use portable_pty::cmdbuilder::CommandBuilder;
use portable_pty::{PtySize, PtySystem};
use promise::{Future, Promise};
use std::collections::HashSet;
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use std::rc::Rc;
use termwiz::cell::{unicode_column_width, AttributeChange, Intensity};
use termwiz::lineedit::*;
use termwiz::surface::Change;
use termwiz::terminal::*;

fn password_prompt(
    instructions: &str,
    prompt: &str,
    username: &str,
    remote_address: &str,
) -> Option<String> {
    let title = "🔐 wezterm: SSH authentication".to_string();
    let text = format!(
        "🔐 SSH Authentication for {} @ {}\r\n{}\r\n",
        username, remote_address, instructions
    );
    let prompt = prompt.to_string();

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
        fn highlight_line(
            &self,
            line: &str,
            cursor_position: usize,
        ) -> (Vec<OutputElement>, usize) {
            let placeholder = "🔑";
            let grapheme_count = unicode_column_width(line);
            let mut output = vec![];
            for _ in 0..grapheme_count {
                output.push(OutputElement::Text(placeholder.to_string()));
            }
            (output, unicode_column_width(placeholder) * cursor_position)
        }
    }
    match promise::spawn::block_on(termwiztermtab::run(60, 10, move |mut term| {
        term.render(&[
            // Change::Attribute(AttributeChange::Intensity(Intensity::Bold)),
            Change::Title(title.to_string()),
            Change::Text(text.to_string()),
            Change::Attribute(AttributeChange::Intensity(Intensity::Normal)),
        ])?;

        let mut editor = LineEditor::new(term);
        editor.set_prompt(&format!("{}: ", prompt));

        let mut host = PasswordPromptHost::default();
        if let Some(line) = editor.read_line(&mut host)? {
            Ok(line)
        } else {
            bail!("prompt cancelled");
        }
    })) {
        Ok(p) => Some(p),
        Err(p) => {
            log::error!("failed to prompt for pw: {}", p);
            None
        }
    }
}

fn input_prompt(
    instructions: &str,
    prompt: &str,
    username: &str,
    remote_address: &str,
) -> Option<String> {
    let title = "🔐 wezterm: SSH authentication".to_string();
    let text = format!(
        "SSH Authentication for {} @ {}\r\n{}\r\n{}\r\n",
        username, remote_address, instructions, prompt
    );
    match promise::spawn::block_on(termwiztermtab::run(60, 10, move |mut term| {
        term.render(&[
            Change::Title(title.to_string()),
            Change::Text(text.to_string()),
            Change::Attribute(AttributeChange::Intensity(Intensity::Normal)),
        ])?;

        let mut editor = LineEditor::new(term);

        let mut host = NopLineEditorHost::default();
        if let Some(line) = editor.read_line(&mut host)? {
            Ok(line)
        } else {
            bail!("prompt cancelled");
        }
    })) {
        Ok(p) => Some(p),
        Err(p) => {
            log::error!("failed to prompt for pw: {}", p);
            None
        }
    }
}

struct Prompt<'a> {
    username: &'a str,
    remote_address: &'a str,
}

impl<'a> ssh2::KeyboardInteractivePrompt for Prompt<'a> {
    fn prompt<'b>(
        &mut self,
        _username: &str,
        instructions: &str,
        prompts: &[ssh2::Prompt<'b>],
    ) -> Vec<String> {
        prompts
            .iter()
            .map(|p| {
                let func = if p.echo {
                    input_prompt
                } else {
                    password_prompt
                };

                func(instructions, &p.text, &self.username, &self.remote_address)
                    .unwrap_or_else(String::new)
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

pub fn ssh_connect(remote_address: &str, username: &str) -> anyhow::Result<ssh2::Session> {
    let mut sess = ssh2::Session::new()?;

    let (remote_address, remote_host_name, port) = {
        let parts: Vec<&str> = remote_address.split(':').collect();

        if parts.len() == 2 {
            (remote_address.to_string(), parts[0], parts[1].parse()?)
        } else {
            (format!("{}:22", remote_address), remote_address, 22)
        }
    };

    let tcp = TcpStream::connect(&remote_address)
        .with_context(|| format!("ssh connecting to {}", remote_address))?;
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
                let message = format!(
                    "SSH host {} is not yet trusted.\r\n\
                     {:?} Fingerprint: {}.\r\n\
                     Trust and continue connecting?\r\n",
                    remote_address, key_type, fingerprint
                );

                let allow =
                    promise::spawn::block_on(termwiztermtab::run(80, 10, move |mut term| {
                        let title = "🔐 wezterm: SSH authentication".to_string();
                        term.render(&[Change::Title(title), Change::Text(message.to_string())])?;

                        let mut editor = LineEditor::new(term);
                        editor.set_prompt("Enter [Y/n]> ");

                        let mut host = NopLineEditorHost::default();
                        loop {
                            let line = match editor.read_line(&mut host) {
                                Ok(Some(line)) => line,
                                Ok(None) | Err(_) => return Ok(false),
                            };
                            match line.as_ref() {
                                "y" | "Y" | "yes" | "YES" => return Ok(true),
                                "n" | "N" | "no" | "NO" => return Ok(false),
                                _ => continue,
                            }
                        }
                    }))?;

                if !allow {
                    bail!("user declined to trust host");
                }

                known_hosts
                    .add(remote_host_name, key, &remote_address, key_type.into())
                    .context("adding known_hosts entry in memory")?;

                known_hosts
                    .write_file(&file, ssh2::KnownHostFileKind::OpenSSH)
                    .with_context(|| format!("writing known_hosts file {}", file.display()))?;
            }
            CheckResult::Mismatch => {
                termwiztermtab::message_box_ok(&format!(
                    "🛑 host key mismatch for ssh server {}.\n\
                     Got fingerprint {} instead of expected value from known_hosts\n\
                     file {}.\n\
                     Refusing to connect.",
                    remote_address,
                    fingerprint,
                    file.display()
                ));
                bail!("host mismatch, man in the middle attack?!");
            }
            CheckResult::Failure => {
                termwiztermtab::message_box_ok("🛑 Failed to load and check known ssh hosts");
                bail!("failed to check the known hosts");
            }
        }
    }

    let methods: HashSet<&str> = sess.auth_methods(&username)?.split(',').collect();

    if !sess.authenticated() && methods.contains("publickey") {
        if let Err(err) = sess.userauth_agent(&username) {
            log::info!("while attempting agent auth: {}", err);
        }
    }

    if !sess.authenticated() && methods.contains("password") {
        let pass = password_prompt("", "Password", username, &remote_address)
            .ok_or_else(|| anyhow!("password entry was cancelled"))?;
        if let Err(err) = sess.userauth_password(username, &pass) {
            log::error!("while attempting password auth: {}", err);
        }
    }

    if !sess.authenticated() && methods.contains("keyboard-interactive") {
        let mut prompt = Prompt {
            username,
            remote_address: &remote_address,
        };

        if let Err(err) = sess.userauth_keyboard_interactive(&username, &mut prompt) {
            log::error!("while attempting keyboard-interactive auth: {}", err);
        }
    }

    if !sess.authenticated() {
        bail!("unable to authenticate session");
    }

    Ok(sess)
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
