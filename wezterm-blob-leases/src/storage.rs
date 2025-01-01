use crate::{ContentId, Error, LeaseId};
use std::io::{BufRead, Seek};
use std::sync::{Arc, Mutex};

static STORAGE: Mutex<Option<Arc<dyn BlobStorage + Send + Sync + 'static>>> = Mutex::new(None);

pub trait BufSeekRead: BufRead + Seek {}
pub type BoxedReader = Box<dyn BufSeekRead + Send + Sync>;

/// Implements the actual storage mechanism for blobs
pub trait BlobStorage {
    /// Store data with the provided content_id.
    /// lease_id is provided by the caller to identify this store.
    /// The underlying store is expected to dedup storing data with the same
    /// content_id.
    fn store(&self, content_id: ContentId, data: &[u8], lease_id: LeaseId) -> Result<(), Error>;

    /// Resolve the data associated with content_id.
    /// If found, establish a lease with the given lease_id.
    /// If not found, returns Err(Error::ContentNotFound)
    fn lease_by_content(&self, content_id: ContentId, lease_id: LeaseId) -> Result<(), Error>;

    /// Retrieves the data identified by content_id.
    /// lease_id is provided in order to advise the storage system
    /// which lease fetched it, so that it can choose to record that
    /// information to track the liveness of a lease
    fn get_data(&self, content_id: ContentId, lease_id: LeaseId) -> Result<Vec<u8>, Error>;

    /// Retrieves the data identified by content_id as a readable+seekable
    /// buffered handle.
    ///
    /// lease_id is provided in order to advise the storage system
    /// which lease fetched it, so that it can choose to record that
    /// information to track the liveness of a lease.
    ///
    /// The returned handle serves to extend the lifetime of the lease.
    fn get_reader(&self, content_id: ContentId, lease_id: LeaseId) -> Result<BoxedReader, Error>;

    /// Advises the storage manager that a particular lease has been dropped.
    fn advise_lease_dropped(&self, lease_id: LeaseId, content_id: ContentId) -> Result<(), Error>;
    /// Advises the storage manager that a given process id is now, or
    /// continues to be, alive and a valid consumer of the store.
    fn advise_of_pid(&self, pid: u32) -> Result<(), Error>;

    /// Advises the storage manager that a given process id is, or will
    /// very shortly, terminate and will cease to be a valid consumer
    /// of the store.
    /// It may choose to do something to invalidate all leases with
    /// a corresponding pid.
    fn advise_pid_terminated(&self, pid: u32) -> Result<(), Error>;
}

pub fn register_storage(
    storage: Arc<dyn BlobStorage + Send + Sync + 'static>,
) -> Result<(), Error> {
    STORAGE.lock().unwrap().replace(storage);
    Ok(())
}

pub fn get_storage() -> Result<Arc<dyn BlobStorage + Send + Sync + 'static>, Error> {
    STORAGE
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| s.clone())
        .ok_or_else(|| Error::StorageNotInit)
}

pub fn clear_storage() {
    STORAGE.lock().unwrap().take();
}
