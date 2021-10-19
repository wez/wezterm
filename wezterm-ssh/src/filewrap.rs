use crate::sftp::types::Metadata;
use crate::sftp::{SftpChannelError, SftpChannelResult};
use libssh_rs as libssh;
use std::io::Write;

pub(crate) enum FileWrap {
    Ssh2(ssh2::File),
    LibSsh(libssh::SftpFile),
}

impl FileWrap {
    pub fn reader(&mut self) -> Box<dyn std::io::Read + '_> {
        match self {
            Self::Ssh2(file) => Box::new(file),
            Self::LibSsh(file) => Box::new(file),
        }
    }

    pub fn writer(&mut self) -> Box<dyn std::io::Write + '_> {
        match self {
            Self::Ssh2(file) => Box::new(file),
            Self::LibSsh(file) => Box::new(file),
        }
    }

    pub fn set_metadata(&mut self, metadata: Metadata) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(file) => Ok(file.setstat(metadata.into())?),
            Self::LibSsh(_file) => Err(libssh::Error::fatal(
                "FileWrap::set_metadata not implemented for libssh::SftpFile",
            )
            .into()),
        }
    }

    pub fn metadata(&mut self) -> SftpChannelResult<Metadata> {
        match self {
            Self::Ssh2(file) => Ok(file.stat().map(Metadata::from)?),
            Self::LibSsh(file) => file
                .metadata()
                .map(Metadata::from)
                .map_err(SftpChannelError::from),
        }
    }

    pub fn fsync(&mut self) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(file) => file.fsync().map_err(SftpChannelError::from),
            Self::LibSsh(file) => Ok(file.flush()?),
        }
    }
}
