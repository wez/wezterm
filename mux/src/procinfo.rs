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

#[cfg(any(windows, target_os = "linux"))]
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

#[cfg(target_os = "macos")]
impl LocalProcessInfo {
    pub(crate) fn with_root_pid_macos(pid: u32) -> Option<Self> {
        /// Enumerate all current process identifiers
        fn all_pids() -> Vec<libc::pid_t> {
            let num_pids = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
            if num_pids < 1 {
                return vec![];
            }

            // Give a bit of padding to avoid looping if processes are spawning
            // rapidly while we're trying to collect this info
            const PADDING: usize = 32;
            let mut pids: Vec<libc::pid_t> = Vec::with_capacity(num_pids as usize + PADDING);
            loop {
                let n = unsafe {
                    libc::proc_listallpids(
                        pids.as_mut_ptr() as *mut _,
                        (pids.capacity() * std::mem::size_of::<libc::pid_t>()) as _,
                    )
                };

                if n < 1 {
                    return vec![];
                }

                let n = n as usize;

                if n > pids.capacity() {
                    pids.reserve(n + PADDING);
                    continue;
                }

                unsafe { pids.set_len(n) };
                return pids;
            }
        }

        /// Obtain info block for a pid.
        /// Note that the process could have gone away since we first
        /// observed the pid and the time we call this, so we must
        /// be able to tolerate this failing.
        fn info_for_pid(pid: libc::pid_t) -> Option<libc::proc_bsdinfo> {
            let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
            let wanted_size = std::mem::size_of::<libc::proc_bsdinfo>() as _;
            let res = unsafe {
                libc::proc_pidinfo(
                    pid,
                    libc::PROC_PIDTBSDINFO,
                    0,
                    &mut info as *mut _ as *mut _,
                    wanted_size,
                )
            };

            if res == wanted_size {
                Some(info)
            } else {
                None
            }
        }

        fn cwd_for_pid(pid: libc::pid_t) -> PathBuf {
            let mut pathinfo: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
            let size = std::mem::size_of_val(&pathinfo) as libc::c_int;
            let ret = unsafe {
                libc::proc_pidinfo(
                    pid,
                    libc::PROC_PIDVNODEPATHINFO,
                    0,
                    &mut pathinfo as *mut _ as *mut _,
                    size,
                )
            };
            if ret == size {
                let path =
                    unsafe { std::ffi::CStr::from_ptr(pathinfo.pvi_cdir.vip_path.as_ptr() as _) };
                path.to_str().unwrap_or("").into()
            } else {
                PathBuf::new()
            }
        }

        fn exe_and_args_for_pid_sysctl(pid: libc::pid_t) -> Option<(PathBuf, Vec<String>)> {
            use libc::c_int;
            let mut size = 64 * 1024;
            let mut buf: Vec<u8> = Vec::with_capacity(size);
            let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as c_int];

            let res = unsafe {
                libc::sysctl(
                    mib.as_mut_ptr(),
                    mib.len() as _,
                    buf.as_mut_ptr() as *mut _,
                    &mut size,
                    std::ptr::null_mut(),
                    0,
                )
            };
            if res == -1 {
                return None;
            }
            if size < (std::mem::size_of::<c_int>() * 2) {
                // Not big enough
                return None;
            }
            unsafe { buf.set_len(size) };

            // The data in our buffer is laid out like this:
            // argc - c_int
            // exe_path - NUL terminated string
            // argv[0] - NUL terminated string
            // argv[1] - NUL terminated string
            // ...
            // argv[n] - NUL terminated string
            // envp[0] - NUL terminated string
            // ...

            let mut ptr = &buf[0..size];

            let argc: c_int = unsafe { std::ptr::read(ptr.as_ptr() as *const c_int) };
            ptr = &ptr[std::mem::size_of::<c_int>()..];

            fn consume_cstr(ptr: &mut &[u8]) -> Option<String> {
                let nul = ptr.iter().position(|&c| c == 0)?;
                let s = String::from_utf8_lossy(&ptr[0..nul]).to_owned().to_string();
                *ptr = ptr.get(nul + 1..)?;
                Some(s)
            }

            let exe_path = consume_cstr(&mut ptr)?.into();

            let mut args = vec![];
            for _ in 0..argc {
                args.push(consume_cstr(&mut ptr)?);
            }

            Some((exe_path, args))
        }

        fn exe_for_pid(pid: libc::pid_t) -> PathBuf {
            let mut buffer: Vec<u8> = Vec::with_capacity(libc::PROC_PIDPATHINFO_MAXSIZE as _);
            let x = unsafe {
                libc::proc_pidpath(
                    pid,
                    buffer.as_mut_ptr() as *mut _,
                    libc::PROC_PIDPATHINFO_MAXSIZE as _,
                )
            };
            if x > 0 {
                unsafe { buffer.set_len(x as usize) };
                String::from_utf8_lossy(&buffer)
                    .to_owned()
                    .to_string()
                    .into()
            } else {
                PathBuf::new()
            }
        }

        let procs: Vec<_> = all_pids().into_iter().filter_map(info_for_pid).collect();

        fn build_proc(info: &libc::proc_bsdinfo, procs: &[libc::proc_bsdinfo]) -> LocalProcessInfo {
            let mut children = HashMap::new();

            for kid in procs {
                if kid.pbi_ppid == info.pbi_pid {
                    children.insert(kid.pbi_pid, build_proc(kid, procs));
                }
            }

            let (executable, argv) = exe_and_args_for_pid_sysctl(info.pbi_pid as _)
                .unwrap_or_else(|| (exe_for_pid(info.pbi_pid as _), vec![]));

            let name = executable
                .file_name()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_string();

            LocalProcessInfo {
                pid: info.pbi_pid,
                ppid: info.pbi_ppid,
                name,
                executable,
                cwd: cwd_for_pid(info.pbi_pid as _),
                argv,
                start_time: info.pbi_start_tvsec,
                status: LocalProcessStatus::Idle,
                children,
            }
        }

        if let Some(info) = procs.iter().find(|info| info.pbi_pid == pid) {
            Some(build_proc(info, &procs))
        } else {
            None
        }
    }
}

#[cfg(any(windows, target_os = "linux"))]
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
