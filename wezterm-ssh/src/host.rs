use crate::session::SessionEvent;
use anyhow::Context;
use smol::channel::{bounded, Sender};

#[derive(Debug, thiserror::Error)]
#[error("host key mismatch for ssh server {remote_address}. Got fingerprint {key} instead of the expected value from your known hosts file {file:?}.")]
pub struct HostVerificationFailed {
    pub remote_address: String,
    pub key: String,
    pub file: Option<std::path::PathBuf>,
}

#[derive(Debug)]
pub struct HostVerificationEvent {
    pub message: String,
    pub(crate) reply: Sender<bool>,
}

impl HostVerificationEvent {
    pub async fn answer(self, trust_host: bool) -> anyhow::Result<()> {
        Ok(self.reply.send(trust_host).await?)
    }
    pub fn try_answer(self, trust_host: bool) -> anyhow::Result<()> {
        Ok(self.reply.try_send(trust_host)?)
    }
}

impl crate::sessioninner::SessionInner {
    #[cfg(feature = "libssh-rs")]
    pub fn host_verification_libssh(
        &mut self,
        sess: &libssh_rs::Session,
        hostname: &str,
        port: u16,
    ) -> anyhow::Result<()> {
        let key = sess
            .get_server_public_key()?
            .get_public_key_hash_hexa(libssh_rs::PublicKeyHashType::Sha256)?;

        match sess.is_known_server()? {
            libssh_rs::KnownHosts::Ok => Ok(()),
            libssh_rs::KnownHosts::NotFound | libssh_rs::KnownHosts::Unknown => {
                let (reply, confirm) = bounded(1);
                self.tx_event
                    .try_send(SessionEvent::HostVerify(HostVerificationEvent {
                        message: format!(
                            "SSH host {}:{} is not yet trusted.\n\
                                    Fingerprint: {}.\n\
                                    Trust and continue connecting?",
                            hostname, port, key
                        ),
                        reply,
                    }))
                    .context("sending HostVerify request to user")?;

                let trusted = smol::block_on(confirm.recv())
                    .context("waiting for host verification confirmation from user")?;

                if !trusted {
                    anyhow::bail!("user declined to trust host");
                }

                Ok(sess.update_known_hosts_file()?)
            }
            libssh_rs::KnownHosts::Changed => {
                let mut file = None;
                if let Some(kh) = self.config.get("userknownhostsfile") {
                    for candidate in kh.split_whitespace() {
                        file.replace(candidate.into());
                        break;
                    }
                }

                let failed = HostVerificationFailed {
                    remote_address: format!("{hostname}:{port}"),
                    key,
                    file,
                };
                self.tx_event
                    .try_send(SessionEvent::HostVerificationFailed(failed))
                    .context("sending HostVerificationFailed event to user")?;
                anyhow::bail!("Host key verification failed");
            }
            libssh_rs::KnownHosts::Other => {
                anyhow::bail!(
                    "The host key for this server was not found, but another\n\
            type of key exists. An attacker might change the default\n\
            server key to confuse your client into thinking the key\n\
            does not exist"
                );
            }
        }
    }

    #[cfg(feature = "ssh2")]
    pub fn host_verification(
        &mut self,
        sess: &ssh2::Session,
        remote_host_name: &str,
        port: u16,
        remote_address: &str,
    ) -> anyhow::Result<()> {
        use anyhow::anyhow;
        use std::io::Write;
        use std::path::Path;

        let mut known_hosts = sess.known_hosts().context("preparing known hosts")?;

        let known_hosts_files = self
            .config
            .get("userknownhostsfile")
            .unwrap()
            .split_whitespace()
            .map(|s| s.to_string());

        for file in known_hosts_files {
            let file = Path::new(&file);

            if !file.exists() {
                continue;
            }

            known_hosts
                .read_file(&file, ssh2::KnownHostFileKind::OpenSSH)
                .with_context(|| format!("reading known_hosts file {}", file.display()))?;

            let (key, key_type) = sess
                .host_key()
                .ok_or_else(|| anyhow!("failed to get ssh host key"))?;

            let fingerprint = sess
                .host_key_hash(ssh2::HashType::Sha256)
                .map(|fingerprint| {
                    use base64::Engine;
                    let engine = base64::engine::general_purpose::GeneralPurpose::new(
                        &base64::alphabet::STANDARD,
                        base64::engine::general_purpose::NO_PAD,
                    );
                    format!("SHA256:{}", engine.encode(fingerprint))
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

            match known_hosts.check_port(&remote_host_name, port, key) {
                ssh2::CheckResult::Match => {}
                ssh2::CheckResult::NotFound => {
                    let (reply, confirm) = bounded(1);
                    self.tx_event
                        .try_send(SessionEvent::HostVerify(HostVerificationEvent {
                            message: format!(
                                "SSH host {} is not yet trusted.\n\
                                {:?} Fingerprint: {}.\n\
                                Trust and continue connecting?",
                                remote_address, key_type, fingerprint
                            ),
                            reply,
                        }))
                        .context("sending HostVerify request to user")?;

                    let trusted = smol::block_on(confirm.recv())
                        .context("waiting for host verification confirmation from user")?;

                    if !trusted {
                        anyhow::bail!("user declined to trust host");
                    }

                    let host_and_port = if port != 22 {
                        format!("[{}]:{}", remote_host_name, port)
                    } else {
                        remote_host_name.to_string()
                    };

                    known_hosts
                        .add(&host_and_port, key, &remote_address, key_type.into())
                        .context("adding known_hosts entry in memory")?;

                    known_hosts
                        .write_file(&file, ssh2::KnownHostFileKind::OpenSSH)
                        .with_context(|| format!("writing known_hosts file {}", file.display()))?;
                }
                ssh2::CheckResult::Mismatch => {
                    let failed = HostVerificationFailed {
                        remote_address: remote_address.to_string(),
                        key: fingerprint,
                        file: Some(file.to_path_buf()),
                    };
                    self.tx_event
                        .try_send(SessionEvent::HostVerificationFailed(failed))
                        .context("sending HostVerificationFailed event to user")?;
                    anyhow::bail!("Host key verification failed");
                }
                ssh2::CheckResult::Failure => {
                    anyhow::bail!("failed to check the known hosts");
                }
            }
        }

        Ok(())
    }
}
