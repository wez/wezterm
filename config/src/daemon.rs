use crate::*;
use std::fs::{File, OpenOptions};
use std::path::PathBuf;

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct DaemonOptions {
    pub pid_file: Option<PathBuf>,
    pub stdout: Option<PathBuf>,
    pub stderr: Option<PathBuf>,
}
impl_lua_conversion!(DaemonOptions);

fn open_log(path: PathBuf) -> anyhow::Result<File> {
    create_user_owned_dirs(
        path.parent()
            .ok_or_else(|| anyhow!("path {} has no parent dir!?", path.display()))?,
    )?;
    let mut options = OpenOptions::new();
    options.write(true).create(true).append(true);
    options
        .open(&path)
        .map_err(|e| anyhow!("failed to open log stream: {}: {}", path.display(), e))
}

impl DaemonOptions {
    #[cfg_attr(windows, allow(dead_code))]
    pub fn pid_file(&self) -> PathBuf {
        self.pid_file
            .as_ref()
            .cloned()
            .unwrap_or_else(|| RUNTIME_DIR.join("pid"))
    }

    #[cfg_attr(windows, allow(dead_code))]
    pub fn stdout(&self) -> PathBuf {
        self.stdout
            .as_ref()
            .cloned()
            .unwrap_or_else(|| RUNTIME_DIR.join("log"))
    }

    #[cfg_attr(windows, allow(dead_code))]
    pub fn stderr(&self) -> PathBuf {
        self.stderr
            .as_ref()
            .cloned()
            .unwrap_or_else(|| RUNTIME_DIR.join("log"))
    }

    #[cfg_attr(windows, allow(dead_code))]
    pub fn open_stdout(&self) -> anyhow::Result<File> {
        open_log(self.stdout())
    }

    #[cfg_attr(windows, allow(dead_code))]
    pub fn open_stderr(&self) -> anyhow::Result<File> {
        open_log(self.stderr())
    }
}
