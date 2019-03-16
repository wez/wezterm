//! varbincode is a binary serialization format that uses variable
//! length encoding for integer values, which typically results in
//! reduced size of the encoded data.
pub mod de;
pub mod error;
pub mod ser;
#[cfg(test)]
mod test;

/// A convenience function for serializing a value as a byte vector
/// See also `ser::Serializer`.
pub fn serialize<T: serde::Serialize>(t: &T) -> Result<Vec<u8>, error::Error> {
    let mut result = Vec::new();
    let mut s = ser::Serializer::new(&mut result);
    t.serialize(&mut s)?;
    Ok(result)
}

/// A convenience function for deserializing from a stream.
/// See also `de::Deserializer`.
pub fn deserialize<T: serde::de::DeserializeOwned, R: std::io::Read>(
    mut r: R,
) -> Result<T, error::Error> {
    let mut d = de::Deserializer::new(&mut r);
    serde::Deserialize::deserialize(&mut d)
}
