#[cfg(unix)]
use anyhow::Context;
#[cfg(feature = "serde_support")]
use serde_derive::*;
use std::ffi::{OsStr, OsString};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

/// `CommandBuilder` is used to prepare a command to be spawned into a pty.
/// The interface is intentionally similar to that of `std::process::Command`.
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct CommandBuilder {
    args: Vec<OsString>,
    envs: Vec<(OsString, OsString)>,
    cwd: Option<OsString>,
}

impl CommandBuilder {
    /// Create a new builder instance with argv[0] set to the specified
    /// program.
    pub fn new<S: AsRef<OsStr>>(program: S) -> Self {
        Self {
            args: vec![program.as_ref().to_owned()],
            envs: vec![],
            cwd: None,
        }
    }

    /// Create a new builder instance from a pre-built argument vector
    pub fn from_argv(args: Vec<OsString>) -> Self {
        Self {
            args,
            envs: vec![],
            cwd: None,
        }
    }

    /// Create a new builder instance that will run some idea of a default
    /// program.  Such a builder will panic if `arg` is called on it.
    pub fn new_default_prog() -> Self {
        Self {
            args: vec![],
            envs: vec![],
            cwd: None,
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

    /// Override the value of an environmental variable
    pub fn env<K, V>(&mut self, key: K, val: V)
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.envs
            .push((key.as_ref().to_owned(), val.as_ref().to_owned()));
    }

    pub fn cwd<D>(&mut self, dir: D)
    where
        D: AsRef<OsStr>,
    {
        self.cwd = Some(dir.as_ref().to_owned());
    }

    #[cfg(feature = "ssh")]
    pub(crate) fn iter_env_as_str(&self) -> impl Iterator<Item = (&str, &str)> {
        self.envs.iter().filter_map(|(key, val)| {
            let key = key.to_str()?;
            let val = val.to_str()?;
            Some((key, val))
        })
    }

    #[cfg(feature = "ssh")]
    pub(crate) fn as_unix_command_line(&self) -> anyhow::Result<String> {
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
    /// Convert the CommandBuilder to a `std::process::Command` instance.
    pub(crate) fn as_command(&self) -> anyhow::Result<std::process::Command> {
        let mut cmd = if self.is_default_prog() {
            let mut cmd = std::process::Command::new(&Self::get_shell()?);
            // Run the shell as a login shell.  This is a little shaky; it just
            // happens to be the case that bash, zsh, fish and tcsh use -l
            // to indicate that they are login shells.  Ideally we'd just
            // tell the command builder to prefix argv[0] with a `-`, but
            // Rust doesn't support that.
            cmd.arg("-l");
            let home = Self::get_home_dir()?;
            let dir: &OsStr = self
                .cwd
                .as_ref()
                .map(|dir| dir.as_os_str())
                .filter(|dir| std::path::Path::new(dir).is_dir())
                .unwrap_or(home.as_ref());
            cmd.current_dir(dir);
            cmd
        } else {
            let mut cmd = std::process::Command::new(&self.args[0]);
            cmd.args(&self.args[1..]);
            let home = Self::get_home_dir()?;
            let dir: &OsStr = self
                .cwd
                .as_ref()
                .map(|dir| dir.as_os_str())
                .filter(|dir| std::path::Path::new(dir).is_dir())
                .unwrap_or(home.as_ref());
            cmd.current_dir(dir);
            cmd
        };

        for (key, val) in &self.envs {
            cmd.env(key, val);
        }

        Ok(cmd)
    }

    /// Determine which shell to run.
    /// We take the contents of the $SHELL env var first, then
    /// fall back to looking it up from the password database.
    fn get_shell() -> anyhow::Result<String> {
        std::env::var("SHELL").or_else(|_| {
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
        })
    }

    fn get_home_dir() -> anyhow::Result<String> {
        std::env::var("HOME").or_else(|_| {
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
        })
    }
}

#[cfg(windows)]
impl CommandBuilder {
    fn search_path(exe: &OsStr) -> OsString {
        if let Some(path) = std::env::var_os("PATH") {
            let extensions = std::env::var_os("PATHEXT").unwrap_or(".EXE".into());
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
        self.cwd.as_ref().map(|c| {
            let mut wide = vec![];
            wide.extend(c.encode_wide());
            wide.push(0);
            wide
        })
    }

    /// Constructs an environment block for this spawn attempt.
    /// Uses the current process environment as the base and then
    /// adds/replaces the environment that was specified via the
    /// `env` methods.
    pub(crate) fn environment_block(&self) -> Vec<u16> {
        // Holds an entry with its preferred key case; the environment
        // has case insensitive variable names on windows, so we need
        // to take care to avoid confusing things with conflicting
        // entries, and we'd also like to preserve the original case.
        struct Entry {
            key: OsString,
            value: OsString,
        }

        // Best-effort lowercase transformation of an os string
        fn lowerkey(k: &OsStr) -> OsString {
            if let Some(s) = k.to_str() {
                s.to_lowercase().into()
            } else {
                k.to_os_string()
            }
        }

        // Use a btreemap for a nicer sorted order if you review the
        // environment via `set`.
        let mut env_hash = std::collections::BTreeMap::new();

        // Take the current environment as the base
        for (key, value) in std::env::vars_os() {
            env_hash.insert(lowerkey(&key), Entry { key, value });
        }

        // override with the specified values
        for (key, value) in &self.envs {
            env_hash.insert(
                lowerkey(&key),
                Entry {
                    key: key.clone(),
                    value: value.clone(),
                },
            );
        }

        // and now encode it as wide characters
        let mut block = vec![];

        for entry in env_hash.values() {
            block.extend(entry.key.encode_wide());
            block.push(b'=' as u16);
            block.extend(entry.value.encode_wide());
            block.push(0);
        }
        // and a final terminator for CreateProcessW
        block.push(0);

        block
    }

    pub(crate) fn cmdline(&self) -> anyhow::Result<(Vec<u16>, Vec<u16>)> {
        let mut cmdline = Vec::<u16>::new();

        let exe = if self.is_default_prog() {
            std::env::var_os("ComSpec").unwrap_or("cmd.exe".into())
        } else {
            Self::search_path(&self.args[0])
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
