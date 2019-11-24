use crate::config::*;

#[derive(Default, Debug, Clone, Deserialize)]
pub struct TlsDomainServer {
    /// The address:port combination on which the server will listen
    /// for client connections
    pub bind_address: String,

    /// the path to an x509 PEM encoded private key file
    pub pem_private_key: Option<PathBuf>,

    /// the path to an x509 PEM encoded certificate file
    pub pem_cert: Option<PathBuf>,

    /// the path to an x509 PEM encoded CA chain file
    pub pem_ca: Option<PathBuf>,

    /// A set of paths to load additional CA certificates.
    /// Each entry can be either the path to a directory
    /// or to a PEM encoded CA file.  If an entry is a directory,
    /// then its contents will be loaded as CA certs and added
    /// to the trust store.
    #[serde(default)]
    pub pem_root_certs: Vec<PathBuf>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct TlsDomainClient {
    /// The name of this specific domain.  Must be unique amongst
    /// all types of domain in the configuration file.
    pub name: String,

    /// identifies the host:port pair of the remote server.
    pub remote_address: String,

    /// the path to an x509 PEM encoded private key file
    pub pem_private_key: Option<PathBuf>,

    /// the path to an x509 PEM encoded certificate file
    pub pem_cert: Option<PathBuf>,

    /// the path to an x509 PEM encoded CA chain file
    pub pem_ca: Option<PathBuf>,

    /// A set of paths to load additional CA certificates.
    /// Each entry can be either the path to a directory or to a PEM encoded
    /// CA file.  If an entry is a directory, then its contents will be
    /// loaded as CA certs and added to the trust store.
    #[serde(default)]
    pub pem_root_certs: Vec<PathBuf>,

    /// explicitly control whether the client checks that the certificate
    /// presented by the server matches the hostname portion of
    /// `remote_address`.  The default is true.  This option is made
    /// available for troubleshooting purposes and should not be used outside
    /// of a controlled environment as it weakens the security of the TLS
    /// channel.
    #[serde(default)]
    pub accept_invalid_hostnames: bool,

    /// the hostname string that we expect to match against the common name
    /// field in the certificate presented by the server.  This defaults to
    /// the hostname portion of the `remote_address` configuration and you
    /// should not normally need to override this value.
    pub expected_cn: Option<String>,

    /// If true, connect to this domain automatically at startup
    #[serde(default)]
    pub connect_automatically: bool,
}
