use crate::sessionhandler::{PduSender, SessionHandler};
use anyhow::Context;
use async_ossl::AsyncSslStream;
use codec::{DecodedPdu, Pdu};
use mux::{Mux, MuxNotification};
use smol::prelude::*;
use smol::Async;
use std::sync::{Arc, Mutex};
use wezterm_uds::UnixStream;

#[cfg(unix)]
pub trait AsRawDesc: std::os::unix::io::AsRawFd + std::os::fd::AsFd {}
#[cfg(windows)]
pub trait AsRawDesc: std::os::windows::io::AsRawSocket + std::os::windows::io::AsSocket {}

impl AsRawDesc for UnixStream {}
impl AsRawDesc for AsyncSslStream {}

pub async fn process<T>(stream: T) -> anyhow::Result<()>
where
    T: 'static,
    T: std::io::Read,
    T: std::io::Write,
    T: AsRawDesc,
    T: std::fmt::Debug,
    T: async_io::IoSafe,
{
    let stream = smol::Async::new(stream)?;
    process_async(stream).await
}

pub async fn process_async<T>(stream: Async<T>) -> anyhow::Result<()>
where
    T: 'static,
    T: std::io::Read,
    T: std::io::Write,
    T: std::fmt::Debug,
    T: async_io::IoSafe,
{
    log::trace!("process_async called");

    let (reader, writer) = futures::AsyncReadExt::split(stream);

    // Channel for PDUs to be written to the client.
    // Fed by: SessionHandler responses, scheduled pane pushes, and notifications.
    let (write_tx, write_rx) = smol::channel::unbounded::<DecodedPdu>();

    // Channel for mux notifications to be processed by the notification task.
    let (notif_tx, notif_rx) = smol::channel::unbounded::<MuxNotification>();

    let pdu_sender = PduSender::new({
        let write_tx = write_tx.clone();
        move |pdu| {
            write_tx
                .try_send(pdu)
                .map_err(|e| anyhow::anyhow!("{:?}", e))
        }
    });
    let handler = Arc::new(Mutex::new(SessionHandler::new(pdu_sender)));

    {
        let mux = Mux::get();
        mux.subscribe(move |n| notif_tx.try_send(n).is_ok());
    }

    // Writer task: drain write channel, encode + flush to stream.
    // Independent of reading — never blocks the reader.
    let writer_fut = async {
        let mut writer = writer;

        while let Ok(decoded) = write_rx.recv().await {
            match decoded.pdu.encode_async(&mut writer, decoded.serial).await {
                Ok(()) => {}
                Err(err) => {
                    if let Some(err) = err.root_cause().downcast_ref::<std::io::Error>() {
                        if err.kind() == std::io::ErrorKind::BrokenPipe {
                            return Ok(());
                        }
                    }
                    return Err(err).context("encoding PDU to client");
                }
            };
            match writer.flush().await {
                Ok(()) => {}
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::BrokenPipe {
                        return Ok(());
                    }
                    return Err(err).context("flushing PDU to client");
                }
            }
        }

        Ok(())
    };

    // Reader task: decode client requests and dispatch to handler.
    // Never writes to stream — all PDUs go through write_tx.
    let reader_fut = {
        let handler = Arc::clone(&handler);
        async move {
            let mut reader = reader;

            loop {
                match Pdu::decode_async(&mut reader, None).await {
                    Ok(decoded) => {
                        handler.lock().unwrap().process_one(decoded);
                    }
                    Err(err) => {
                        if let Some(err) = err.root_cause().downcast_ref::<std::io::Error>() {
                            if err.kind() == std::io::ErrorKind::UnexpectedEof {
                                return Ok(());
                            }
                        }
                        return Err(err).context("reading Pdu from client");
                    }
                }
            }
        }
    };

    // Notification task: receive mux notifications and dispatch to handler.
    // Runs independently so notifications are processed promptly even when
    // decode_async is blocked waiting for client data.
    let notif_fut = async {
        while let Ok(notif) = notif_rx.recv().await {
            handle_notification(&handler, &write_tx, notif);
        }
        Ok(())
    };

    // Run all three tasks concurrently; first to finish terminates all
    smol::future::race(writer_fut, smol::future::race(reader_fut, notif_fut)).await
}

