use bitflags::bitflags;

bitflags! {
    struct FileTypeFlags: u32 {
        const DIR = 0o040000;
        const FILE = 0o100000;
        const SYMLINK = 0o120000;
    }
}

bitflags! {
    struct FilePermissionFlags: u32 {
        const OWNER_READ = 0o400;
        const OWNER_WRITE = 0o200;
        const OWNER_EXEC = 0o100;
        const GROUP_READ = 0o40;
        const GROUP_WRITE = 0o20;
        const GROUP_EXEC = 0o10;
        const OTHER_READ = 0o4;
        const OTHER_WRITE = 0o2;
        const OTHER_EXEC = 0o1;
    }
}

/// Represents the type associated with a remote file
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum FileType {
    Dir,
    File,
    Symlink,
    Other,
}

impl FileType {
    /// Returns true if file is a type of directory
    pub fn is_dir(self) -> bool {
        matches!(self, Self::Dir)
    }

    /// Returns true if file is a type of regular file
    pub fn is_file(self) -> bool {
        matches!(self, Self::File)
    }

    /// Returns true if file is a type of symlink
    pub fn is_symlink(self) -> bool {
        matches!(self, Self::Symlink)
    }

    /// Create from a unix mode bitset
    pub fn from_unix_mode(mode: u32) -> Self {
        let flags = FileTypeFlags::from_bits_truncate(mode);
        if flags.contains(FileTypeFlags::DIR) {
            Self::Dir
        } else if flags.contains(FileTypeFlags::FILE) {
            Self::File
        } else if flags.contains(FileTypeFlags::SYMLINK) {
            Self::Symlink
        } else {
            Self::Other
        }
    }

    /// Convert to a unix mode bitset
    pub fn to_unix_mode(self) -> u32 {
        let flags = match self {
            FileType::Dir => FileTypeFlags::DIR,
            FileType::File => FileTypeFlags::FILE,
            FileType::Symlink => FileTypeFlags::SYMLINK,
            FileType::Other => FileTypeFlags::empty(),
        };

        flags.bits
    }
}

/// Represents permissions associated with a remote file
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct FilePermissions {
    pub owner_read: bool,
    pub owner_write: bool,
    pub owner_exec: bool,

    pub group_read: bool,
    pub group_write: bool,
    pub group_exec: bool,

    pub other_read: bool,
    pub other_write: bool,
    pub other_exec: bool,
}

impl FilePermissions {
    /// Returns true if all write permissions (owner, group, other) are false.
    pub fn is_readonly(self) -> bool {
        !(self.owner_write || self.group_write || self.other_write)
    }

    /// Create from a unix mode bitset
    pub fn from_unix_mode(mode: u32) -> Self {
        let flags = FilePermissionFlags::from_bits_truncate(mode);
        Self {
            owner_read: flags.contains(FilePermissionFlags::OWNER_READ),
            owner_write: flags.contains(FilePermissionFlags::OWNER_WRITE),
            owner_exec: flags.contains(FilePermissionFlags::OWNER_EXEC),
            group_read: flags.contains(FilePermissionFlags::GROUP_READ),
            group_write: flags.contains(FilePermissionFlags::GROUP_WRITE),
            group_exec: flags.contains(FilePermissionFlags::GROUP_EXEC),
            other_read: flags.contains(FilePermissionFlags::OTHER_READ),
            other_write: flags.contains(FilePermissionFlags::OTHER_WRITE),
            other_exec: flags.contains(FilePermissionFlags::OTHER_EXEC),
        }
    }

    /// Convert to a unix mode bitset
    pub fn to_unix_mode(self) -> u32 {
        let mut flags = FilePermissionFlags::empty();

        if self.owner_read {
            flags.insert(FilePermissionFlags::OWNER_READ);
        }
        if self.owner_write {
            flags.insert(FilePermissionFlags::OWNER_WRITE);
        }
        if self.owner_exec {
            flags.insert(FilePermissionFlags::OWNER_EXEC);
        }

        if self.group_read {
            flags.insert(FilePermissionFlags::GROUP_READ);
        }
        if self.group_write {
            flags.insert(FilePermissionFlags::GROUP_WRITE);
        }
        if self.group_exec {
            flags.insert(FilePermissionFlags::GROUP_EXEC);
        }

        if self.other_read {
            flags.insert(FilePermissionFlags::OTHER_READ);
        }
        if self.other_write {
            flags.insert(FilePermissionFlags::OTHER_WRITE);
        }
        if self.other_exec {
            flags.insert(FilePermissionFlags::OTHER_EXEC);
        }

        flags.bits
    }
}

/// Represents metadata about a remote file
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct Metadata {
    /// Type of the remote file
    pub ty: FileType,

    /// Permissions associated with the file
    pub permissions: Option<FilePermissions>,

    /// File size, in bytes of the file
    pub size: Option<u64>,

    /// Owner ID of the file
    pub uid: Option<u32>,

    /// Owning group of the file
    pub gid: Option<u32>,

    /// Last access time of the file
    pub accessed: Option<u64>,

    /// Last modification time of the file
    pub modified: Option<u64>,
}

impl Metadata {
    /// Returns true if metadata is for a directory
    pub fn is_dir(self) -> bool {
        self.ty.is_dir()
    }

    /// Returns true if metadata is for a regular file
    pub fn is_file(self) -> bool {
        self.ty.is_file()
    }

    /// Returns true if metadata is for a symlink
    pub fn is_symlink(self) -> bool {
        self.ty.is_symlink()
    }
}

