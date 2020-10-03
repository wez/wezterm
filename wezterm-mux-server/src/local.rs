use crate::sessionhandler::{PduSender, SessionHandler};
use crate::{UnixListener, UnixStream};
use anyhow::{anyhow, Context as _};
use codec::{DecodedPdu, Pdu};
use config::{create_user_owned_dirs, UnixDomain};
use mux::{Mux, MuxNotification};
use promise::spawn::{spawn, spawn_into_main_thread};

pub struct LocalListener {
    listener: UnixListener,
}

impl LocalListener {
    pub fn new(listener: UnixListener) -> Self {
        Self { listener }
    }

    pub fn with_domain(unix_dom: &UnixDomain) -> anyhow::Result<Self> {
        let listener = safely_create_sock_path(unix_dom)?;
        Ok(Self::new(listener))
    }

    pub fn run(&mut self) {
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    spawn_into_main_thread(async move { Self::process(stream) });
                }
                Err(err) => {
                    log::error!("accept failed: {}", err);
                    return;
                }
            }
        }
    }

    async fn process(stream: UnixStream) -> anyhow::Result<()> {
        let mut write_stream = smol::Async::new(stream.try_clone()?)?;
        let mut read_stream = smol::Async::new(stream)?;
        let (write_tx, write_rx) = smol::channel::unbounded::<DecodedPdu>();

        // Process the PDU write queue to send to the peer
        spawn(async move {
            while let Ok(decoded) = write_rx.recv().await {
                log::trace!("writing pdu with serial {}", decoded.serial);
                decoded
                    .pdu
                    .encode_async(&mut write_stream, decoded.serial)
                    .await?;
            }
            Ok::<(), anyhow::Error>(())
        });

        let pdu_sender = PduSender::with_smol(write_tx);
        let mut handler = SessionHandler::new(pdu_sender);

        enum Item {
            Notif(MuxNotification),
            Pdu(DecodedPdu),
        }

        let (item_tx, item_rx) = smol::channel::unbounded::<Item>();
        {
            let mux = Mux::get().expect("to be running on gui thread");
            let tx = item_tx.clone();
            mux.subscribe(move |n| tx.try_send(Item::Notif(n)).is_ok());
        }

        spawn(async move {
            while let Ok(decoded) = Pdu::decode_async(&mut read_stream).await {
                item_tx.send(Item::Pdu(decoded)).await?;
            }
            Ok::<(), anyhow::Error>(())
        });

        while let Ok(notif) = item_rx.recv().await {
            match notif {
                Item::Notif(MuxNotification::PaneOutput(pane_id)) => {
                    handler.schedule_pane_push(pane_id);
                }
                Item::Notif(MuxNotification::WindowCreated(_window_id)) => {}
                Item::Pdu(decoded) => {
                    handler.process_one(decoded);
                }
            }
        }

        Ok(())
    }
}

/// Take care when setting up the listener socket;
/// we need to be sure that the directory that we create it in
/// is owned by the user and has appropriate file permissions
/// that prevent other users from manipulating its contents.
fn safely_create_sock_path(unix_dom: &UnixDomain) -> anyhow::Result<UnixListener> {
    let sock_path = &unix_dom.socket_path();
    log::debug!("setting up {}", sock_path.display());

    let sock_dir = sock_path
        .parent()
        .ok_or_else(|| anyhow!("sock_path {} has no parent dir", sock_path.display()))?;

    create_user_owned_dirs(sock_dir)?;

    #[cfg(unix)]
    {
        use config::running_under_wsl;
        use std::os::unix::fs::PermissionsExt;

        if !running_under_wsl() && !unix_dom.skip_permissions_check {
            // Let's be sure that the ownership looks sane
            let meta = sock_dir.symlink_metadata()?;

            let permissions = meta.permissions();
            if (permissions.mode() & 0o22) != 0 {
                anyhow::bail!(
                    "The permissions for {} are insecure and currently \
                     allow other users to write to it (permissions={:?})",
                    sock_dir.display(),
                    permissions
                );
            }
        }
    }

    if sock_path.exists() {
        std::fs::remove_file(sock_path)?;
    }

    UnixListener::bind(sock_path)
        .with_context(|| format!("Failed to bind to {}", sock_path.display()))
}
