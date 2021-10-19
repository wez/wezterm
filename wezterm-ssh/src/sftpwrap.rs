use crate::dirwrap::DirWrap;
use crate::filewrap::FileWrap;
use crate::sftp::types::{Metadata, OpenOptions, RenameOptions};
use crate::sftp::{SftpChannelError, SftpChannelResult};
use camino::{Utf8Path, Utf8PathBuf};
use std::convert::TryFrom;

pub(crate) enum SftpWrap {
    Ssh2(ssh2::Sftp),
}

impl SftpWrap {
    pub fn open(&self, filename: &Utf8Path, opts: OpenOptions) -> SftpChannelResult<FileWrap> {
        match self {
            Self::Ssh2(sftp) => {
                let flags: ssh2::OpenFlags = opts.into();
                let mode = opts.mode;
                let open_type: ssh2::OpenType = opts.ty.into();

                let file = sftp
                    .open_mode(filename.as_std_path(), flags, mode, open_type)
                    .map_err(SftpChannelError::from)?;
                Ok(FileWrap::Ssh2(file))
            }
        }
    }

    pub fn symlink(&self, path: &Utf8Path, target: &Utf8Path) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(sftp) => sftp
                .symlink(path.as_std_path(), target.as_std_path())
                .map_err(SftpChannelError::from),
        }
    }

    pub fn read_link(&self, filename: &Utf8Path) -> SftpChannelResult<Utf8PathBuf> {
        match self {
            Self::Ssh2(sftp) => sftp
                .readlink(filename.as_std_path())
                .map_err(SftpChannelError::from)
                .and_then(|path| {
                    Utf8PathBuf::try_from(path).map_err(|x| {
                        SftpChannelError::from(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            x,
                        ))
                    })
                }),
        }
    }

    pub fn canonicalize(&self, filename: &Utf8Path) -> SftpChannelResult<Utf8PathBuf> {
        match self {
            Self::Ssh2(sftp) => sftp
                .realpath(filename.as_std_path())
                .map_err(SftpChannelError::from)
                .and_then(|path| {
                    Utf8PathBuf::try_from(path).map_err(|x| {
                        SftpChannelError::from(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            x,
                        ))
                    })
                }),
        }
    }

    pub fn unlink(&self, filename: &Utf8Path) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(sftp) => sftp
                .unlink(filename.as_std_path())
                .map_err(SftpChannelError::from),
        }
    }

    pub fn remove_dir(&self, filename: &Utf8Path) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(sftp) => sftp
                .rmdir(filename.as_std_path())
                .map_err(SftpChannelError::from),
        }
    }

    pub fn create_dir(&self, filename: &Utf8Path, mode: i32) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(sftp) => sftp
                .mkdir(filename.as_std_path(), mode)
                .map_err(SftpChannelError::from),
        }
    }

    pub fn rename(
        &self,
        src: &Utf8Path,
        dest: &Utf8Path,
        opts: RenameOptions,
    ) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(sftp) => sftp
                .rename(src.as_std_path(), dest.as_std_path(), Some(opts.into()))
                .map_err(SftpChannelError::from),
        }
    }

    pub fn symlink_metadata(&self, filename: &Utf8Path) -> SftpChannelResult<Metadata> {
        match self {
            Self::Ssh2(sftp) => sftp
                .lstat(filename.as_std_path())
                .map(Metadata::from)
                .map_err(SftpChannelError::from),
        }
    }

    pub fn metadata(&self, filename: &Utf8Path) -> SftpChannelResult<Metadata> {
        match self {
            Self::Ssh2(sftp) => sftp
                .stat(filename.as_std_path())
                .map(Metadata::from)
                .map_err(SftpChannelError::from),
        }
    }

    pub fn set_metadata(&self, filename: &Utf8Path, metadata: Metadata) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(sftp) => sftp
                .setstat(filename.as_std_path(), metadata.into())
                .map_err(SftpChannelError::from),
        }
    }

    pub fn open_dir(&self, filename: &Utf8Path) -> SftpChannelResult<DirWrap> {
        match self {
            Self::Ssh2(sftp) => sftp
                .opendir(filename.as_std_path())
                .map_err(SftpChannelError::from)
                .map(DirWrap::Ssh2),
        }
    }

    pub fn read_dir(&self, filename: &Utf8Path) -> SftpChannelResult<Vec<(Utf8PathBuf, Metadata)>> {
        match self {
            Self::Ssh2(sftp) => sftp
                .readdir(filename.as_std_path())
                .map_err(SftpChannelError::from)
                .and_then(|entries| {
                    let mut mapped_entries = Vec::new();
                    for (path, stat) in entries {
                        match Utf8PathBuf::try_from(path) {
                            Ok(path) => mapped_entries.push((path, Metadata::from(stat))),
                            Err(x) => {
                                return Err(SftpChannelError::from(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    x,
                                )));
                            }
                        }
                    }

                    Ok(mapped_entries)
                }),
        }
    }
}
