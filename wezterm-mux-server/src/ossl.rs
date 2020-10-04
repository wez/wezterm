use crate::PKI;
use anyhow::{anyhow, Context, Error};
use config::TlsDomainServer;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream, SslVerifyMode};
use openssl::x509::X509;
use promise::spawn::spawn_into_main_thread;
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;

struct OpenSSLNetListener {
    acceptor: Arc<SslAcceptor>,
    listener: TcpListener,
}

struct AsyncSslStream {
    s: SslStream<TcpStream>,
}

impl AsyncSslStream {
    pub fn new(s: SslStream<TcpStream>) -> Self {
        Self { s }
    }
}

impl crate::dispatch::TryClone for AsyncSslStream {
    fn try_to_clone(&self) -> anyhow::Result<Self> {
        use foreign_types_shared::ForeignTypeRef;
        let stream = self.s.get_ref().try_clone()?;
        let s = unsafe { SslStream::from_raw_parts(self.s.ssl().as_ptr(), stream) };
        Ok(Self { s })
    }
}

#[cfg(unix)]
impl std::os::unix::io::AsRawFd for AsyncSslStream {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        self.s.get_ref().as_raw_fd()
    }
}

#[cfg(windows)]
impl std::os::windows::io::AsRawSocket for AsyncSslStream {
    fn as_raw_socket(&self) -> std::os::windows::io::RawSocket {
        self.s.get_ref().as_raw_socket()
    }
}

impl crate::dispatch::AsRawDesc for AsyncSslStream {}

impl std::io::Read for AsyncSslStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        self.s.read(buf)
    }
}

impl std::io::Write for AsyncSslStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.s.write(buf)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.s.flush()
    }
}

impl OpenSSLNetListener {
    pub fn new(listener: TcpListener, acceptor: SslAcceptor) -> Self {
        Self {
            listener,
            acceptor: Arc::new(acceptor),
        }
    }

    /// Authenticates the peer.
    /// The requirements are:
    /// * The peer must have a certificate
    /// * The peer certificate must be trusted
    /// * The peer certificate must include a CN string that is
    ///   either an exact match for the unix username of the
    ///   user running this mux server instance, or must match
    ///   a special encoded prefix set up by a proprietary PKI
    ///   infrastructure in an environment used by the author.
    fn verify_peer_cert<T>(stream: &SslStream<T>) -> anyhow::Result<()> {
        let cert = stream
            .ssl()
            .peer_certificate()
            .ok_or_else(|| anyhow!("no peer cert"))?;
        let subject = cert.subject_name();
        let cn = subject
            .entries_by_nid(openssl::nid::Nid::COMMONNAME)
            .next()
            .ok_or_else(|| anyhow!("cert has no CN"))?;
        let cn_str = cn.data().as_utf8()?.to_string();

        let wanted_unix_name = std::env::var("USER")?;

        if wanted_unix_name == cn_str {
            log::info!(
                "Peer certificate CN `{}` == $USER `{}`",
                cn_str,
                wanted_unix_name
            );
            Ok(())
        } else {
            // Some environments that are used by the author of this
            // program encode the CN in the form `user:unixname/DATA`
            let maybe_encoded = format!("user:{}/", wanted_unix_name);
            if cn_str.starts_with(&maybe_encoded) {
                log::info!(
                    "Peer certificate CN `{}` matches $USER `{}`",
                    cn_str,
                    wanted_unix_name
                );
                Ok(())
            } else {
                anyhow::bail!("CN `{}` did not match $USER `{}`", cn_str, wanted_unix_name);
            }
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
                            if let Err(err) = Self::verify_peer_cert(&stream) {
                                log::error!("problem with peer cert: {}", err);
                                break;
                            }
                            spawn_into_main_thread(async move {
                                crate::dispatch::process(AsyncSslStream::new(stream)).await
                            });
                        }
                        Err(e) => {
                            log::error!("failed TlsAcceptor: {}", e);
                        }
                    }
                }
                Err(err) => {
                    log::error!("accept failed: {}", err);
                    return;
                }
            }
        }
    }
}

pub fn spawn_tls_listener(tls_server: &TlsDomainServer) -> Result<(), Error> {
    openssl::init();

    let mut acceptor = SslAcceptor::mozilla_modern(SslMethod::tls())?;

    let cert_file = tls_server
        .pem_cert
        .clone()
        .unwrap_or_else(|| PKI.server_pem());
    acceptor
        .set_certificate_file(&cert_file, SslFiletype::PEM)
        .context(format!(
            "set_certificate_file to {} for TLS listener",
            cert_file.display()
        ))?;

    if let Some(chain_file) = tls_server.pem_ca.as_ref() {
        acceptor
            .set_certificate_chain_file(&chain_file)
            .context(format!(
                "set_certificate_chain_file to {} for TLS listener",
                chain_file.display()
            ))?;
    }

    let key_file = tls_server
        .pem_private_key
        .clone()
        .unwrap_or_else(|| PKI.server_pem());
    acceptor
        .set_private_key_file(&key_file, SslFiletype::PEM)
        .context(format!(
            "set_private_key_file to {} for TLS listener",
            key_file.display()
        ))?;

    fn load_cert(name: &Path) -> anyhow::Result<X509> {
        let cert_bytes = std::fs::read(name)?;
        log::trace!("loaded {}", name.display());
        Ok(X509::from_pem(&cert_bytes)?)
    }
    for name in &tls_server.pem_root_certs {
        if name.is_dir() {
            for entry in std::fs::read_dir(name)? {
                if let Ok(cert) = load_cert(&entry?.path()) {
                    acceptor.cert_store_mut().add_cert(cert).ok();
                }
            }
        } else {
            acceptor.cert_store_mut().add_cert(load_cert(name)?)?;
        }
    }

    acceptor
        .cert_store_mut()
        .add_cert(load_cert(&PKI.ca_pem())?)?;

    acceptor.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT);

    let acceptor = acceptor.build();

    log::error!("listening with TLS on {:?}", tls_server.bind_address);

    let mut net_listener = OpenSSLNetListener::new(
        TcpListener::bind(&tls_server.bind_address).with_context(|| {
            format!(
                "error binding to mux_server_bind_address {}",
                tls_server.bind_address,
            )
        })?,
        acceptor,
    );
    std::thread::spawn(move || {
        net_listener.run();
    });
    Ok(())
}
