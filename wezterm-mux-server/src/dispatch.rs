use crate::sessionhandler::{PduSender, SessionHandler};
use crate::UnixStream;
use codec::{DecodedPdu, Pdu};
use mux::{Mux, MuxNotification};
use promise::spawn::spawn;
use smol::io::{AsyncRead, AsyncWrite};
use std::marker::Unpin;

#[cfg(unix)]
pub trait AsRawDesc: std::os::unix::io::AsRawFd {}
#[cfg(windows)]
pub trait AsRawDesc: std::os::windows::io::AsRawSocket {}

pub trait TryClone {
    fn try_to_clone(&self) -> anyhow::Result<Self>
    where
        Self: std::marker::Sized;
}

impl AsRawDesc for UnixStream {}
impl TryClone for UnixStream {
    fn try_to_clone(&self) -> anyhow::Result<Self>
    where
        Self: std::marker::Sized,
    {
        Ok(self.try_clone()?)
    }
}

#[derive(Debug)]
enum Item {
    Notif(MuxNotification),
    Pdu(DecodedPdu),
    Done(String),
}

async fn catch(
    f: impl smol::future::Future<Output = anyhow::Result<()>>,
    tx: smol::channel::Sender<Item>,
) -> anyhow::Result<()> {
    if let Err(err) = f.await {
        tx.try_send(Item::Done(err.to_string())).ok();
    }
    Ok(())
}

async fn process_write_queue<T>(
    mut write_stream: T,
    write_rx: smol::channel::Receiver<DecodedPdu>,
) -> anyhow::Result<()>
where
    T: AsyncWrite + Unpin,
{
    loop {
        let decoded = write_rx.recv().await?;

        log::trace!("writing pdu with serial {}", decoded.serial);
        decoded
            .pdu
            .encode_async(&mut write_stream, decoded.serial)
            .await?;
    }
}

async fn read_pdus<T>(
    mut read_stream: T,
    item_tx: smol::channel::Sender<Item>,
) -> anyhow::Result<()>
where
    T: AsyncRead + Unpin,
{
    loop {
        let decoded = Pdu::decode_async(&mut read_stream).await?;
        item_tx.send(Item::Pdu(decoded)).await?;
    }
}

async fn multiplex(
    write_tx: smol::channel::Sender<DecodedPdu>,
    item_rx: smol::channel::Receiver<Item>,
) -> anyhow::Result<()> {
    let pdu_sender = PduSender::with_smol(write_tx);
    let mut handler = SessionHandler::new(pdu_sender);

    loop {
        let item = match item_rx.recv().await {
            Ok(item) => item,
            Err(err) => {
                log::error!("{}", err);
                return Ok(());
            }
        };
        match item {
            Item::Notif(MuxNotification::PaneOutput(pane_id)) => {
                handler.schedule_pane_push(pane_id);
            }
            Item::Notif(MuxNotification::WindowCreated(_window_id)) => {}
            Item::Pdu(decoded) => {
                handler.process_one(decoded);
            }
            Item::Done(e) => {
                log::error!("{}", e);
                return Ok(());
            }
        }
    }
}

pub async fn process<T>(stream: T) -> anyhow::Result<()>
where
    T: 'static,
    T: TryClone,
    T: std::io::Read,
    T: std::io::Write,
    T: AsRawDesc,
{
    let write_stream = smol::Async::new(stream.try_to_clone()?)?;
    let read_stream = smol::Async::new(stream)?;

    process_async(write_stream, read_stream).await
}

pub async fn process_async<T>(write_stream: T, read_stream: T) -> anyhow::Result<()>
where
    T: 'static,
    T: AsyncRead,
    T: AsyncWrite,
    T: Unpin,
{
    let (item_tx, item_rx) = smol::channel::unbounded::<Item>();
    let (write_tx, write_rx) = smol::channel::unbounded::<DecodedPdu>();

    // Process the PDU write queue to send to the peer
    spawn({
        let item_tx = item_tx.clone();
        async move {
            catch(
                async move { process_write_queue(write_stream, write_rx).await },
                item_tx,
            )
            .await
        }
    });

    {
        let mux = Mux::get().expect("to be running on gui thread");
        let tx = item_tx.clone();
        mux.subscribe(move |n| tx.try_send(Item::Notif(n)).is_ok());
    }

    {
        let item_tx = item_tx.clone();
        spawn(async move {
            let tx = item_tx.clone();
            catch(async move { read_pdus(read_stream, item_tx).await }, tx).await
        });
    }

    catch(async move { multiplex(write_tx, item_rx).await }, item_tx).await
}
