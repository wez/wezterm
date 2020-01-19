#![cfg(not(any(feature = "openssl", unix)))]
use super::*;
use native_tls::TlsAcceptor;
use std::convert::TryInto;

struct NetListener {
    acceptor: Arc<TlsAcceptor>,
    listener: TcpListener,
}

impl NetListener {
    pub fn new(listener: TcpListener, acceptor: TlsAcceptor) -> Self {
        Self {
            listener,
            acceptor: Arc::new(acceptor),
        }
    }

    fn run(&mut self) {
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    stream.set_nodelay(true).ok();
                    let acceptor = self.acceptor.clone();

                    match acceptor.accept(stream) {
                        Ok(stream) => {
                            spawn_into_main_thread(async move {
                                let mut session = ClientSession::new(stream);
                                thread::spawn(move || session.run());
                            });
                        }
                        Err(e) => {
                            error!("failed TlsAcceptor: {}", e);
                        }
                    }
                }
                Err(err) => {
                    error!("accept failed: {}", err);
                    return;
                }
            }
        }
    }
}

pub fn spawn_tls_listener(tls_server: &TlsDomainServer) -> anyhow::Result<()> {
    let identity = IdentitySource::PemFiles {
        key: tls_server
            .pem_private_key
            .as_ref()
            .ok_or_else(|| anyhow!("missing pem_private_key config value"))?
            .into(),
        cert: tls_server.pem_cert.clone(),
        chain: tls_server.pem_ca.clone(),
    };

    let mut net_listener = NetListener::new(
        TcpListener::bind(&tls_server.bind_address).with_context(|| {
            format!("error binding to bind_address {}", tls_server.bind_address,)
        })?,
        TlsAcceptor::new(identity.try_into()?)?,
    );
    thread::spawn(move || {
        net_listener.run();
    });
    Ok(())
}

pub fn pem_files_to_identity(
    _key: PathBuf,
    _cert: Option<PathBuf>,
    _chain: Option<PathBuf>,
) -> anyhow::Result<Identity> {
    bail!("recompile wezterm using --features openssl")
}
