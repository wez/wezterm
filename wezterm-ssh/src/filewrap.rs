use crate::sftp::{Metadata, SftpChannelError, SftpChannelResult};
use camino::Utf8PathBuf;
use std::convert::TryFrom;

pub(crate) enum FileWrap {
    Ssh2(ssh2::File),
}

impl FileWrap {
    pub fn reader(&mut self) -> impl std::io::Read + '_ {
        match self {
            Self::Ssh2(file) => file,
        }
    }

    pub fn writer(&mut self) -> impl std::io::Write + '_ {
        match self {
            Self::Ssh2(file) => file,
        }
    }

    pub fn set_metadata(&mut self, metadata: Metadata) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(file) => file
                .setstat(metadata.into())
                .map_err(SftpChannelError::from),
        }
    }

    pub fn metadata(&mut self) -> SftpChannelResult<Metadata> {
        match self {
            Self::Ssh2(file) => file
                .stat()
                .map(Metadata::from)
                .map_err(SftpChannelError::from),
        }
    }

    pub fn read_dir(&mut self) -> SftpChannelResult<(Utf8PathBuf, Metadata)> {
        match self {
            Self::Ssh2(file) => {
                file.readdir()
                    .map_err(SftpChannelError::from)
                    .and_then(|(path, stat)| match Utf8PathBuf::try_from(path) {
                        Ok(path) => Ok((path, Metadata::from(stat))),
                        Err(x) => Err(SftpChannelError::from(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            x,
                        ))),
                    })
            }
        }
    }

    pub fn fsync(&mut self) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(file) => file.fsync().map_err(SftpChannelError::from),
        }
    }
}
