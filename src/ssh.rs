use crate::config::Config;
use crate::frontend::guicommon::localtab::LocalTab;
use crate::mux::domain::{alloc_domain_id, Domain, DomainId, DomainState};
use crate::mux::tab::Tab;
use crate::mux::window::WindowId;
use crate::mux::Mux;
use failure::Error;
use failure::{bail, format_err, Fallible};
use portable_pty::cmdbuilder::CommandBuilder;
use portable_pty::{PtySize, PtySystem};
use std::collections::HashSet;
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

fn password_prompt(
    instructions: &str,
    prompt: &str,
    username: &str,
    remote_address: &str,
) -> Option<String> {
    let text = format!(
        "SSH Authentication for {} @ {}\n{}\n{}",
        username, remote_address, instructions, prompt
    );
    tinyfiledialogs::password_box("wezterm", &text)
}

fn input_prompt(
    instructions: &str,
    prompt: &str,
    username: &str,
    remote_address: &str,
) -> Option<String> {
    let text = format!(
        "SSH Authentication for {} @ {}\n{}\n{}",
        username, remote_address, instructions, prompt
    );
    tinyfiledialogs::input_box("wezterm", &text, "")
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

pub fn ssh_connect(remote_address: &str, username: &str) -> Fallible<ssh2::Session> {
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
        .map_err(|e| format_err!("ssh connecting to {}: {}", remote_address, e))?;
    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format_err!("ssh handshake with {}: {}", remote_address, e))?;

    if let Ok(mut known_hosts) = sess.known_hosts() {
        let varname = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let var = std::env::var_os(varname)
            .ok_or_else(|| failure::format_err!("environment variable {} is missing", varname))?;
        let file = Path::new(&var).join(".ssh/known_hosts");
        if file.exists() {
            known_hosts
                .read_file(&file, ssh2::KnownHostFileKind::OpenSSH)
                .map_err(|e| {
                    failure::format_err!("reading known_hosts file {}: {}", file.display(), e)
                })?;
        }

        let (key, key_type) = sess
            .host_key()
            .ok_or_else(|| failure::err_msg("failed to get ssh host key"))?;

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
            .ok_or_else(|| failure::err_msg("failed to get host fingerprint"))?;

        use ssh2::CheckResult;
        match known_hosts.check_port(&remote_host_name, port, key) {
            CheckResult::Match => {}
            CheckResult::NotFound => {
                let allow = tinyfiledialogs::message_box_yes_no(
                    "wezterm",
                    &format!(
                        "SSH host {} is not yet trusted.\n\
                         {:?} Fingerprint: {}.\n\
                         Trust and continue connecting?",
                        remote_address, key_type, fingerprint
                    ),
                    tinyfiledialogs::MessageBoxIcon::Question,
                    tinyfiledialogs::YesNo::No,
                );

                if tinyfiledialogs::YesNo::No == allow {
                    bail!("user declined to trust host");
                }

                known_hosts
                    .add(remote_host_name, key, &remote_address, key_type.into())
                    .map_err(|e| {
                        failure::format_err!("adding known_hosts entry in memory: {}", e)
                    })?;

                known_hosts
                    .write_file(&file, ssh2::KnownHostFileKind::OpenSSH)
                    .map_err(|e| {
                        failure::format_err!("writing known_hosts file {}: {}", file.display(), e)
                    })?;
            }
            CheckResult::Mismatch => {
                tinyfiledialogs::message_box_ok(
                    "wezterm",
                    &format!(
                        "host key mismatch for ssh server {}.\n\
                         Got fingerprint {} instead of expected value from known_hosts\n\
                         file {}.\n\
                         Refusing to connect.",
                        remote_address,
                        fingerprint,
                        file.display()
                    ),
                    tinyfiledialogs::MessageBoxIcon::Error,
                );
                bail!("host mismatch, man in the middle attack?!");
            }
            CheckResult::Failure => {
                tinyfiledialogs::message_box_ok(
                    "wezterm",
                    "Failed to load and check known ssh hosts",
                    tinyfiledialogs::MessageBoxIcon::Error,
                );
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

    if !sess.authenticated() && methods.contains("keyboard-interactive") {
        let mut prompt = Prompt {
            username,
            remote_address: &remote_address,
        };

        if let Err(err) = sess.userauth_keyboard_interactive(&username, &mut prompt) {
            log::error!("while attempting keyboard-interactive auth: {}", err);
        }
    }

    if !sess.authenticated() && methods.contains("password") {
        let pass = password_prompt("", "Password", username, &remote_address)
            .ok_or_else(|| failure::err_msg("password entry was cancelled"))?;
        if let Err(err) = sess.userauth_password(username, &pass) {
            log::error!("while attempting password auth: {}", err);
        }
    }

    if !sess.authenticated() {
        failure::bail!("unable to authenticate session");
    }

    Ok(sess)
}

pub struct RemoteSshDomain {
    pty_system: Box<dyn PtySystem>,
    config: Arc<Config>,
    id: DomainId,
    name: String,
}

impl RemoteSshDomain {
    pub fn with_pty_system(
        name: &str,
        config: &Arc<Config>,
        pty_system: Box<dyn PtySystem>,
    ) -> Self {
        let config = Arc::clone(config);
        let id = alloc_domain_id();
        Self {
            pty_system,
            config,
            id,
            name: name.to_string(),
        }
    }
}

impl Domain for RemoteSshDomain {
    fn spawn(
        &self,
        size: PtySize,
        command: Option<CommandBuilder>,
        window: WindowId,
    ) -> Result<Rc<dyn Tab>, Error> {
        let cmd = match command {
            Some(c) => c,
            None => CommandBuilder::new_default_prog(),
        };
        let pair = self.pty_system.openpty(size)?;
        let child = pair.slave.spawn_command(cmd)?;
        log::info!("spawned: {:?}", child);

        let mut terminal = term::Terminal::new(
            size.rows as usize,
            size.cols as usize,
            size.pixel_width as usize,
            size.pixel_height as usize,
            self.config.scrollback_lines.unwrap_or(3500),
            self.config.hyperlink_rules.clone(),
        );

        let mux = Mux::get().unwrap();

        if let Some(palette) = mux.config().colors.as_ref() {
            *terminal.palette_mut() = palette.clone().into();
        }

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

    fn attach(&self) -> Fallible<()> {
        Ok(())
    }

    fn detach(&self) -> Fallible<()> {
        failure::bail!("detach not implemented");
    }

    fn state(&self) -> DomainState {
        DomainState::Attached
    }
}
