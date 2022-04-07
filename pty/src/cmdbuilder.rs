#[cfg(unix)]
use anyhow::Context;
#[cfg(feature = "serde_support")]
use serde_derive::*;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

/// Used to deal with Windows having case-insensitive environment variables.
#[derive(Clone, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
struct EnvEntry {
    /// Whether or not this environment variable came from the base environment,
    /// as opposed to having been explicitly set by the caller.
    is_from_base_env: bool,

    /// For case-insensitive platforms, the environment variable key in its preferred casing.
    preferred_key: OsString,

    /// The environment variable value.
    value: OsString,
}

impl EnvEntry {
    fn map_key(k: OsString) -> OsString {
        if cfg!(windows) {
            // Best-effort lowercase transformation of an os string
            match k.to_str() {
                Some(s) => s.to_lowercase().into(),
                None => k,
            }
        } else {
            k
        }
    }
}

fn get_base_env() -> BTreeMap<OsString, EnvEntry> {
    std::env::vars_os()
        .map(|(key, value)| {
            (
                EnvEntry::map_key(key.clone()),
                EnvEntry {
                    is_from_base_env: true,
                    preferred_key: key,
                    value,
                },
            )
        })
        .collect()
}

/// `CommandBuilder` is used to prepare a command to be spawned into a pty.
/// The interface is intentionally similar to that of `std::process::Command`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct CommandBuilder {
    args: Vec<OsString>,
    envs: BTreeMap<OsString, EnvEntry>,
    cwd: Option<OsString>,
    #[cfg(unix)]
    pub(crate) umask: Option<libc::mode_t>,
}

impl CommandBuilder {
    /// Create a new builder instance with argv[0] set to the specified
    /// program.
    pub fn new<S: AsRef<OsStr>>(program: S) -> Self {
        Self {
            args: vec![program.as_ref().to_owned()],
            envs: get_base_env(),
            cwd: None,
            #[cfg(unix)]
            umask: None,
        }
    }

    /// Create a new builder instance from a pre-built argument vector
    pub fn from_argv(args: Vec<OsString>) -> Self {
        Self {
            args,
            envs: get_base_env(),
            cwd: None,
            #[cfg(unix)]
            umask: None,
        }
    }

    /// Create a new builder instance that will run some idea of a default
    /// program.  Such a builder will panic if `arg` is called on it.
    pub fn new_default_prog() -> Self {
        Self {
            args: vec![],
            envs: get_base_env(),
            cwd: None,
            #[cfg(unix)]
            umask: None,
        }
    }

    /// Returns true if this builder was created via `new_default_prog`
    pub fn is_default_prog(&self) -> bool {
        self.args.is_empty()
    }

    /// Append an argument to the current command line.
    /// Will panic if called on a builder created via `new_default_prog`.
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) {
        if self.is_default_prog() {
            panic!("attempted to add args to a default_prog builder");
        }
        self.args.push(arg.as_ref().to_owned());
    }

    /// Append a sequence of arguments to the current command line
    pub fn args<I, S>(&mut self, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for arg in args {
            self.arg(arg);
        }
    }

    pub fn get_argv(&self) -> &Vec<OsString> {
        &self.args
    }

    pub fn get_argv_mut(&mut self) -> &mut Vec<OsString> {
        &mut self.args
    }

    /// Override the value of an environmental variable
    pub fn env<K, V>(&mut self, key: K, value: V)
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let key: OsString = key.as_ref().into();
        let value: OsString = value.as_ref().into();
        self.envs.insert(
            EnvEntry::map_key(key.clone()),
            EnvEntry {
                is_from_base_env: false,
                preferred_key: key,
                value: value,
            },
        );
    }

    pub fn env_remove<K>(&mut self, key: K)
    where
        K: AsRef<OsStr>,
    {
        let key = key.as_ref().into();
        self.envs.remove(&EnvEntry::map_key(key));
    }

    pub fn env_clear(&mut self) {
        self.envs.clear();
    }

    fn get_env<K>(&self, key: K) -> Option<&OsStr>
    where
        K: AsRef<OsStr>,
    {
        let key = key.as_ref().into();
        self.envs.get(&EnvEntry::map_key(key)).map(
            |EnvEntry {
                 is_from_base_env: _,
                 preferred_key: _,
                 value,
             }| value.as_os_str(),
        )
    }

    pub fn cwd<D>(&mut self, dir: D)
    where
        D: AsRef<OsStr>,
    {
        self.cwd = Some(dir.as_ref().to_owned());
    }

    pub fn clear_cwd(&mut self) {
        self.cwd.take();
    }

    pub fn get_cwd(&self) -> Option<&OsString> {
        self.cwd.as_ref()
    }

    /// Iterate over the configured environment. Only includes environment
    /// variables set by the caller via `env`, not variables set in the base
    /// environment.
    pub fn iter_extra_env_as_str(&self) -> impl Iterator<Item = (&str, &str)> {
        self.envs.values().filter_map(
            |EnvEntry {
                 is_from_base_env,
                 preferred_key,
                 value,
             }| {
                if *is_from_base_env {
                    None
                } else {
                    let key = preferred_key.to_str()?;
                    let value = value.to_str()?;
                    Some((key, value))
                }
            },
        )
    }

    /// Return the configured command and arguments as a single string,
    /// quoted per the unix shell conventions.
    pub fn as_unix_command_line(&self) -> anyhow::Result<String> {
        let mut strs = vec![];
        for arg in &self.args {
            let s = arg
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("argument cannot be represented as utf8"))?;
            strs.push(s);
        }
        Ok(shell_words::join(strs))
    }
}

