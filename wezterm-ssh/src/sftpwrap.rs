use crate::dirwrap::DirWrap;
use crate::filewrap::FileWrap;
use crate::sftp::types::{Metadata, OpenOptions, RenameOptions, WriteMode};
use crate::sftp::{SftpChannelError, SftpChannelResult};
use camino::{Utf8Path, Utf8PathBuf};
use libc::{O_APPEND, O_RDONLY, O_RDWR, O_WRONLY};
use libssh_rs as libssh;
use std::convert::{TryFrom, TryInto};

pub(crate) enum SftpWrap {
    Ssh2(ssh2::Sftp),
    LibSsh(libssh::Sftp),
}

fn pathconv(path: std::path::PathBuf) -> SftpChannelResult<Utf8PathBuf> {
    Ok(Utf8PathBuf::try_from(path).map_err(|x| {
        SftpChannelError::from(std::io::Error::new(std::io::ErrorKind::InvalidData, x))
    })?)
}

impl SftpWrap {
    pub fn open(&self, filename: &Utf8Path, opts: OpenOptions) -> SftpChannelResult<FileWrap> {
        match self {
            Self::Ssh2(sftp) => {
                let flags: ssh2::OpenFlags = opts.into();
                let mode = opts.mode;
                let open_type: ssh2::OpenType = opts.ty.into();

                let file = sftp.open_mode(filename.as_std_path(), flags, mode, open_type)?;
                Ok(FileWrap::Ssh2(file))
            }
            Self::LibSsh(sftp) => {
                let accesstype = match (opts.write, opts.read) {
                    (Some(WriteMode::Append), true) => O_RDWR | O_APPEND,
                    (Some(WriteMode::Append), false) => O_WRONLY | O_APPEND,
                    (Some(WriteMode::Write), false) => O_WRONLY,
                    (Some(WriteMode::Write), true) => O_RDWR,
                    (None, true) => O_RDONLY,
                    (None, false) => 0,
                };
                let file =
                    sftp.open(filename.as_str(), accesstype, opts.mode.try_into().unwrap())?;
                Ok(FileWrap::LibSsh(file))
            }
        }
    }

    pub fn symlink(&self, path: &Utf8Path, target: &Utf8Path) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(sftp) => Ok(sftp.symlink(path.as_std_path(), target.as_std_path())?),
            Self::LibSsh(sftp) => Ok(sftp.symlink(path.as_str(), target.as_str())?),
        }
    }

    pub fn read_link(&self, filename: &Utf8Path) -> SftpChannelResult<Utf8PathBuf> {
        match self {
            Self::Ssh2(sftp) => Ok(pathconv(sftp.readlink(filename.as_std_path())?)?),
            Self::LibSsh(sftp) => Ok(sftp.read_link(filename.as_str())?.into()),
        }
    }

    pub fn canonicalize(&self, filename: &Utf8Path) -> SftpChannelResult<Utf8PathBuf> {
        match self {
            Self::Ssh2(sftp) => Ok(pathconv(sftp.realpath(filename.as_std_path())?)?),
            Self::LibSsh(sftp) => Ok(sftp.canonicalize(filename.as_str())?.into()),
        }
    }

    pub fn unlink(&self, filename: &Utf8Path) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(sftp) => Ok(sftp.unlink(filename.as_std_path())?),
            Self::LibSsh(sftp) => Ok(sftp.remove_file(filename.as_str())?),
        }
    }

    pub fn remove_dir(&self, filename: &Utf8Path) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(sftp) => Ok(sftp.rmdir(filename.as_std_path())?),
            Self::LibSsh(sftp) => Ok(sftp.remove_dir(filename.as_str())?),
        }
    }

    pub fn create_dir(&self, filename: &Utf8Path, mode: i32) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(sftp) => Ok(sftp.mkdir(filename.as_std_path(), mode)?),
            Self::LibSsh(sftp) => Ok(sftp.create_dir(filename.as_str(), mode.try_into().unwrap())?),
        }
    }

    pub fn rename(
        &self,
        src: &Utf8Path,
        dest: &Utf8Path,
        opts: RenameOptions,
    ) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(sftp) => {
                Ok(sftp.rename(src.as_std_path(), dest.as_std_path(), Some(opts.into()))?)
            }
            Self::LibSsh(sftp) => Ok(sftp.rename(src.as_str(), dest.as_str())?),
        }
    }

    pub fn symlink_metadata(&self, filename: &Utf8Path) -> SftpChannelResult<Metadata> {
        match self {
            Self::Ssh2(sftp) => Ok(sftp.lstat(filename.as_std_path()).map(Metadata::from)?),
            Self::LibSsh(sftp) => Ok(sftp
                .symlink_metadata(filename.as_str())
                .map(Metadata::from)?),
        }
    }

    pub fn metadata(&self, filename: &Utf8Path) -> SftpChannelResult<Metadata> {
        match self {
            Self::Ssh2(sftp) => Ok(sftp.stat(filename.as_std_path()).map(Metadata::from)?),
            Self::LibSsh(sftp) => Ok(sftp.metadata(filename.as_str()).map(Metadata::from)?),
        }
    }

    pub fn set_metadata(&self, filename: &Utf8Path, metadata: Metadata) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(sftp) => Ok(sftp.setstat(filename.as_std_path(), metadata.into())?),
            Self::LibSsh(sftp) => {
                let attr: libssh::SetAttributes = metadata.into();
                Ok(sftp.set_metadata(filename.as_str(), &attr)?)
            }
        }
    }

    pub fn open_dir(&self, filename: &Utf8Path) -> SftpChannelResult<DirWrap> {
        match self {
            Self::Ssh2(sftp) => Ok(sftp.opendir(filename.as_std_path()).map(DirWrap::Ssh2)?),
            Self::LibSsh(sftp) => Ok(sftp.open_dir(filename.as_str()).map(DirWrap::LibSsh)?),
        }
    }

    pub fn read_dir(&self, filename: &Utf8Path) -> SftpChannelResult<Vec<(Utf8PathBuf, Metadata)>> {
        match self {
            Self::Ssh2(sftp) => {
                let entries = sftp.readdir(filename.as_std_path())?;
                let mut mapped_entries = vec![];
                for (path, stat) in entries {
                    let path = pathconv(path)?;
                    mapped_entries.push((path, Metadata::from(stat)));
                }

                Ok(mapped_entries)
            }
            Self::LibSsh(sftp) => {
                let entries = sftp.read_dir(filename.as_str())?;
                let mut mapped_entries = vec![];
                for metadata in entries {
                    let path = metadata
                        .name()
                        .expect("name to be present in read dir results");
                    if path == "." || path == ".." {
                        continue;
                    }
                    mapped_entries.push((filename.join(path), metadata.into()));
                }

                Ok(mapped_entries)
            }
        }
    }
}
