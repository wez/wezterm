//! encode and decode the frames for the mux protocol.
//! The frames include the length of a PDU as well as an identifier
//! that informs us how to decode it.  The length and ident are encoded
//! using a variable length integer encoding.
//! Rather than rely solely on serde to serialize and deserialize an
//! enum, we encode the enum variants with a version/identifier tag
//! for ourselves.  This will make it a little easier to manage
//! client and server instances that are built from different versions
//! of this code; in this way the client and server can more gracefully
//! manage unknown enum variants.
#![allow(dead_code)]

use bincode;
use failure::Error;
use varu64;

pub fn encode_raw<W: std::io::Write>(
    ident: u64,
    data: &[u8],
    mut w: W,
) -> Result<(), std::io::Error> {
    let len = data.len() + varu64::encoding_length(ident);
    varu64::encode_write(len as u64, w.by_ref())?;
    varu64::encode_write(ident, w.by_ref())?;
    w.write_all(data)
}

fn read_u64<R: std::io::Read>(mut r: R) -> Result<u64, std::io::Error> {
    let mut intbuf = [0u8; 9];
    r.read_exact(&mut intbuf[0..1])?;
    let len = match intbuf[0] {
        0...247 => 0,
        248 => 1,
        249 => 2,
        250 => 3,
        251 => 4,
        252 => 5,
        253 => 6,
        254 => 7,
        255 => 8,
        _ => unreachable!(),
    };
    if len > 0 {
        r.read_exact(&mut intbuf[1..=len])?;
    }
    let (value, _) = varu64::decode(&intbuf[0..=len])
        .map_err(|(err, _)| std::io::Error::new(std::io::ErrorKind::Other, format!("{}", err)))?;
    Ok(value)
}

pub fn decode_raw<R: std::io::Read>(mut r: R) -> Result<(u64, Vec<u8>), std::io::Error> {
    let len = read_u64(r.by_ref())? as usize;
    let ident = read_u64(r.by_ref())?;
    let data_len = len - varu64::encoding_length(ident);
    let mut data = vec![0u8; data_len];
    r.read_exact(&mut data)?;
    Ok((ident, data))
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct Ping {}
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct Pong {}

macro_rules! pdu {
    ($( $name:ident:$vers:expr),* $(,)?) => {
        #[derive(PartialEq, Debug)]
        pub enum Pdu {
            Invalid{ident: u64},
            $(
                $name($name)
            ,)*
        }

        impl Pdu {
            pub fn encode<W: std::io::Write>(&self, w: W) -> Result<(), Error> {
                match self {
                    Pdu::Invalid{..} => bail!("attempted to serialize Pdu::Invalid"),
                    $(
                        Pdu::$name(s) => {
                            let data = bincode::serialize(s)?;
                            encode_raw($vers, &data, w)?;
                            Ok(())
                        }
                    ,)*
                }
            }

            pub fn decode<R: std::io::Read>(r:R) -> Result<Pdu, Error> {
                let (ident, data) = decode_raw(r)?;
                match ident {
                    $(
                        $vers => {
                            Ok(Pdu::$name(bincode::deserialize(&data)?))
                        }
                    ,)*
                    _ => Ok(Pdu::Invalid{ident}),
                }
            }
        }
    }
}

/// Defines the Pdu enum.
/// Each struct has an explicit identifying number.
/// This allows removal of obsolete structs,
/// and defining newer structs as the protocol evolves.
pdu! {
    Ping: 1,
    Pong: 2
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_frame() {
        let mut encoded = Vec::new();
        encode_raw(0x81, b"hello", &mut encoded).unwrap();
        assert_eq!(&encoded, b"\x06\x81hello");
        let (ident, data) = decode_raw(encoded.as_slice()).unwrap();
        assert_eq!(ident, 0x81);
        assert_eq!(data, b"hello");
    }

    #[test]
    fn test_frame_lengths() {
        for target_len in &[128, 247, 256, 65536, 16777216] {
            let mut payload = Vec::with_capacity(*target_len);
            payload.resize(*target_len, b'a');
            let mut encoded = Vec::new();
            encode_raw(0x42, payload.as_slice(), &mut encoded).unwrap();
            let (ident, data) = decode_raw(encoded.as_slice()).unwrap();
            assert_eq!(ident, 0x42);
            assert_eq!(data, payload);
        }
    }

    #[test]
    fn test_pdu_ping() {
        let mut encoded = Vec::new();
        Pdu::Ping(Ping {}).encode(&mut encoded).unwrap();
        assert_eq!(&encoded, b"\x01\x01");
        assert_eq!(Pdu::Ping(Ping {}), Pdu::decode(encoded.as_slice()).unwrap());
    }

    #[test]
    fn test_pdu_ping_base91() {
        let mut encoded = Vec::new();
        {
            let mut encoder = base91::Base91Encoder::new(&mut encoded);
            Pdu::Ping(Ping {}).encode(&mut encoder).unwrap();
        }
        assert_eq!(&encoded, b";CA");
        let decoded = base91::decode(&encoded);
        assert_eq!(Pdu::Ping(Ping {}), Pdu::decode(decoded.as_slice()).unwrap());
    }

    #[test]
    fn test_pdu_pong() {
        let mut encoded = Vec::new();
        Pdu::Pong(Pong {}).encode(&mut encoded).unwrap();
        assert_eq!(&encoded, b"\x01\x02");
        assert_eq!(Pdu::Pong(Pong {}), Pdu::decode(encoded.as_slice()).unwrap());
    }

    #[test]
    fn test_bogus_pdu() {
        let mut encoded = Vec::new();
        encode_raw(0xdeadbeef, b"hello", &mut encoded).unwrap();
        assert_eq!(
            Pdu::Invalid { ident: 0xdeadbeef },
            Pdu::decode(encoded.as_slice()).unwrap()
        );
    }
}