#[cfg(unix)]
impl CommandBuilder {
    pub fn umask(&mut self, mask: Option<libc::mode_t>) {
        self.umask = mask;
    }

    fn resolve_path(&self) -> Option<&OsStr> {
        self.get_env("PATH")
    }

    fn search_path(&self, exe: &OsStr, cwd: &OsStr) -> anyhow::Result<OsString> {
        use std::path::Path;
        let exe_path: &Path = exe.as_ref();
        if exe_path.is_relative() {
            let cwd: &Path = cwd.as_ref();
            let abs_path = cwd.join(exe_path);
            if abs_path.exists() {
                return Ok(abs_path.into_os_string());
            }

            if let Some(path) = self.resolve_path() {
                for path in std::env::split_paths(&path) {
                    let candidate = path.join(&exe);
                    if candidate.exists() {
                        return Ok(candidate.into_os_string());
                    }
                }
            }
            anyhow::bail!(
                "Unable to spawn {} because it doesn't exist on the filesystem \
                and was not found in PATH",
                exe_path.display()
            );
        } else {
            if !exe_path.exists() {
                anyhow::bail!(
                    "Unable to spawn {} because it doesn't exist on the filesystem",
                    exe_path.display()
                );
            }

            Ok(exe.to_owned())
        }
    }

    /// Convert the CommandBuilder to a `std::process::Command` instance.
    pub(crate) fn as_command(&self) -> anyhow::Result<std::process::Command> {
        use std::os::unix::process::CommandExt;

        let home = self.get_home_dir()?;
        let dir: &OsStr = self
            .cwd
            .as_ref()
            .map(|dir| dir.as_os_str())
            .filter(|dir| std::path::Path::new(dir).is_dir())
            .unwrap_or(home.as_ref());

        let mut cmd = if self.is_default_prog() {
            let shell = self.get_shell()?;

            let mut cmd = std::process::Command::new(&shell);

            // Run the shell as a login shell by prefixing the shell's
            // basename with `-` and setting that as argv0
            let basename = shell.rsplit('/').next().unwrap_or(&shell);
            cmd.arg0(&format!("-{}", basename));
            cmd
        } else {
            let resolved = self.search_path(&self.args[0], dir)?;
            let mut cmd = std::process::Command::new(&resolved);
            cmd.arg0(&self.args[0]);
            cmd.args(&self.args[1..]);
            cmd
        };

        cmd.current_dir(dir);

        cmd.env_clear();
        cmd.envs(self.envs.values().map(
            |EnvEntry {
                 is_from_base_env: _,
                 preferred_key,
                 value,
             }| (preferred_key.as_os_str(), value.as_os_str()),
        ));

        Ok(cmd)
    }

