use crate::session::SessionEvent;
use anyhow::Context;
use smol::channel::{bounded, Sender};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct AuthenticationPrompt {
    pub prompt: String,
    pub echo: bool,
}

#[derive(Debug)]
pub struct AuthenticationEvent {
    pub username: String,
    pub instructions: String,
    pub prompts: Vec<AuthenticationPrompt>,
    pub(crate) reply: Sender<Vec<String>>,
}

impl AuthenticationEvent {
    pub async fn answer(self, answers: Vec<String>) -> anyhow::Result<()> {
        Ok(self.reply.send(answers).await?)
    }

    pub fn try_answer(self, answers: Vec<String>) -> anyhow::Result<()> {
        Ok(self.reply.try_send(answers)?)
    }
}

impl crate::session::SessionInner {
    fn agent_auth(&mut self, sess: &ssh2::Session, user: &str) -> anyhow::Result<bool> {
        if let Some(only) = self.config.get("identitiesonly") {
            if only == "yes" {
                return Ok(false);
            }
        }

        let mut agent = sess.agent()?;
        agent.connect()?;
        agent.list_identities()?;
        let identities = agent.identities()?;
        for identity in identities {
            if agent.userauth(user, &identity).is_ok() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn pubkey_auth(
        &mut self,
        sess: &ssh2::Session,
        user: &str,
        host: &str,
    ) -> anyhow::Result<bool> {
        if let Some(files) = self.config.get("identityfile") {
            for file in files.split_whitespace() {
                let pubkey: PathBuf = format!("{}.pub", file).into();
                let file = Path::new(file);

                if !file.exists() {
                    continue;
                }

                let pubkey = if pubkey.exists() && false {
                    Some(pubkey.as_ref())
                } else {
                    None
                };

                match sess.userauth_pubkey_file(user, pubkey, &file, None) {
                    Ok(_) => return Ok(true),
                    Err(err) => {
                        if err.code() == ssh2::ErrorCode::Session(-16)
                            || err.code() == ssh2::ErrorCode::Session(-18)
                        {
                            // Need a passphrase to decrypt the key

                            let (reply, answers) = bounded(1);
                            self.tx_event
                                .try_send(SessionEvent::Authenticate(AuthenticationEvent {
                                    username: "".to_string(),
                                    instructions: "".to_string(),
                                    prompts: vec![AuthenticationPrompt {
                                        prompt: format!(
                                            "Passphrase to decrypt {} for {}@{}: ",
                                            file.display(),
                                            user,
                                            host
                                        ),
                                        echo: false,
                                    }],
                                    reply,
                                }))
                                .context("sending Authenticate request to user")?;

                            let answers = smol::block_on(answers.recv())
                                .context("waiting for authentication answers from user")?;

                            if answers.is_empty() {
                                anyhow::bail!("user cancelled authentication");
                            }

                            let passphrase = &answers[0];

                            match sess.userauth_pubkey_file(user, pubkey, &file, Some(passphrase)) {
                                Ok(_) => return Ok(true),
                                Err(err) => {
                                    log::warn!("pubkey auth: {:#}", err);
                                }
                            }
                        } else {
                            log::warn!("pubkey auth: {:#}", err);
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    pub fn authenticate(
        &mut self,
        sess: &ssh2::Session,
        user: &str,
        host: &str,
    ) -> anyhow::Result<()> {
        loop {
            if sess.authenticated() {
                return Ok(());
            }

            // Re-query the auth methods on each loop as a successful method
            // may unlock a new method on a subsequent iteration (eg: password
            // auth may then unlock 2fac)
            let methods: HashSet<&str> = sess.auth_methods(&user)?.split(',').collect();
            log::trace!("ssh auth methods: {:?}", methods);

            if !sess.authenticated() && methods.contains("publickey") {
                match self.agent_auth(sess, user) {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(err) => {
                        log::warn!("while attempting agent auth: {}", err)
                    }
                }

                match self.pubkey_auth(sess, user, host) {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(err) => {
                        log::warn!("while attempting auth: {}", err)
                    }
                }
            }

            if !sess.authenticated() && methods.contains("password") {
                let (reply, answers) = bounded(1);
                self.tx_event
                    .try_send(SessionEvent::Authenticate(AuthenticationEvent {
                        username: user.to_string(),
                        instructions: "".to_string(),
                        prompts: vec![AuthenticationPrompt {
                            prompt: format!("Password for {}@{}: ", user, host),
                            echo: false,
                        }],
                        reply,
                    }))
                    .context("sending Authenticate request to user")?;

                let answers = smol::block_on(answers.recv())
                    .context("waiting for authentication answers from user")?;

                if answers.is_empty() {
                    anyhow::bail!("user cancelled authentication");
                }

                if let Err(err) = sess.userauth_password(user, &answers[0]) {
                    log::error!("while attempting password auth: {}", err);
                }
            }

            if !sess.authenticated() && methods.contains("keyboard-interactive") {
                struct Helper<'a> {
                    tx_event: &'a Sender<SessionEvent>,
                }

                impl<'a> ssh2::KeyboardInteractivePrompt for Helper<'a> {
                    fn prompt<'b>(
                        &mut self,
                        username: &str,
                        instructions: &str,
                        prompts: &[ssh2::Prompt<'b>],
                    ) -> Vec<String> {
                        let (reply, answers) = bounded(1);
                        if let Err(err) = self.tx_event.try_send(SessionEvent::Authenticate(
                            AuthenticationEvent {
                                username: username.to_string(),
                                instructions: instructions.to_string(),
                                prompts: prompts
                                    .iter()
                                    .map(|p| AuthenticationPrompt {
                                        prompt: p.text.to_string(),
                                        echo: p.echo,
                                    })
                                    .collect(),
                                reply,
                            },
                        )) {
                            log::error!("sending Authenticate request to user: {:#}", err);
                            return vec![];
                        }

                        match smol::block_on(answers.recv()) {
                            Err(err) => {
                                log::error!(
                                    "waiting for authentication answers from user: {:#}",
                                    err
                                );
                                return vec![];
                            }
                            Ok(answers) => answers,
                        }
                    }
                }

                let mut helper = Helper {
                    tx_event: &self.tx_event,
                };

                if let Err(err) = sess.userauth_keyboard_interactive(user, &mut helper) {
                    log::error!("while attempting keyboard-interactive auth: {}", err);
                }
            }
        }
    }
}
