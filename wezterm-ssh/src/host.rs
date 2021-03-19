use crate::session::SessionEvent;
use anyhow::{anyhow, Context};
use smol::channel::{bounded, Sender};
use ssh2::CheckResult;
use std::io::Write;
use std::path::Path;

#[derive(Debug)]
pub struct HostVerificationEvent {
    pub message: String,
    reply: Sender<bool>,
}

impl HostVerificationEvent {
    pub async fn answer(self, trust_host: bool) -> anyhow::Result<()> {
        Ok(self.reply.send(trust_host).await?)
    }
    pub fn try_answer(self, trust_host: bool) -> anyhow::Result<()> {
        Ok(self.reply.try_send(trust_host)?)
    }
}

impl crate::session::SessionInner {
    pub fn host_verification(
        &mut self,
        sess: &ssh2::Session,
        remote_host_name: &str,
        port: u16,
        remote_address: &str,
    ) -> anyhow::Result<()> {
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

            match known_hosts.check_port(&remote_host_name, port, key) {
                CheckResult::Match => {}
                CheckResult::NotFound => {
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
                CheckResult::Mismatch => {
                    anyhow::bail!(
                        "host key mismatch for ssh server {}.\n\
                         Got fingerprint {} instead of expected value from known_hosts\n\
                         file {}.\n\
                         Refusing to connect.",
                        remote_address,
                        fingerprint,
                        file.display()
                    );
                }
                CheckResult::Failure => {
                    anyhow::bail!("failed to check the known hosts");
                }
            }
        }

        Ok(())
    }
}
