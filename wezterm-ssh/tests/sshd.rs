use assert_fs::prelude::*;
use assert_fs::TempDir;
use rstest::*;
use std::collections::HashMap;
use std::io::Result as IoResult;
use std::path::Path;
use std::process::{Child, Command};
use std::sync::LazyLock;
use std::time::Duration;
use wezterm_ssh::{Config, Session, SessionEvent};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// NOTE: OpenSSH's sshd requires absolute path
const BIN_PATH_STR: &str = "/usr/sbin/sshd";

pub fn sshd_available() -> bool {
    Path::new(BIN_PATH_STR).exists()
}

/// Ask the kernel to assign a free port.
/// We pass this to sshd and tell it to listen on that port.
/// This is racy, as releasing the socket technically makes
/// that port available to others using the same technique.
fn allocate_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1:0 failed");
    listener.local_addr().unwrap().port()
}

const USERNAME: LazyLock<String> = LazyLock::new(whoami::username);

pub struct SshKeygen;

impl SshKeygen {
    // ssh-keygen -t rsa -f $ROOT/id_rsa -N "" -q
    pub fn generate_rsa(path: impl AsRef<Path>, passphrase: impl AsRef<str>) -> IoResult<bool> {
        let res = Command::new("ssh-keygen")
            .args(&["-m", "PEM"])
            .args(&["-t", "rsa"])
            .arg("-f")
            .arg(path.as_ref())
            .arg("-N")
            .arg(passphrase.as_ref())
            .arg("-q")
            .status()
            .map(|status| status.success())?;

        #[cfg(unix)]
        if res {
            // chmod 600 id_rsa* -> ida_rsa + ida_rsa.pub
            std::fs::metadata(path.as_ref().with_extension("pub"))?
                .permissions()
                .set_mode(0o600);
            std::fs::metadata(path)?.permissions().set_mode(0o600);
        }

        Ok(res)
    }
}

pub struct SshAgent;

impl SshAgent {
    pub fn generate_shell_env() -> IoResult<HashMap<String, String>> {
        let output = Command::new("ssh-agent").arg("-s").output()?;
        let stdout = String::from_utf8(output.stdout)
            .map_err(|x| std::io::Error::new(std::io::ErrorKind::InvalidData, x))?;
        Ok(stdout
            .split(";")
            .map(str::trim)
            .filter(|s| s.contains("="))
            .map(|s| {
                let mut tokens = s.split("=");
                let key = tokens.next().unwrap().trim().to_string();
                let rest = tokens
                    .map(str::trim)
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join("=");
                (key, rest)
            })
            .collect::<HashMap<String, String>>())
    }

    pub fn update_tests_with_shell_env() -> IoResult<()> {
        let env_map = Self::generate_shell_env()?;
        for (key, value) in env_map {
            std::env::set_var(key, value);
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct SshdConfig(HashMap<String, Vec<String>>);

impl Default for SshdConfig {
    fn default() -> Self {
        let mut config = Self::new();

        config.set_authentication_methods(vec!["publickey".to_string()]);
        config.set_use_privilege_separation(false);
        config.set_subsystem(true, true);
        config.set_use_pam(true);
        config.set_x11_forwarding(true);
        config.set_print_motd(true);
        config.set_permit_tunnel(true);
        config.set_kbd_interactive_authentication(true);
        config.set_allow_tcp_forwarding(true);
        config.set_max_startups(500, None);
        config.set_strict_modes(false);

        config
    }
}

impl SshdConfig {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn set_authentication_methods(&mut self, methods: Vec<String>) {
        self.0.insert("AuthenticationMethods".to_string(), methods);
    }

    pub fn set_authorized_keys_file(&mut self, path: impl AsRef<Path>) {
        self.0.insert(
            "AuthorizedKeysFile".to_string(),
            vec![path.as_ref().to_string_lossy().to_string()],
        );
    }

    pub fn set_host_key(&mut self, path: impl AsRef<Path>) {
        self.0.insert(
            "HostKey".to_string(),
            vec![path.as_ref().to_string_lossy().to_string()],
        );
    }

    pub fn set_pid_file(&mut self, path: impl AsRef<Path>) {
        self.0.insert(
            "PidFile".to_string(),
            vec![path.as_ref().to_string_lossy().to_string()],
        );
    }

    pub fn set_subsystem(&mut self, sftp: bool, internal_sftp: bool) {
        let mut values = Vec::new();
        if sftp {
            values.push("sftp".to_string());
        }
        if internal_sftp {
            values.push("internal-sftp".to_string());
        }

        self.0.insert("Subsystem".to_string(), values);
    }

    pub fn set_use_pam(&mut self, yes: bool) {
        self.0.insert("UsePAM".to_string(), Self::yes_value(yes));
    }

    pub fn set_x11_forwarding(&mut self, yes: bool) {
        self.0
            .insert("X11Forwarding".to_string(), Self::yes_value(yes));
    }

    pub fn set_use_privilege_separation(&mut self, yes: bool) {
        self.0
            .insert("UsePrivilegeSeparation".to_string(), Self::yes_value(yes));
    }

    pub fn set_print_motd(&mut self, yes: bool) {
        self.0.insert("PrintMotd".to_string(), Self::yes_value(yes));
    }

    pub fn set_permit_tunnel(&mut self, yes: bool) {
        self.0
            .insert("PermitTunnel".to_string(), Self::yes_value(yes));
    }

    pub fn set_kbd_interactive_authentication(&mut self, yes: bool) {
        self.0.insert(
            "KbdInteractiveAuthentication".to_string(),
            Self::yes_value(yes),
        );
    }

    pub fn set_allow_tcp_forwarding(&mut self, yes: bool) {
        self.0
            .insert("AllowTcpForwarding".to_string(), Self::yes_value(yes));
    }

    pub fn set_max_startups(&mut self, start: u16, rate_full: Option<(u16, u16)>) {
        let value = format!(
            "{}{}",
            start,
            rate_full
                .map(|(r, f)| format!(":{}:{}", r, f))
                .unwrap_or_default(),
        );

        self.0.insert("MaxStartups".to_string(), vec![value]);
    }

    pub fn set_strict_modes(&mut self, yes: bool) {
        self.0
            .insert("StrictModes".to_string(), Self::yes_value(yes));
    }

    fn yes_value(yes: bool) -> Vec<String> {
        vec![Self::yes_string(yes)]
    }

    fn yes_string(yes: bool) -> String {
        Self::yes_str(yes).to_string()
    }

    const fn yes_str(yes: bool) -> &'static str {
        if yes {
            "yes"
        } else {
            "no"
        }
    }
}

impl std::fmt::Display for SshdConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for (keyword, values) in self.0.iter() {
            writeln!(
                f,
                "{} {}",
                keyword,
                values
                    .iter()
                    .map(|v| {
                        let v = v.trim();
                        if v.contains(|c: char| c.is_whitespace()) {
                            format!("\"{}\"", v)
                        } else {
                            v.to_string()
                        }
                    })
                    .collect::<Vec<String>>()
                    .join(" ")
            )?;
        }
        Ok(())
    }
}

