#![cfg(any(feature = "openssl", unix))]
use super::*;
use openssl::pkcs12::Pkcs12;
use openssl::pkey::PKey;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream, SslVerifyMode};
use openssl::x509::X509;

struct OpenSSLNetListener {
    acceptor: Arc<SslAcceptor>,
    listener: TcpListener,
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
                bail!("CN `{}` did not match $USER `{}`", cn_str, wanted_unix_name);
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
                                error!("problem with peer cert: {}", err);
                                break;
                            }

                            spawn_into_main_thread(async move {
                                let mut session = clientsession::ClientSession::new(stream);
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

pub fn spawn_tls_listener(tls_server: &TlsDomainServer) -> Result<(), Error> {
    openssl::init();

    let mut acceptor = SslAcceptor::mozilla_modern(SslMethod::tls())?;

    if let Some(cert_file) = tls_server.pem_cert.as_ref() {
        acceptor.set_certificate_file(cert_file, SslFiletype::PEM)?;
    }
    if let Some(chain_file) = tls_server.pem_ca.as_ref() {
        acceptor.set_certificate_chain_file(chain_file)?;
    }
    if let Some(key_file) = tls_server.pem_private_key.as_ref() {
        acceptor.set_private_key_file(key_file, SslFiletype::PEM)?;
    }
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

    acceptor.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT);

    let acceptor = acceptor.build();

    let mut net_listener = OpenSSLNetListener::new(
        TcpListener::bind(&tls_server.bind_address).with_context(|| {
            format!(
                "error binding to mux_server_bind_address {}",
                tls_server.bind_address,
            )
        })?,
        acceptor,
    );
    thread::spawn(move || {
        net_listener.run();
    });
    Ok(())
}

pub fn pem_files_to_identity(
    key: PathBuf,
    cert: Option<PathBuf>,
    chain: Option<PathBuf>,
) -> anyhow::Result<Identity> {
    // This is a bit of a redundant dance around;
    // the native_tls interface only allows for pkcs12
    // encoded identity information, but in my use case
    // I only have pem encoded identity information.
    // We can use openssl to convert the data to pkcs12
    // so that we can then pass it on using the Identity
    // type that native_tls requires.
    let key_bytes = std::fs::read(&key)?;
    let pkey = PKey::private_key_from_pem(&key_bytes)?;

    let cert_bytes = std::fs::read(cert.as_ref().unwrap_or(&key))?;
    let x509_cert = X509::from_pem(&cert_bytes)?;

    let chain_bytes = std::fs::read(chain.as_ref().unwrap_or(&key))?;
    let x509_chain = X509::stack_from_pem(&chain_bytes)?;

    let password = "internal";
    let mut ca_stack = openssl::stack::Stack::new()?;
    for ca in x509_chain.into_iter() {
        ca_stack.push(ca)?;
    }
    let mut builder = Pkcs12::builder();
    builder.ca(ca_stack);
    let pkcs12 = builder.build(password, "", &pkey, &x509_cert)?;

    let der = pkcs12.to_der()?;
    Identity::from_pkcs12(&der, password).with_context(|| {
        format!(
            "error creating identity from pkcs12 generated \
             from PemFiles {}, {:?}, {:?}",
            key.display(),
            cert,
            chain,
        )
    })
}