    /// Determine which shell to run.
    /// We take the contents of the $SHELL env var first, then
    /// fall back to looking it up from the password database.
    pub fn get_shell(&self) -> anyhow::Result<String> {
        if let Some(shell) = self.get_env("SHELL").and_then(OsStr::to_str) {
            return Ok(shell.into());
        }

        let ent = unsafe { libc::getpwuid(libc::getuid()) };
        if ent.is_null() {
            Ok("/bin/sh".into())
        } else {
            use std::ffi::CStr;
            use std::str;
            let shell = unsafe { CStr::from_ptr((*ent).pw_shell) };
            shell
                .to_str()
                .map(str::to_owned)
                .context("failed to resolve shell")
        }
    }

    fn get_home_dir(&self) -> anyhow::Result<String> {
        if let Some(home_dir) = self.get_env("HOME").and_then(OsStr::to_str) {
            return Ok(home_dir.into());
        }

        let ent = unsafe { libc::getpwuid(libc::getuid()) };
        if ent.is_null() {
            Ok("/".into())
        } else {
            use std::ffi::CStr;
            use std::str;
            let home = unsafe { CStr::from_ptr((*ent).pw_dir) };
            home.to_str()
                .map(str::to_owned)
                .context("failed to resolve home dir")
        }
    }
}

#[cfg(windows)]
impl CommandBuilder {
    fn search_path(&self, exe: &OsStr) -> OsString {
        if let Some(path) = self.get_env("PATH") {
            let extensions = self.get_env("PATHEXT").unwrap_or(OsStr::new(".EXE"));
            for path in std::env::split_paths(&path) {
                // Check for exactly the user's string in this path dir
                let candidate = path.join(&exe);
                if candidate.exists() {
                    return candidate.into_os_string();
                }

                // otherwise try tacking on some extensions.
                // Note that this really replaces the extension in the
                // user specified path, so this is potentially wrong.
                for ext in std::env::split_paths(&extensions) {
                    // PATHEXT includes the leading `.`, but `with_extension`
                    // doesn't want that
                    let ext = ext.to_str().expect("PATHEXT entries must be utf8");
                    let path = path.join(&exe).with_extension(&ext[1..]);
                    if path.exists() {
                        return path.into_os_string();
                    }
                }
            }
        }

        exe.to_owned()
    }

    pub(crate) fn current_directory(&self) -> Option<Vec<u16>> {
        use std::path::Path;

        let home: Option<&OsStr> = self
            .get_env("USERPROFILE")
            .filter(|path| Path::new(path).is_dir());
        let cwd: Option<&OsStr> = self.cwd.as_deref().filter(|path| Path::new(path).is_dir());
        let dir: Option<&OsStr> = cwd.or(home);

        dir.map(|dir| {
            let mut wide = vec![];

            if Path::new(dir).is_relative() {
                if let Ok(ccwd) = std::env::current_dir() {
                    wide.extend(ccwd.join(dir).as_os_str().encode_wide());
                } else {
                    wide.extend(dir.encode_wide());
                }
            } else {
                wide.extend(dir.encode_wide());
            }

            wide.push(0);
            wide
        })
    }

    /// Constructs an environment block for this spawn attempt.
    /// Uses the current process environment as the base and then
    /// adds/replaces the environment that was specified via the
    /// `env` methods.
    pub(crate) fn environment_block(&self) -> Vec<u16> {
        // encode the environment as wide characters
        let mut block = vec![];

        for EnvEntry {
            is_from_base_env: _,
            preferred_key,
            value,
        } in self.envs.values()
        {
            block.extend(preferred_key.encode_wide());
            block.push(b'=' as u16);
            block.extend(value.encode_wide());
            block.push(0);
        }
        // and a final terminator for CreateProcessW
        block.push(0);

        block
    }

    pub fn get_shell(&self) -> anyhow::Result<String> {
        let exe: OsString = self
            .get_env("ComSpec")
            .unwrap_or(OsStr::new("cmd.exe"))
            .into();
        Ok(exe.into_string()?)
    }