/// Handle a single mux notification.
fn handle_notification(
    handler: &Arc<Mutex<SessionHandler>>,
    write_tx: &smol::channel::Sender<DecodedPdu>,
    notif: MuxNotification,
) {
    match notif {
        MuxNotification::PaneOutput(pane_id) => {
            handler.lock().unwrap().schedule_pane_push(pane_id);
        }
        MuxNotification::Alert { pane_id, alert } => {
            let mut handler = handler.lock().unwrap();
            {
                let per_pane = handler.per_pane(pane_id);
                let mut per_pane = per_pane.lock().unwrap();
                per_pane.notifications.push(alert);
            }
            handler.schedule_pane_push(pane_id);
        }
        MuxNotification::PaneRemoved(pane_id) => {
            send_pdu(write_tx, Pdu::PaneRemoved(codec::PaneRemoved { pane_id }));
        }
        MuxNotification::AssignClipboard {
            pane_id,
            selection,
            clipboard,
        } => {
            send_pdu(
                write_tx,
                Pdu::SetClipboard(codec::SetClipboard {
                    pane_id,
                    clipboard,
                    selection,
                }),
            );
        }
        MuxNotification::TabAddedToWindow { tab_id, window_id } => {
            send_pdu(
                write_tx,
                Pdu::TabAddedToWindow(codec::TabAddedToWindow { tab_id, window_id }),
            );
        }
        MuxNotification::WindowWorkspaceChanged(window_id) => {
            let workspace = {
                let mux = Mux::get();
                mux.get_window(window_id)
                    .map(|w| w.get_workspace().to_string())
            };
            if let Some(workspace) = workspace {
                send_pdu(
                    write_tx,
                    Pdu::WindowWorkspaceChanged(codec::WindowWorkspaceChanged {
                        window_id,
                        workspace,
                    }),
                );
            }
        }
        MuxNotification::PaneFocused(pane_id) => {
            send_pdu(write_tx, Pdu::PaneFocused(codec::PaneFocused { pane_id }));
        }
        MuxNotification::TabResized(tab_id) => {
            send_pdu(write_tx, Pdu::TabResized(codec::TabResized { tab_id }));
        }
        MuxNotification::TabTitleChanged { tab_id, title } => {
            send_pdu(
                write_tx,
                Pdu::TabTitleChanged(codec::TabTitleChanged { tab_id, title }),
            );
        }
        MuxNotification::WindowTitleChanged { window_id, title } => {
            send_pdu(
                write_tx,
                Pdu::WindowTitleChanged(codec::WindowTitleChanged { window_id, title }),
            );
        }
        MuxNotification::WorkspaceRenamed {
            old_workspace,
            new_workspace,
        } => {
            send_pdu(
                write_tx,
                Pdu::RenameWorkspace(codec::RenameWorkspace {
                    old_workspace,
                    new_workspace,
                }),
            );
        }
        MuxNotification::PaneAdded(_) => {}
        MuxNotification::SaveToDownloads { .. } => {}
        MuxNotification::WindowRemoved(_) => {}
        MuxNotification::WindowCreated(_) => {}
        MuxNotification::WindowInvalidated(_) => {}
        MuxNotification::ActiveWorkspaceChanged(_) => {}
        MuxNotification::Empty => {}
    }
}

/// Helper to send a unilateral PDU (serial 0) to the write channel.
fn send_pdu(write_tx: &smol::channel::Sender<DecodedPdu>, pdu: Pdu) {
    let _ = write_tx.try_send(DecodedPdu { serial: 0, pdu });
}
