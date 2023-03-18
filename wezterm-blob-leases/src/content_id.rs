#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use sha2::Digest;

/// Identifies data within the store.
/// This is an (unspecified) hash of the content
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ContentId([u8; 32]);

impl ContentId {
    pub fn for_bytes(bytes: &[u8]) -> Self {
        let mut hasher = sha2::Sha256::new();
        hasher.update(bytes);
        Self(hasher.finalize().into())
    }

    pub fn as_hash_bytes(&self) -> [u8; 32] {
        self.0
    }
}

impl std::fmt::Display for ContentId {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "sha256-")?;
        for byte in &self.0 {
            write!(fmt, "{byte:x}")?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for ContentId {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "ContentId({self})")
    }
}
