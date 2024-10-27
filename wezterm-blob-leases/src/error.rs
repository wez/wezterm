use crate::ContentId;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Lease Expired, data is no longer accessible")]
    LeaseExpired,

    #[error("Content with id {0} not found")]
    ContentNotFound(ContentId),

    #[error("Io error in BlobLease: {0}")]
    Io(#[from] std::io::Error),

    #[error("Storage has already been initialized")]
    AlreadyInitializedStorage,

    #[error("Storage has not been initialized")]
    StorageNotInit,

    #[error("Storage location {0} may be corrupt: {1}")]
    StorageDirIoError(PathBuf, std::io::Error),
}
