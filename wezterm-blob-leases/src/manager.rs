use crate::{get_storage, BlobLease, ContentId, Error, LeaseId};

pub struct BlobManager {}

impl BlobManager {
    /// Store data into the store, de-duplicating it and returning
    /// a BlobLease that can be used to reference and access it.
    pub fn store(data: &[u8]) -> Result<BlobLease, Error> {
        let storage = get_storage()?;

        let lease_id = LeaseId::new();
        let content_id = ContentId::for_bytes(data);

        storage.store(content_id, data, lease_id)?;

        Ok(BlobLease::make_lease(content_id, lease_id))
    }

    /// Attempt to resolve by content id
    pub fn get_by_content_id(content_id: ContentId) -> Result<BlobLease, Error> {
        let storage = get_storage()?;

        let lease_id = LeaseId::new();
        storage.lease_by_content(content_id, lease_id)?;

        Ok(BlobLease::make_lease(content_id, lease_id))
    }
}
