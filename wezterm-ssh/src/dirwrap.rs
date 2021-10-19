use crate::sftp::types::Metadata;
use crate::sftp::{SftpChannelError, SftpChannelResult};
use camino::Utf8PathBuf;
use std::convert::TryFrom;

pub(crate) enum DirWrap {
    Ssh2(ssh2::File),
}

impl DirWrap {
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
}
