use crate::sftp::types::Metadata;
use crate::sftp::{SftpChannelError, SftpChannelResult};

pub(crate) enum FileWrap {
    #[cfg(feature = "ssh2")]
    Ssh2(ssh2::File),

    #[cfg(feature = "libssh-rs")]
    LibSsh(libssh_rs::SftpFile),
}

impl FileWrap {
    pub fn reader(&mut self) -> Box<dyn std::io::Read + '_> {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(file) => Box::new(file),

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(file) => Box::new(file),
        }
    }

    pub fn writer(&mut self) -> Box<dyn std::io::Write + '_> {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(file) => Box::new(file),

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(file) => Box::new(file),
        }
    }

    pub fn set_metadata(
        &mut self,
        #[cfg_attr(not(feature = "ssh2"), allow(unused_variables))] metadata: Metadata,
    ) -> SftpChannelResult<()> {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(file) => Ok(file.setstat(metadata.into())?),

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(_file) => Err(libssh_rs::Error::fatal(
                "FileWrap::set_metadata not implemented for libssh::SftpFile",
            )
            .into()),
        }
    }

    pub fn metadata(&mut self) -> SftpChannelResult<Metadata> {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(file) => Ok(file.stat().map(Metadata::from)?),

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(file) => file
                .metadata()
                .map(Metadata::from)
                .map_err(SftpChannelError::from),
        }
    }

    pub fn fsync(&mut self) -> SftpChannelResult<()> {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(file) => file.fsync().map_err(SftpChannelError::from),

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(file) => {
                use std::io::Write;
                Ok(file.flush()?)
            }
        }
    }
}
