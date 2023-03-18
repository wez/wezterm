use crate::{get_storage, BoxedReader, ContentId, Error, LeaseId};
use std::sync::Arc;

/// A lease represents a handle to data in the store.
/// The lease will help to keep the data alive in the store.
/// Depending on the policy configured for the store, it
/// may guarantee to keep the data intact for its lifetime,
/// or in some cases, it the store is being thrashed and at
/// capacity, it may have been evicted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobLease {
    inner: Arc<LeaseInner>,
}

#[derive(Debug, PartialEq, Eq)]
struct LeaseInner {
    pub content_id: ContentId,
    pub lease_id: LeaseId,
}

impl BlobLease {
    pub(crate) fn make_lease(content_id: ContentId, lease_id: LeaseId) -> Self {
        Self {
            inner: Arc::new(LeaseInner {
                content_id,
                lease_id,
            }),
        }
    }

    /// Returns a copy of the data, owned by the caller
    pub fn get_data(&self) -> Result<Vec<u8>, Error> {
        let storage = get_storage()?;
        storage.get_data(self.inner.content_id, self.inner.lease_id)
    }

    /// Returns a reader that can be used to stream/seek into
    /// the data
    pub fn get_reader(&self) -> Result<BoxedReader, Error> {
        let storage = get_storage()?;
        storage.get_reader(self.inner.content_id, self.inner.lease_id)
    }

    pub fn content_id(&self) -> ContentId {
        self.inner.content_id
    }
}

impl Drop for LeaseInner {
    fn drop(&mut self) {
        if let Ok(storage) = get_storage() {
            storage
                .advise_lease_dropped(self.lease_id, self.content_id)
                .ok();
        }
    }
}

/// Serialize a lease as the corresponding data bytes.
/// This can fail during serialization if the lease is
/// stale, but not during deserialization, as deserialiation
/// will store the data implicitly.
#[cfg(feature = "serde")]
pub mod lease_bytes {
    use super::*;
    use crate::BlobManager;
    use serde::{de, ser, Deserialize, Serialize};

    /// Serialize a lease as its bytes
    pub fn serialize<S>(lease: &BlobLease, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let data = lease
            .get_data()
            .map_err(|err| ser::Error::custom(format!("{err:#}")))?;
        data.serialize(serializer)
    }

    /// Deserialize a lease from bytes.
    pub fn deserialize<'de, D>(d: D) -> Result<BlobLease, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let data = <Vec<u8> as Deserialize>::deserialize(d)?;

        BlobManager::store(&data).map_err(|err| de::Error::custom(format!("{err:#}")))
    }
}

/// Serialize a lease to/from its content id.
/// This can fail in either direction if the lease is stale
/// during serialization, or if the data for that content id
/// is not available during deserialization.
#[cfg(feature = "serde")]
pub mod lease_content_id {

    use super::*;
    use crate::BlobManager;
    use serde::{de, ser, Deserialize, Serialize};

    /// Serialize a lease as its content id
    pub fn serialize<S>(lease: &BlobLease, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        lease.inner.content_id.serialize(serializer)
    }

    /// Deserialize a lease from a content id.
    /// Will fail unless the content id is already available
    /// to the local storage manager
    pub fn deserialize<'de, D>(d: D) -> Result<BlobLease, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let content_id = <ContentId as Deserialize>::deserialize(d)?;
        BlobManager::get_by_content_id(content_id)
            .map_err(|err| de::Error::custom(format!("{err:#}")))
    }
}
