use failure::{bail, format_err, Fallible};
use std::collections::HashSet;
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;

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

pub fn ssh_connect(remote_address: &str, username: &str) -> Fallible<ssh2::Session> {
    let mut sess = ssh2::Session::new()?;

    let tcp = TcpStream::connect(&remote_address)?;
    sess.set_tcp_stream(tcp);
    sess.handshake()?;

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

        let remote_host_name = remote_address.split(':').next().ok_or_else(|| {
            format_err!(
                "expected remote_address to have the form 'host:port', but have {}",
                remote_address
            )
        })?;

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
        match known_hosts.check(&remote_host_name, key) {
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
        struct Prompt<'a> {
            username: &'a str,
            remote_address: &'a str,
        }

        let mut prompt = Prompt {
            username,
            remote_address,
        };
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

        if let Err(err) = sess.userauth_keyboard_interactive(&username, &mut prompt) {
            log::error!("while attempting keyboard-interactive auth: {}", err);
        }
    }

    if !sess.authenticated() && methods.contains("password") {
        let pass = password_prompt("", "Password", username, remote_address)
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