/// Represents options to provide when opening a file or directory
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct OpenOptions {
    /// If true, opens a file (or directory) for reading
    pub read: bool,

    /// If provided, opens a file for writing or appending
    pub write: Option<WriteMode>,

    /// Unix mode that is used when creating a new file
    pub mode: i32,

    /// Whether opening a file or directory
    pub ty: OpenFileType,
}

/// Represents whether opening a file or directory
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum OpenFileType {
    Dir,
    File,
}

/// Represents different writing modes for opening a file
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum WriteMode {
    /// Append data to end of file instead of overwriting it
    Append,

    /// Overwrite an existing file when opening to write it
    Write,
}

/// Represents options to provide when renaming a file or directory
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct RenameOptions {
    /// Overwrite the destination if it exists, otherwise fail
    pub overwrite: bool,

    /// Request atomic rename operation
    pub atomic: bool,

    /// Request native system calls
    pub native: bool,
}

impl Default for RenameOptions {
    /// Default is to enable all options
    fn default() -> Self {
        Self {
            overwrite: true,
            atomic: true,
            native: true,
        }
    }
}

/// Contains libssh2-specific implementations
#[cfg(feature = "ssh2")]
mod ssh2_impl {
    use super::*;
    use ::ssh2::{
        FileStat as Ssh2FileStat, FileType as Ssh2FileType, OpenFlags as Ssh2OpenFlags,
        OpenType as Ssh2OpenType, RenameFlags as Ssh2RenameFlags,
    };

    impl From<OpenFileType> for Ssh2OpenType {
        fn from(ty: OpenFileType) -> Self {
            match ty {
                OpenFileType::Dir => Self::Dir,
                OpenFileType::File => Self::File,
            }
        }
    }

    impl From<RenameOptions> for Ssh2RenameFlags {
        fn from(opts: RenameOptions) -> Self {
            let mut flags = Self::empty();

            if opts.overwrite {
                flags |= Self::OVERWRITE;
            }

            if opts.atomic {
                flags |= Self::ATOMIC;
            }

            if opts.native {
                flags |= Self::NATIVE;
            }

            flags
        }
    }

    impl From<OpenOptions> for Ssh2OpenFlags {
        fn from(opts: OpenOptions) -> Self {
            let mut flags = Self::empty();

            if opts.read {
                flags |= Self::READ;
            }

            match opts.write {
                Some(WriteMode::Write) => flags |= Self::WRITE | Self::TRUNCATE,
                Some(WriteMode::Append) => flags |= Self::WRITE | Self::APPEND | Self::CREATE,
                None => {}
            }

            flags
        }
    }

    impl From<Ssh2FileType> for FileType {
        fn from(ft: Ssh2FileType) -> Self {
            if ft.is_dir() {
                Self::Dir
            } else if ft.is_file() {
                Self::File
            } else if ft.is_symlink() {
                Self::Symlink
            } else {
                Self::Other
            }
        }
    }

    impl From<Ssh2FileStat> for Metadata {
        fn from(stat: Ssh2FileStat) -> Self {
            Self {
                ty: FileType::from(stat.file_type()),
                permissions: stat.perm.map(FilePermissions::from_unix_mode),
                size: stat.size,
                uid: stat.uid,
                gid: stat.gid,
                accessed: stat.atime,
                modified: stat.mtime,
            }
        }
    }

    impl From<Metadata> for Ssh2FileStat {
        fn from(metadata: Metadata) -> Self {
            let ft = metadata.ty;

            Self {
                perm: metadata
                    .permissions
                    .map(|p| p.to_unix_mode() | ft.to_unix_mode()),
                size: metadata.size,
                uid: metadata.uid,
                gid: metadata.gid,
                atime: metadata.accessed,
                mtime: metadata.modified,
            }
        }
    }
}

#[cfg(feature = "libssh-rs")]
mod libssh_impl {
    use super::*;
    use std::time::SystemTime;

    impl From<libssh_rs::FileType> for FileType {
        fn from(ft: libssh_rs::FileType) -> Self {
            match ft {
                libssh_rs::FileType::Directory => Self::Dir,
                libssh_rs::FileType::Regular => Self::File,
                libssh_rs::FileType::Symlink => Self::Symlink,
                _ => Self::Other,
            }
        }
    }

    fn sys_time_to_unix(t: SystemTime) -> u64 {
        t.duration_since(SystemTime::UNIX_EPOCH)
            .expect("UNIX_EPOCH < SystemTime")
            .as_secs()
    }

    fn unix_to_sys(u: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(u)
    }

    impl From<libssh_rs::Metadata> for Metadata {
        fn from(stat: libssh_rs::Metadata) -> Self {
            Self {
                ty: stat
                    .file_type()
                    .map(FileType::from)
                    .unwrap_or(FileType::Other),
                permissions: stat.permissions().map(FilePermissions::from_unix_mode),
                size: stat.len(),
                uid: stat.uid(),
                gid: stat.gid(),
                accessed: stat.accessed().map(sys_time_to_unix),
                modified: stat.modified().map(sys_time_to_unix),
            }
        }
    }

    impl Into<libssh_rs::SetAttributes> for Metadata {
        fn into(self) -> libssh_rs::SetAttributes {
            let size = self.size;
            let uid_gid = match (self.uid, self.gid) {
                (Some(uid), Some(gid)) => Some((uid, gid)),
                _ => None,
            };
            let permissions = self.permissions.map(FilePermissions::to_unix_mode);
            let atime_mtime = match (self.accessed, self.modified) {
                (Some(a), Some(m)) => {
                    let a = unix_to_sys(a);
                    let m = unix_to_sys(m);
                    Some((a, m))
                }
                _ => None,
            };
            libssh_rs::SetAttributes {
                size,
                uid_gid,
                permissions,
                atime_mtime,
            }
        }
    }
}
