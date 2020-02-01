use crate::config::*;
use serde::Deserialize;

#[derive(Default, Debug, Clone, Deserialize)]
pub struct SshDomain {
    /// The name of this specific domain.  Must be unique amongst
    /// all types of domain in the configuration file.
    pub name: String,

    /// identifies the host:port pair of the remote server.
    pub remote_address: String,

    /// Whether agent auth should be disabled
    #[serde(default)]
    pub no_agent_auth: bool,

    /// The username to use for authenticating with the remote host
    pub username: String,

    /// If true, connect to this domain automatically at startup
    #[serde(default)]
    pub connect_automatically: bool,

    #[serde(default = "default_read_timeout")]
    pub timeout: Duration,
}