    pub(crate) fn cmdline(&self) -> anyhow::Result<(Vec<u16>, Vec<u16>)> {
        let mut cmdline = Vec::<u16>::new();

        let exe: OsString = if self.is_default_prog() {
            self.get_env("ComSpec")
                .unwrap_or(OsStr::new("cmd.exe"))
                .into()
        } else {
            self.search_path(&self.args[0])
        };

        Self::append_quoted(&exe, &mut cmdline);

        // Ensure that we nul terminate the module name, otherwise we'll
        // ask CreateProcessW to start something random!
        let mut exe: Vec<u16> = exe.encode_wide().collect();
        exe.push(0);

        for arg in self.args.iter().skip(1) {
            cmdline.push(' ' as u16);
            anyhow::ensure!(
                !arg.encode_wide().any(|c| c == 0),
                "invalid encoding for command line argument {:?}",
                arg
            );
            Self::append_quoted(arg, &mut cmdline);
        }
        // Ensure that the command line is nul terminated too!
        cmdline.push(0);
        Ok((exe, cmdline))
    }

    // Borrowed from https://github.com/hniksic/rust-subprocess/blob/873dfed165173e52907beb87118b2c0c05d8b8a1/src/popen.rs#L1117
    // which in turn was translated from ArgvQuote at http://tinyurl.com/zmgtnls
    fn append_quoted(arg: &OsStr, cmdline: &mut Vec<u16>) {
        if !arg.is_empty()
            && !arg.encode_wide().any(|c| {
                c == ' ' as u16
                    || c == '\t' as u16
                    || c == '\n' as u16
                    || c == '\x0b' as u16
                    || c == '\"' as u16
            })
        {
            cmdline.extend(arg.encode_wide());
            return;
        }
        cmdline.push('"' as u16);

        let arg: Vec<_> = arg.encode_wide().collect();
        let mut i = 0;
        while i < arg.len() {
            let mut num_backslashes = 0;
            while i < arg.len() && arg[i] == '\\' as u16 {
                i += 1;
                num_backslashes += 1;
            }

            if i == arg.len() {
                for _ in 0..num_backslashes * 2 {
                    cmdline.push('\\' as u16);
                }
                break;
            } else if arg[i] == b'"' as u16 {
                for _ in 0..num_backslashes * 2 + 1 {
                    cmdline.push('\\' as u16);
                }
                cmdline.push(arg[i]);
            } else {
                for _ in 0..num_backslashes {
                    cmdline.push('\\' as u16);
                }
                cmdline.push(arg[i]);
            }
            i += 1;
        }
        cmdline.push('"' as u16);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env() {
        let mut cmd = CommandBuilder::new("dummy");
        let package_authors = cmd.get_env("CARGO_PKG_AUTHORS");
        println!("package_authors: {:?}", package_authors);
        assert!(package_authors == Some(OsStr::new("Wez Furlong")));

        cmd.env("foo key", "foo value");
        cmd.env("bar key", "bar value");

        let iterated_envs = cmd.iter_extra_env_as_str().collect::<Vec<_>>();
        println!("iterated_envs: {:?}", iterated_envs);
        assert!(iterated_envs == vec![("bar key", "bar value"), ("foo key", "foo value")]);

        {
            let mut cmd = cmd.clone();
            cmd.env_remove("foo key");

            let iterated_envs = cmd.iter_extra_env_as_str().collect::<Vec<_>>();
            println!("iterated_envs: {:?}", iterated_envs);
            assert!(iterated_envs == vec![("bar key", "bar value")]);
        }

        {
            let mut cmd = cmd.clone();
            cmd.env_remove("bar key");

            let iterated_envs = cmd.iter_extra_env_as_str().collect::<Vec<_>>();
            println!("iterated_envs: {:?}", iterated_envs);
            assert!(iterated_envs == vec![("foo key", "foo value")]);
        }

        {
            let mut cmd = cmd.clone();
            cmd.env_clear();

            let iterated_envs = cmd.iter_extra_env_as_str().collect::<Vec<_>>();
            println!("iterated_envs: {:?}", iterated_envs);
            assert!(iterated_envs.is_empty());
        }
    }

    #[cfg(windows)]
    #[test]
    fn test_env_case_insensitive_override() {
        let mut cmd = CommandBuilder::new("dummy");
        cmd.env("Cargo_Pkg_Authors", "Not Wez");
        assert!(cmd.get_env("cargo_pkg_authors") == Some(OsStr::new("Not Wez")));

        cmd.env_remove("cARGO_pKG_aUTHORS");
        assert!(cmd.get_env("CARGO_PKG_AUTHORS").is_none());
    }
}
