use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

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

#[cfg(target_os = "linux")]
impl From<&str> for LocalProcessStatus {
    fn from(s: &str) -> Self {
        match s {
            "R" => Self::Run,
            "S" => Self::Sleep,
            "D" => Self::Idle,
            "Z" => Self::Zombie,
            "T" => Self::Stop,
            "t" => Self::Tracing,
            "X" | "x" => Self::Dead,
            "K" => Self::Wakekill,
            "W" => Self::Waking,
            "P" => Self::Parked,
            _ => Self::Unknown,
        }
    }
}

#[cfg(target_os = "macos")]
impl From<u32> for LocalProcessStatus {
    fn from(s: u32) -> Self {
        match s {
            1 => Self::Idle,
            2 => Self::Run,
            3 => Self::Sleep,
            4 => Self::Stop,
            5 => Self::Zombie,
            _ => Self::Unknown,
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
    pub start_time: SystemTime,
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

#[cfg(target_os = "linux")]
impl LocalProcessInfo {
    pub(crate) fn with_root_pid_linux(pid: u32) -> Option<Self> {
        use libc::pid_t;

        let pid = pid as pid_t;

        fn all_pids() -> Vec<pid_t> {
            let mut pids = vec![];
            if let Ok(dir) = std::fs::read_dir("/proc") {
                for entry in dir {
                    if let Ok(entry) = entry {
                        if let Ok(file_type) = entry.file_type() {
                            if file_type.is_dir() {
                                if let Some(name) = entry.file_name().to_str() {
                                    if let Ok(pid) = name.parse::<pid_t>() {
                                        pids.push(pid);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            pids
        }

        struct LinuxStat {
            pid: pid_t,
            name: String,
            status: String,
            ppid: pid_t,
            // Time process started after boot, measured in ticks
            starttime: u64,
        }

        fn info_for_pid(pid: pid_t) -> Option<LinuxStat> {
            let data = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
            let (_pid_space, name) = data.split_once('(')?;
            let (name, fields) = name.rsplit_once(')')?;
            let fields = fields.split_whitespace().collect::<Vec<_>>();

            Some(LinuxStat {
                pid,
                name: name.to_string(),
                status: fields.get(0)?.to_string(),
                ppid: fields.get(1)?.parse().ok()?,
                starttime: fields.get(20)?.parse().ok()?,
            })
        }

        fn exe_for_pid(pid: pid_t) -> PathBuf {
            std::fs::read_link(format!("/proc/{}/exe", pid)).unwrap_or_else(|_| PathBuf::new())
        }
        fn cwd_for_pid(pid: pid_t) -> PathBuf {
            std::fs::read_link(format!("/proc/{}/cwd", pid)).unwrap_or_else(|_| PathBuf::new())
        }

        fn parse_cmdline(pid: pid_t) -> Vec<String> {
            let data = match std::fs::read(format!("/proc/{}/cmdline", pid)) {
                Ok(data) => data,
                Err(_) => return vec![],
            };

            let mut args = vec![];

            let data = data.strip_suffix(&[0]).unwrap_or(&data);

            for arg in data.split(|&c| c == 0) {
                args.push(String::from_utf8_lossy(arg).to_owned().to_string());
            }

            args
        }

        let procs: Vec<_> = all_pids().into_iter().filter_map(info_for_pid).collect();

        fn build_proc(info: &LinuxStat, procs: &[LinuxStat]) -> LocalProcessInfo {
            let mut children = HashMap::new();

            for kid in procs {
                if kid.ppid == info.pid {
                    children.insert(kid.pid as u32, build_proc(kid, procs));
                }
            }

            let executable = exe_for_pid(info.pid);
            let name = info.name.clone();
            let argv = parse_cmdline(info.pid);

            LocalProcessInfo {
                pid: info.pid as _,
                ppid: info.ppid as _,
                name,
                executable,
                cwd: cwd_for_pid(info.pid),
                argv,
                start_time: info.starttime,
                status: info.status.as_str().into(),
                children,
            }
        }

        if let Some(info) = procs.iter().find(|info| info.pid == pid) {
            Some(build_proc(info, &procs))
        } else {
            None
        }
    }
}

#[cfg(windows)]
impl LocalProcessInfo {
    pub(crate) fn with_root_pid_windows(pid: u32) -> Option<Self> {
        use ntapi::ntpebteb::PEB;
        use ntapi::ntpsapi::{
            NtQueryInformationProcess, ProcessBasicInformation, ProcessWow64Information,
            PROCESS_BASIC_INFORMATION,
        };
        use ntapi::ntrtl::RTL_USER_PROCESS_PARAMETERS;
        use ntapi::ntwow64::RTL_USER_PROCESS_PARAMETERS32;
        use std::ffi::OsString;
        use std::mem::MaybeUninit;
        use std::os::windows::ffi::OsStringExt;
        use winapi::shared::minwindef::{FILETIME, HMODULE, LPVOID, MAX_PATH};
        use winapi::shared::ntdef::{FALSE, NT_SUCCESS};
        use winapi::um::handleapi::CloseHandle;
        use winapi::um::memoryapi::ReadProcessMemory;
        use winapi::um::processthreadsapi::{GetProcessTimes, OpenProcess};
        use winapi::um::psapi::{EnumProcessModulesEx, GetModuleFileNameExW, LIST_MODULES_ALL};
        use winapi::um::shellapi::CommandLineToArgvW;
        use winapi::um::tlhelp32::*;
        use winapi::um::winbase::LocalFree;
        use winapi::um::winnt::{HANDLE, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};

        struct Snapshot(HANDLE);

        impl Snapshot {
            pub fn new() -> Option<Self> {
                let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
                if handle.is_null() {
                    None
                } else {
                    Some(Self(handle))
                }
            }

            pub fn iter(&self) -> ProcIter {
                ProcIter {
                    snapshot: &self,
                    first: true,
                }
            }
        }

        impl Drop for Snapshot {
            fn drop(&mut self) {
                unsafe { CloseHandle(self.0) };
            }
        }

        struct ProcIter<'a> {
            snapshot: &'a Snapshot,
            first: bool,
        }

        impl<'a> Iterator for ProcIter<'a> {
            type Item = PROCESSENTRY32W;

            fn next(&mut self) -> Option<Self::Item> {
                let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
                entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as _;
                let res = if self.first {
                    self.first = false;
                    unsafe { Process32FirstW(self.snapshot.0, &mut entry) }
                } else {
                    unsafe { Process32NextW(self.snapshot.0, &mut entry) }
                };
                if res == 0 {
                    None
                } else {
                    Some(entry)
                }
            }
        }

        let snapshot = Snapshot::new()?;
        let procs: Vec<_> = snapshot.iter().collect();

        fn wstr_to_path(slice: &[u16]) -> PathBuf {
            match slice.iter().position(|&c| c == 0) {
                Some(nul) => OsString::from_wide(&slice[..nul]),
                None => OsString::from_wide(slice),
            }
            .into()
        }
        fn wstr_to_string(slice: &[u16]) -> String {
            wstr_to_path(slice).to_string_lossy().into_owned()
        }

        struct ProcParams {
            argv: Vec<String>,
            cwd: PathBuf,
        }

        struct ProcHandle(HANDLE);
        impl ProcHandle {
            fn new(pid: u32) -> Option<Self> {
                let options = PROCESS_QUERY_INFORMATION | PROCESS_VM_READ;
                let handle = unsafe { OpenProcess(options, FALSE as _, pid) };
                if handle.is_null() {
                    return None;
                }
                Some(Self(handle))
            }

            fn hmodule(&self) -> Option<HMODULE> {
                let mut needed = 0;
                let mut hmod = [0 as HMODULE];
                let size = std::mem::size_of_val(&hmod);
                let res = unsafe {
                    EnumProcessModulesEx(
                        self.0,
                        hmod.as_mut_ptr(),
                        size as _,
                        &mut needed,
                        LIST_MODULES_ALL,
                    )
                };
                if res == 0 {
                    None
                } else {
                    Some(hmod[0])
                }
            }

            fn executable(&self) -> Option<PathBuf> {
                let hmod = self.hmodule()?;
                let mut buf = [0u16; MAX_PATH + 1];
                let res =
                    unsafe { GetModuleFileNameExW(self.0, hmod, buf.as_mut_ptr(), buf.len() as _) };
                if res == 0 {
                    None
                } else {
                    Some(wstr_to_path(&buf))
                }
            }

            fn get_peb32_addr(&self) -> Option<LPVOID> {
                let mut peb32_addr = MaybeUninit::<LPVOID>::uninit();
                let res = unsafe {
                    NtQueryInformationProcess(
                        self.0,
                        ProcessWow64Information,
                        peb32_addr.as_mut_ptr() as _,
                        std::mem::size_of::<LPVOID>() as _,
                        std::ptr::null_mut(),
                    )
                };
                if !NT_SUCCESS(res) {
                    return None;
                }
                let peb32_addr = unsafe { peb32_addr.assume_init() };
                if peb32_addr.is_null() {
                    None
                } else {
                    Some(peb32_addr)
                }
            }

            fn get_params(&self) -> Option<ProcParams> {
                match self.get_peb32_addr() {
                    Some(peb32) => self.get_params_32(peb32),
                    None => self.get_params_64(),
                }
            }

            fn get_basic_info(&self) -> Option<PROCESS_BASIC_INFORMATION> {
                let mut info = MaybeUninit::<PROCESS_BASIC_INFORMATION>::uninit();
                let res = unsafe {
                    NtQueryInformationProcess(
                        self.0,
                        ProcessBasicInformation,
                        info.as_mut_ptr() as _,
                        std::mem::size_of::<PROCESS_BASIC_INFORMATION>() as _,
                        std::ptr::null_mut(),
                    )
                };
                if !NT_SUCCESS(res) {
                    return None;
                }
                let info = unsafe { info.assume_init() };
                Some(info)
            }

            fn read_struct<T>(&self, addr: LPVOID) -> Option<T> {
                let mut data = MaybeUninit::<T>::uninit();
                let res = unsafe {
                    ReadProcessMemory(
                        self.0,
                        addr as _,
                        data.as_mut_ptr() as _,
                        std::mem::size_of::<T>() as _,
                        std::ptr::null_mut(),
                    )
                };
                if res == 0 {
                    return None;
                }
                let data = unsafe { data.assume_init() };
                Some(data)
            }

            fn get_peb(&self, info: &PROCESS_BASIC_INFORMATION) -> Option<PEB> {
                self.read_struct(info.PebBaseAddress as _)
            }

            fn get_proc_params(&self, peb: &PEB) -> Option<RTL_USER_PROCESS_PARAMETERS> {
                self.read_struct(peb.ProcessParameters as _)
            }

            fn get_params_64(&self) -> Option<ProcParams> {
                let info = self.get_basic_info()?;
                let peb = self.get_peb(&info)?;
                let params = self.get_proc_params(&peb)?;

                let cmdline = self.read_process_wchar(
                    params.CommandLine.Buffer as _,
                    params.CommandLine.Length as _,
                )?;
                let cwd = self.read_process_wchar(
                    params.CurrentDirectory.DosPath.Buffer as _,
                    params.CurrentDirectory.DosPath.Length as _,
                )?;

                Some(ProcParams {
                    argv: cmd_line_to_argv(&cmdline),
                    cwd: wstr_to_path(&cwd),
                })
            }

            fn get_proc_params_32(&self, peb32: LPVOID) -> Option<RTL_USER_PROCESS_PARAMETERS32> {
                self.read_struct(peb32)
            }

            fn get_params_32(&self, peb32: LPVOID) -> Option<ProcParams> {
                let params = self.get_proc_params_32(peb32)?;

                let cmdline = self.read_process_wchar(
                    params.CommandLine.Buffer as _,
                    params.CommandLine.Length as _,
                )?;
                let cwd = self.read_process_wchar(
                    params.CurrentDirectory.DosPath.Buffer as _,
                    params.CurrentDirectory.DosPath.Length as _,
                )?;

                Some(ProcParams {
                    argv: cmd_line_to_argv(&cmdline),
                    cwd: wstr_to_path(&cwd),
                })
            }

            fn read_process_wchar(&self, ptr: LPVOID, size: usize) -> Option<Vec<u16>> {
                let mut buf = vec![0u16; size / 2];

                let res = unsafe {
                    ReadProcessMemory(
                        self.0,
                        ptr as _,
                        buf.as_mut_ptr() as _,
                        size,
                        std::ptr::null_mut(),
                    )
                };
                if res == 0 {
                    return None;
                }

                Some(buf)
            }

            fn start_time(&self) -> Option<SystemTime> {
                let mut start = FILETIME {
                    dwLowDateTime: 0,
                    dwHighDateTime: 0,
                };
                let mut exit = FILETIME {
                    dwLowDateTime: 0,
                    dwHighDateTime: 0,
                };
                let mut kernel = FILETIME {
                    dwLowDateTime: 0,
                    dwHighDateTime: 0,
                };
                let mut user = FILETIME {
                    dwLowDateTime: 0,
                    dwHighDateTime: 0,
                };
                let res = unsafe {
                    GetProcessTimes(self.0, &mut start, &mut exit, &mut kernel, &mut user)
                };
                if res == 0 {
                    return None;
                }

                // Units are 100 nanoseconds
                let start = (start.dwHighDateTime as u64) << 32 | start.dwLowDateTime as u64;
                let start = Duration::from_nanos(start * 100);

                // Difference between the windows epoch and the unix epoch
                const WINDOWS_EPOCH: Duration = Duration::from_secs(11_644_473_600);

                Some(SystemTime::UNIX_EPOCH + start - WINDOWS_EPOCH)
            }
        }

        fn cmd_line_to_argv(buf: &[u16]) -> Vec<String> {
            let mut argc = 0;
            let argvp = unsafe { CommandLineToArgvW(buf.as_ptr(), &mut argc) };
            if argvp.is_null() {
                return vec![];
            }

            let argv = unsafe { std::slice::from_raw_parts(argvp, argc as usize) };
            let mut args = vec![];
            for &arg in argv {
                let len = unsafe { libc::wcslen(arg) };
                let arg = unsafe { std::slice::from_raw_parts(arg, len) };
                args.push(wstr_to_string(arg));
            }
            unsafe { LocalFree(argvp as _) };
            args
        }

        impl Drop for ProcHandle {
            fn drop(&mut self) {
                unsafe { CloseHandle(self.0) };
            }
        }

        fn build_proc(info: &PROCESSENTRY32W, procs: &[PROCESSENTRY32W]) -> LocalProcessInfo {
            let mut children = HashMap::new();

            for kid in procs {
                if kid.th32ParentProcessID == info.th32ProcessID {
                    children.insert(kid.th32ProcessID, build_proc(kid, procs));
                }
            }

            let mut executable = wstr_to_path(&info.szExeFile);

            let name = match executable.file_name() {
                Some(name) => name.to_string_lossy().into_owned(),
                None => String::new(),
            };

            let mut start_time = SystemTime::now();
            let mut cwd = PathBuf::new();
            let mut argv = vec![];

            if let Some(proc) = ProcHandle::new(info.th32ProcessID) {
                if let Some(exe) = proc.executable() {
                    executable = exe;
                }
                if let Some(params) = proc.get_params() {
                    cwd = params.cwd;
                    argv = params.argv;
                }
                if let Some(start) = proc.start_time() {
                    start_time = start;
                }
            }

            LocalProcessInfo {
                pid: info.th32ProcessID,
                ppid: info.th32ParentProcessID,
                name,
                executable,
                cwd,
                argv,
                start_time,
                status: LocalProcessStatus::Run,
                children,
            }
        }

        if let Some(info) = procs.iter().find(|info| info.th32ProcessID == pid) {
            Some(build_proc(info, &procs))
        } else {
            None
        }
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

            let name = unsafe { std::ffi::CStr::from_ptr(info.pbi_comm.as_ptr() as _) };
            let name = name.to_str().unwrap_or("").to_string();

            LocalProcessInfo {
                pid: info.pbi_pid,
                ppid: info.pbi_ppid,
                name,
                executable,
                cwd: cwd_for_pid(info.pbi_pid as _),
                argv,
                start_time: info.pbi_start_tvsec,
                status: LocalProcessStatus::from(info.pbi_status),
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
