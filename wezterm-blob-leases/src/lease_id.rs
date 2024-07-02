use once_cell::sync::Lazy;
use uuid::Uuid;

/// Represents an individual lease
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct LeaseId {
    uuid: Uuid,
    pid: u32,
}

impl std::fmt::Display for LeaseId {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "lease:pid={},{}", self.pid, self.uuid.hyphenated())
    }
}

fn get_mac_address() -> [u8; 6] {
    match mac_address::get_mac_address() {
        Ok(Some(addr)) => addr.bytes(),
        _ => {
            let mut mac = [0u8; 6];
            getrandom::getrandom(&mut mac).ok();
            mac
        }
    }
}

impl LeaseId {
    pub fn new() -> Self {
        static MAC: Lazy<[u8; 6]> = Lazy::new(get_mac_address);
        let uuid = Uuid::now_v1(&MAC);
        let pid = std::process::id();
        Self { uuid, pid }
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }
}

impl Default for LeaseId {
    fn default() -> Self {
        Self::new()
    }
}
