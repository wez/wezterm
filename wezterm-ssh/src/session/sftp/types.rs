const OCTAL_FT_DIR: u32 = 0o040000;
const OCTAL_FT_FILE: u32 = 0o100000;
const OCTAL_FT_SYMLINK: u32 = 0o120000;
const OCTAL_FT_OTHER: u32 = 0;

const OCTAL_PERM_OWNER_READ: u32 = 0o400;
const OCTAL_PERM_OWNER_WRITE: u32 = 0o200;
const OCTAL_PERM_OWNER_EXEC: u32 = 0o100;
const OCTAL_PERM_GROUP_READ: u32 = 0o40;
const OCTAL_PERM_GROUP_WRITE: u32 = 0o20;
const OCTAL_PERM_GROUP_EXEC: u32 = 0o10;
const OCTAL_PERM_OTHER_READ: u32 = 0o4;
const OCTAL_PERM_OTHER_WRITE: u32 = 0o2;
const OCTAL_PERM_OTHER_EXEC: u32 = 0o1;

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
        if mode & OCTAL_FT_DIR != 0 {
            Self::Dir
        } else if mode & OCTAL_FT_FILE != 0 {
            Self::File
        } else if mode & OCTAL_FT_SYMLINK != 0 {
            Self::Symlink
        } else {
            Self::Other
        }
    }

    /// Convert to a unix mode bitset
    pub fn to_unix_mode(self) -> u32 {
        match self {
            FileType::Dir => OCTAL_FT_DIR,
            FileType::File => OCTAL_FT_FILE,
            FileType::Symlink => OCTAL_FT_SYMLINK,
            FileType::Other => OCTAL_FT_OTHER,
        }
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
    pub fn is_readonly(self) -> bool {
        !(self.owner_read || self.group_read || self.other_read)
    }

    /// Create from a unix mode bitset
    pub fn from_unix_mode(mode: u32) -> Self {
        Self {
            owner_read: mode | OCTAL_PERM_OWNER_READ != 0,
            owner_write: mode | OCTAL_PERM_OWNER_WRITE != 0,
            owner_exec: mode | OCTAL_PERM_OWNER_EXEC != 0,
            group_read: mode | OCTAL_PERM_GROUP_READ != 0,
            group_write: mode | OCTAL_PERM_GROUP_WRITE != 0,
            group_exec: mode | OCTAL_PERM_GROUP_EXEC != 0,
            other_read: mode | OCTAL_PERM_OTHER_READ != 0,
            other_write: mode | OCTAL_PERM_OTHER_WRITE != 0,
            other_exec: mode | OCTAL_PERM_OTHER_EXEC != 0,
        }
    }

    /// Convert to a unix mode bitset
    pub fn to_unix_mode(self) -> u32 {
        let mut mode: u32 = 0;

        if self.owner_read {
            mode |= OCTAL_PERM_OWNER_READ;
        }
        if self.owner_write {
            mode |= OCTAL_PERM_OWNER_WRITE;
        }
        if self.owner_exec {
            mode |= OCTAL_PERM_OWNER_EXEC;
        }

        if self.group_read {
            mode |= OCTAL_PERM_GROUP_READ;
        }
        if self.group_write {
            mode |= OCTAL_PERM_GROUP_WRITE;
        }
        if self.group_exec {
            mode |= OCTAL_PERM_GROUP_EXEC;
        }

        if self.other_read {
            mode |= OCTAL_PERM_OTHER_READ;
        }
        if self.other_write {
            mode |= OCTAL_PERM_OTHER_WRITE;
        }
        if self.other_exec {
            mode |= OCTAL_PERM_OTHER_EXEC;
        }

        mode
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
    /// Returns the size of the file, in bytes (or zero if unknown)
    pub fn len(self) -> u64 {
        self.size.unwrap_or(0)
    }

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

    /// Returns true if metadata permissions indicate file is readonly
    pub fn is_readonly(self) -> bool {
        self.permissions
            .map(FilePermissions::is_readonly)
            .unwrap_or_default()
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
