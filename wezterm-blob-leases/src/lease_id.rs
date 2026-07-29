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

impl LeaseId {
    pub fn new() -> Self {
        let uuid = Uuid::new_v4();
        let pid = std::process::id();
        Self { uuid, pid }
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }
}
