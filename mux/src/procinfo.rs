use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum LocalProcessStatus {
    Idle,
    Run,
    Sleep,
    Stop,
    Zombie,
    Tracing,
    Dead,
    Wakekill,
    Waking,
    Parked,
    LockBlocked,
    Unknown,
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
impl LocalProcessStatus {
    fn from_process_status(status: sysinfo::ProcessStatus) -> Self {
        match status {
            sysinfo::ProcessStatus::Idle => Self::Idle,
            sysinfo::ProcessStatus::Run => Self::Run,
            sysinfo::ProcessStatus::Sleep => Self::Sleep,
            sysinfo::ProcessStatus::Stop => Self::Stop,
            sysinfo::ProcessStatus::Zombie => Self::Zombie,
            sysinfo::ProcessStatus::Tracing => Self::Tracing,
            sysinfo::ProcessStatus::Dead => Self::Dead,
            sysinfo::ProcessStatus::Wakekill => Self::Wakekill,
            sysinfo::ProcessStatus::Waking => Self::Waking,
            sysinfo::ProcessStatus::Parked => Self::Parked,
            sysinfo::ProcessStatus::LockBlocked => Self::LockBlocked,
            sysinfo::ProcessStatus::Unknown(_) => Self::Unknown,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub executable: PathBuf,
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    pub status: LocalProcessStatus,
    pub children: HashMap<u32, LocalProcessInfo>,
    pub start_time: u64,
}
luahelper::impl_lua_conversion!(LocalProcessInfo);

impl LocalProcessInfo {
    pub fn flatten_to_exe_names(&self) -> HashSet<String> {
        let mut names = HashSet::new();

        fn flatten(item: &LocalProcessInfo, names: &mut HashSet<String>) {
            if let Some(exe) = item.executable.file_name() {
                names.insert(exe.to_string_lossy().into_owned());
            }
            for proc in item.children.values() {
                flatten(proc, names);
            }
        }

        flatten(self, &mut names);
        names
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
impl LocalProcessInfo {
    pub(crate) fn with_root_pid(system: &sysinfo::System, pid: u32) -> Option<Self> {
        use sysinfo::{AsU32, Pid, Process, ProcessExt, SystemExt};

        fn build_proc(proc: &Process, processes: &HashMap<Pid, Process>) -> LocalProcessInfo {
            // Process has a `tasks` field but it does not correspond to child processes,
            // so we need to repeatedly walk the full process list and establish that
            // linkage for ourselves here
            let mut children = HashMap::new();
            let pid = proc.pid();
            for (child_pid, child_proc) in processes {
                if child_proc.parent() == Some(pid) {
                    children.insert(child_pid.as_u32(), build_proc(child_proc, processes));
                }
            }

            LocalProcessInfo {
                pid: proc.pid().as_u32(),
                ppid: proc.parent().map(|pid| pid.as_u32()).unwrap_or(1),
                name: proc.name().to_string(),
                executable: proc.exe().to_path_buf(),
                cwd: proc.cwd().to_path_buf(),
                argv: proc.cmd().to_vec(),
                start_time: proc.start_time(),
                status: LocalProcessStatus::from_process_status(proc.status()),
                children,
            }
        }

        let proc = system.process(pid as Pid)?;
        let procs = system.processes();

        Some(build_proc(&proc, &procs))
    }
}
