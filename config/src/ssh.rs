use crate::*;
use std::fmt::Display;
use std::str::FromStr;

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
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
    pub username: Option<String>,

    /// If true, connect to this domain automatically at startup
    #[serde(default)]
    pub connect_automatically: bool,

    #[serde(default = "default_read_timeout")]
    pub timeout: Duration,

    /// The path to the wezterm binary on the remote host
    pub remote_wezterm_path: Option<String>,
}
impl_lua_conversion!(SshDomain);

#[derive(Clone, Debug)]
pub struct SshParameters {
    pub username: Option<String>,
    pub host_and_port: String,
}

impl Display for SshParameters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(user) = &self.username {
            write!(f, "{}@{}", user, self.host_and_port)
        } else {
            write!(f, "{}", self.host_and_port)
        }
    }
}

pub fn username_from_env() -> anyhow::Result<String> {
    #[cfg(unix)]
    const USER: &str = "USER";
    #[cfg(windows)]
    const USER: &str = "USERNAME";

    std::env::var(USER).with_context(|| format!("while resolving {} env var", USER))
}

impl FromStr for SshParameters {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('@').collect();

        if parts.len() == 2 {
            Ok(Self {
                username: Some(parts[0].to_string()),
                host_and_port: parts[1].to_string(),
            })
        } else if parts.len() == 1 {
            Ok(Self {
                username: None,
                host_and_port: parts[0].to_string(),
            })
        } else {
            bail!("failed to parse ssh parameters from `{}`", s);
        }
    }
}
