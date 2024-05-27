use clap::Parser;
use codec::{Pdu, SetClientId};
use config::ConfigHandle;
use mux::activity::Activity;
use mux::client::ClientId;
use mux::Mux;
use std::io::{Read, Write};
use std::sync::Arc;
use wezterm_client::client::{unix_connect_with_retry, Client};
use wezterm_client::domain::ClientDomainConfig;

#[derive(Debug, Parser, Clone)]
pub struct ProxyCommand {}

impl ProxyCommand {
    pub async fn run(&self, client: Client, _config: &ConfigHandle) -> anyhow::Result<()> {
        // The client object we created above will have spawned
        // the server if needed, so now all we need to do is turn
        // ourselves into basically netcat.

        // Extract the selected configuration from the client,
        // closing it in the process
        let ClientDomainConfig::Unix(unix_dom) = client.into_client_domain_config() else {
            anyhow::bail!("expected client to have connected to a unix domain");
        };

        let mux = Arc::new(mux::Mux::new(None));
        Mux::set_mux(&mux);

        let target = unix_dom.target();
        let mut stream = unix_connect_with_retry(&target, false, None)?;

        let pdu = Pdu::SetClientId(SetClientId {
            client_id: ClientId::new(),
            is_proxy: true,
        });
        let serial = 1;
        pdu.encode(&mut stream, serial)?;
        Pdu::decode(&mut stream)?;

        // Spawn a thread to pull data from the socket and write
        // it to stdout
        let duped = stream.try_clone()?;
        let activity = Activity::new();
        std::thread::spawn(move || {
            let stdout = std::io::stdout();
            consume_stream_then_exit_process(duped, stdout.lock(), activity);
        });

        // and pull data from stdin and write it to the socket
        let activity = Activity::new();
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            consume_stream_then_exit_process(stdin.lock(), stream, activity);
        });

        // Wait forever; the stdio threads will terminate on EOF
        smol::future::pending().await
    }
}

fn consume_stream<F: Read, T: Write>(mut from_stream: F, mut to_stream: T) -> anyhow::Result<()> {
    let mut buf = [0u8; 8192];

    loop {
        let size = from_stream.read(&mut buf)?;
        if size == 0 {
            break;
        }
        to_stream.write_all(&buf[0..size])?;
        to_stream.flush()?;
    }
    Ok(())
}

fn consume_stream_then_exit_process<F: Read, T: Write>(
    from_stream: F,
    to_stream: T,
    activity: Activity,
) -> ! {
    consume_stream(from_stream, to_stream).ok();
    std::thread::sleep(std::time::Duration::new(2, 0));
    drop(activity);
    std::process::exit(0);
}
