use failure::Error;
use std::env;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStrExt;

#[derive(Debug)]
pub struct CommandBuilder {
    args: Vec<OsString>,
}

impl CommandBuilder {
    pub fn new<S: AsRef<OsStr>>(program: S) -> Self {
        Self {
            args: vec![program.as_ref().to_owned()],
        }
    }

    fn search_path(exe: &OsStr) -> OsString {
        if let Some(path) = env::var_os("PATH") {
            let extensions = env::var_os("PATHEXT").unwrap_or(".EXE".into());
            for path in env::split_paths(&path) {
                // Check for exactly the user's string in this path dir
                let candidate = path.join(&exe);
                if candidate.exists() {
                    return candidate.into_os_string();
                }

                // otherwise try tacking on some extensions.
                // Note that this really replaces the extension in the
                // user specified path, so this is potentially wrong.
                for ext in env::split_paths(&extensions) {
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

    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) {
        self.args.push(arg.as_ref().to_owned());
    }

    pub fn args<I, S>(&mut self, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for arg in args {
            self.arg(arg);
        }
    }

    pub fn env<K, V>(&mut self, key: K, val: V)
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        eprintln!(
            "ignoring env {:?}={:?} for child; FIXME: implement this!",
            key.as_ref(),
            val.as_ref()
        );
    }

    pub fn cmdline(&self) -> Result<(Vec<u16>, Vec<u16>), Error> {
        let mut cmdline = Vec::<u16>::new();

        let exe = Self::search_path(&self.args[0]);
        Self::append_quoted(&exe, &mut cmdline);

        // Ensure that we nul terminate the module name, otherwise we'll
        // ask CreateProcessW to start something random!
        let mut exe: Vec<u16> = exe.encode_wide().collect();
        exe.push(0);

        for arg in self.args.iter().skip(1) {
            cmdline.push(' ' as u16);
            ensure!(
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