/// Context for some sshd instance
pub struct Sshd {
    child: Child,

    /// Port that sshd is listening on
    pub port: u16,

    /// Temporary directory used to hold resources for sshd such as its config, keys, and log
    pub tmp: TempDir,
}

impl Sshd {
    pub fn spawn(mut config: SshdConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let _ = env_logger::Builder::new()
            .is_test(true)
            .filter_level(log::LevelFilter::Trace)
            .try_init();

        let tmp = TempDir::new()?;

        // Ensure that everything needed for interacting with ssh-agent is set
        SshAgent::update_tests_with_shell_env()?;

        // ssh-keygen -t rsa -f $ROOT/id_rsa -N "" -q
        let id_rsa_file = tmp.child("id_rsa");
        assert!(
            SshKeygen::generate_rsa(id_rsa_file.path(), "")?,
            "Failed to ssh-keygen id_rsa"
        );

        // cp $ROOT/id_rsa.pub $ROOT/authorized_keys
        let authorized_keys_file = tmp.child("authorized_keys");
        std::fs::copy(
            id_rsa_file.path().with_extension("pub"),
            authorized_keys_file.path(),
        )?;

        // ssh-keygen -t rsa -f $ROOT/ssh_host_rsa_key -N "" -q
        let ssh_host_rsa_key_file = tmp.child("ssh_host_rsa_key");
        assert!(
            SshKeygen::generate_rsa(ssh_host_rsa_key_file.path(), "")?,
            "Failed to ssh-keygen ssh_host_rsa_key"
        );

        config.set_authorized_keys_file(id_rsa_file.path().with_extension("pub"));
        config.set_host_key(ssh_host_rsa_key_file.path());

        let sshd_pid_file = tmp.child("sshd.pid");
        config.set_pid_file(sshd_pid_file.path());

        // Generate $ROOT/sshd_config based on config
        let sshd_config_file = tmp.child("sshd_config");
        let config_string = config.to_string();
        sshd_config_file.write_str(&config_string)?;
        eprintln!("{config_string}");

        let sshd_log_file = tmp.child("sshd.log");

        let (child, port) = Self::try_spawn_next(sshd_config_file.path(), sshd_log_file.path())
            .expect("No open port available for sshd");

        Ok(Self { child, port, tmp })
    }

    fn try_spawn_next(
        config_path: impl AsRef<Path>,
        log_path: impl AsRef<Path>,
    ) -> IoResult<(Child, u16)> {
        let mut err = None;

        for _ in 0..100 {
            let port = allocate_port();

            match Self::try_spawn(port, config_path.as_ref(), log_path.as_ref()) {
                // If successful, return our spawned server child process
                Ok(child) => return Ok((child, port)),

                Err(x) => {
                    err.replace(x);
                }
            }
        }

        Err(err.unwrap())
    }

