use crate::sftp::types::Metadata;
use crate::sftp::{SftpChannelError, SftpChannelResult};
use camino::Utf8PathBuf;
use libssh_rs as libssh;
use std::convert::TryFrom;

pub(crate) enum DirWrap {
    Ssh2(ssh2::File),
    LibSsh(libssh::SftpDir),
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
            Self::LibSsh(dir) => match dir.read_dir() {
                None => Err(SftpChannelError::from(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "no more files",
                ))),
                Some(Err(err)) => Err(SftpChannelError::from(err)),
                Some(Ok(metadata)) => {
                    let path: Utf8PathBuf = metadata
                        .name()
                        .expect("name to be present in read_dir")
                        .into();
                    let md: Metadata = metadata.into();
                    Ok((path, md))
                }
            },
        }
    }
}
