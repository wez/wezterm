use crate::*;
use std::path::PathBuf;

/// Configures an instance of a multiplexer that can be communicated
/// with via a unix domain socket
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct UnixDomain {
    /// The name of this specific domain.  Must be unique amongst
    /// all types of domain in the configuration file.
    pub name: String,

    /// The path to the socket.  If unspecified, a resonable default
    /// value will be computed.
    pub socket_path: Option<PathBuf>,

    /// If true, connect to this domain automatically at startup
    #[serde(default)]
    pub connect_automatically: bool,

    /// If true, do not attempt to start this server if we try and fail to
    /// connect to it.
    #[serde(default)]
    pub no_serve_automatically: bool,

    /// If we decide that we need to start the server, the command to run
    /// to set that up.  The default is to spawn:
    /// `wezterm-mux-server --daemonize`
    /// but it can be useful to set this to eg:
    /// `wsl -e wezterm-mux-server --daemonize` to start up
    /// a unix domain inside a wsl container.
    pub serve_command: Option<Vec<String>>,

    /// If true, bypass checking for secure ownership of the
    /// socket_path.  This is not recommended on a multi-user
    /// system, but is useful for example when running the
    /// server inside a WSL container but with the socket
    /// on the host NTFS volume.
    #[serde(default)]
    pub skip_permissions_check: bool,

    #[serde(default = "default_read_timeout")]
    pub read_timeout: Duration,

    #[serde(default = "default_write_timeout")]
    pub write_timeout: Duration,
}
impl_lua_conversion!(UnixDomain);

impl UnixDomain {
    pub fn socket_path(&self) -> PathBuf {
        self.socket_path
            .as_ref()
            .cloned()
            .unwrap_or_else(|| RUNTIME_DIR.join("sock"))
    }

    pub fn default_unix_domains() -> Vec<Self> {
        vec![UnixDomain {
            read_timeout: default_read_timeout(),
            write_timeout: default_read_timeout(),
            ..Default::default()
        }]
    }

    pub fn serve_command(&self) -> anyhow::Result<Vec<OsString>> {
        match self.serve_command.as_ref() {
            Some(cmd) => Ok(cmd.iter().map(Into::into).collect()),
            None => Ok(vec![
                std::env::current_exe()?
                    .with_file_name(if cfg!(windows) {
                        "wezterm-mux-server.exe"
                    } else {
                        "wezterm-mux-server"
                    })
                    .into_os_string(),
                OsString::from("--daemonize"),
            ]),
        }
    }
}