    fn try_spawn(
        port: u16,
        config_path: impl AsRef<Path>,
        log_path: impl AsRef<Path>,
    ) -> IoResult<Child> {
        let mut child = Command::new(BIN_PATH_STR)
            .arg("-D")
            .arg("-p")
            .arg(port.to_string())
            .arg("-f")
            .arg(config_path.as_ref())
            .arg("-E")
            .arg(log_path.as_ref())
            .spawn()
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("spawning {} failed {:#}", BIN_PATH_STR, e),
                )
            })?;

        for _ in 0..10 {
            // Wait until the port is up
            std::thread::sleep(Duration::from_millis(100));

            // If the server exited already, then we know something is wrong!
            if let Some(exit_status) = child.try_wait()? {
                let output = child.wait_with_output()?;
                let code = exit_status.code();
                let msg = format!(
                    "{}\n{}",
                    String::from_utf8(output.stdout).unwrap(),
                    String::from_utf8(output.stderr).unwrap(),
                );

                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!(
                        "{} failed [{}]: {}",
                        BIN_PATH_STR,
                        code.map(|x| x.to_string())
                            .unwrap_or_else(|| String::from("???")),
                        msg
                    ),
                ));
            }

            // If the port is up, then we're good!
            if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
                return Ok(child);
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "ran out of ports when spawning sshd",
        ))
    }
}

impl Drop for Sshd {
    /// Kills server upon drop
    fn drop(&mut self) {
        let _ = self.child.kill();

        // NOTE: Should wait to ensure that the process does not become a zombie
        let _ = self.child.wait();
    }
}

#[fixture]
/// Stand up a singular sshd session and hold onto it for the lifetime
/// of our tests, returning a reference to it with each fixture ref
pub fn sshd() -> Sshd {
    Sshd::spawn(Default::default()).unwrap()
}

pub struct SessionWithSshd {
    _sshd: Sshd,
    session: Session,
}

impl std::ops::Deref for SessionWithSshd {
    type Target = Session;
    fn deref(&self) -> &Session {
        &self.session
    }
}

impl std::ops::DerefMut for SessionWithSshd {
    fn deref_mut(&mut self) -> &mut Session {
        &mut self.session
    }
}

#[fixture]
/// Stand up an sshd instance and then connect to it and perform authentication
pub async fn session(#[default(Config::new())] config: Config, sshd: Sshd) -> SessionWithSshd {
    let port = sshd.port;

    // Do not add the default config files; they take the config of the
    // user that is running the tests which can vary wildly and have
    // inappropriate configuration that disrupts the tests.
    // NO: config.add_default_config_files();

    // Load our config to point to ourselves, using current sshd instance's port,
    // generated identity file, and host file
    let mut config = config.for_host("localhost");
    config.insert("port".to_string(), port.to_string());
    config.insert("wezterm_ssh_verbose".to_string(), "true".to_string());

    // If libssh-rs is not loaded (but ssh2 is), then we use ssh2 as the backend
    #[cfg(not(feature = "libssh-rs"))]
    config.insert("wezterm_ssh_backend".to_string(), "ssh2".to_string());

    config.insert("user".to_string(), USERNAME.to_string());
    config.insert("identitiesonly".to_string(), "yes".to_string());
    config.insert(
        "pubkeyacceptedtypes".to_string(),
        // Ensure that we have ssh-rsa in the list, as debian9
        // seems unhappy without it
        "ssh-rsa,ssh-ed25519,\
                  rsa-sha2-512,rsa-sha2-256,ecdsa-sha2-nistp521,\
                  ecdsa-sha2-nistp384,ecdsa-sha2-nistp256"
            .to_string(),
    );
    config.insert(
        "identityfile".to_string(),
        sshd.tmp
            .child("id_rsa")
            .path()
            .to_str()
            .expect("Failed to get string path for id_rsa")
            .to_string(),
    );
    config.insert(
        "userknownhostsfile".to_string(),
        sshd.tmp
            .child("known_hosts")
            .path()
            .to_str()
            .expect("Failed to get string path for known_hosts")
            .to_string(),
    );

    // Perform our actual connection
    let (session, events) = Session::connect(config.clone()).expect("Failed to connect to sshd");

    // Perform automated authentication, assuming that we have a publickey with empty password
    while let Ok(event) = events.recv().await {
        match event {
            SessionEvent::Banner(banner) => {
                if let Some(banner) = banner {
                    log::trace!("{}", banner);
                }
            }
            SessionEvent::HostVerify(verify) => {
                eprintln!("{}", verify.message);

                // Automatically verify any host
                verify
                    .answer(true)
                    .await
                    .expect("Failed to send host verification");
            }
            SessionEvent::Authenticate(auth) => {
                if !auth.username.is_empty() {
                    eprintln!("Authentication for {}", auth.username);
                }
                if !auth.instructions.is_empty() {
                    eprintln!("{}", auth.instructions);
                }

                // Reply with empty string to all authentication requests
                let answers = vec![String::new(); auth.prompts.len()];
                auth.answer(answers)
                    .await
                    .expect("Failed to send authenticate response");
            }
            SessionEvent::HostVerificationFailed(failed) => {
                panic!("{}", failed);
            }
            SessionEvent::Error(err) => {
                panic!("{}", err);
            }
            SessionEvent::Authenticated => break,
        }
    }

    SessionWithSshd {
        session,
        _sshd: sshd,
    }
}
