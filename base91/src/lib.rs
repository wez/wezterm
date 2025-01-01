//! Implements basE91 encoding; see http://base91.sourceforge.net/
//! basE91 is an advanced method for encoding binary data as ASCII characters. It is similar to
//! UUencode or base64, but is more efficient. The overhead produced by basE91 depends on the input
//! data. It amounts at most to 23% (versus 33% for base64) and can range down to 14%, which
//! typically occurs on 0-byte blocks. This makes basE91 very useful for transferring larger files
//! over binary unsafe connections like e-mail or terminal lines.

// This Rust implementation was made by Wez Furlong based on C code that is:
// Copyright (c) 2000-2006 Joachim Henke
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
//  - Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//  - Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//  - Neither the name of Joachim Henke nor the names of his contributors may
//    be used to endorse or promote products derived from this software without
//    specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.

use std::io::Write;

const ENCTAB: [u8; 91] = *b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!#$%&()*+,./:;<=>?@[]^_`{|}~\"";

/// An invalid mapping; used to represent positions in DECTAB that have no valid
/// representation in the original input data.  These are skipped; this accomodates
/// breaking the data in eg: whitespace separated lines.
const INV: u8 = 91;
const DECTAB: [u8; 256] = [
    INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV,
    INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, 62, 90, 63, 64, 65, 66,
    INV, 67, 68, 69, 70, 71, INV, 72, 73, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 74, 75, 76, 77,
    78, 79, 80, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
    23, 24, 25, 81, INV, 82, 83, 84, 85, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39,
    40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 86, 87, 88, 89, INV, INV, INV, INV, INV, INV,
    INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV,
    INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV,
    INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV,
    INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV,
    INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV,
    INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV, INV,
    INV, INV, INV, INV, INV, INV, INV, INV, INV,
];

/// `Base91Encoder` wraps an impl of `std::io::Write` and does itself impl `std::io::Write`,
/// and performs a base91 encode operation on the bytes that are written to it.
/// It is important to remember to `flush` the writer at end of the data, as the encoder
/// maintains up to 2 bytes of pending data; the Drop impl will implicitly flush on
/// your behalf, but will mask any error that may occur during the flush.
pub struct Base91Encoder<'a> {
    writer: &'a mut dyn Write,
    accumulator: u64,
    bits: u32,
}

impl<'a> Base91Encoder<'a> {
    /// Construct a Base91Encoder that writes encoded data to the supplied writer
    pub fn new(writer: &'a mut dyn Write) -> Self {
        Self {
            writer,
            accumulator: 0,
            bits: 0,
        }
    }
}

impl<'a> Drop for Base91Encoder<'a> {
    fn drop(&mut self) {
        self.flush().ok();
    }
}

impl<'a> std::io::Write for Base91Encoder<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        for b in buf {
            self.accumulator |= u64::from(*b) << self.bits;
            self.bits += 8;

            if self.bits > 13 {
                let val = self.accumulator & 8191;

                let val = if val > 88 {
                    self.accumulator >>= 13;
                    self.bits -= 13;
                    val as usize
                } else {
                    // We can take 14 bits
                    let val = self.accumulator & 16383;
                    self.accumulator >>= 14;
                    self.bits -= 14;
                    val as usize
                };

                let out: [u8; 2] = [ENCTAB[val % 91], ENCTAB[val / 91]];
                self.writer.write_all(&out)?;
            }
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.bits > 0 {
            let val = self.accumulator as usize;
            if self.bits > 7 || self.accumulator > 90 {
                let out: [u8; 2] = [ENCTAB[val % 91], ENCTAB[val / 91]];
                self.writer.write_all(&out)?;
            } else {
                let out: [u8; 1] = [ENCTAB[val % 91]];
                self.writer.write_all(&out)?;
            }
        }
        self.bits = 0;
        self.accumulator = 0;
        self.writer.flush()
    }
}

/// A convenience function that wraps Base91Encoder; it encodes a slice of data
/// and returns a vector holding the base91 encoded data.
pub fn encode(buf: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity((buf.len() * 123) / 100);
    {
        let mut writer = Base91Encoder::new(&mut result);
        writer.write_all(buf).unwrap();
        writer.flush().unwrap();
    }
    result
}

/// `Base91Decoder` wraps an impl of `std::io::Write` and does itself impl `std::io::Write`,
/// and performs a base91 decode operation on the bytes that are written to it.
/// It is important to remember to `flush` the writer at end of the data, as the encoder
/// maintains up to 1 byte of pending data; the Drop impl will implicitly flush on
/// your behalf, but will mask any error that may occur during the flush.
pub struct Base91Decoder<'a> {
    writer: &'a mut dyn Write,
    accumulator: u64,
    bits: u32,
    value: Option<u8>,
}

impl<'a> Base91Decoder<'a> {
    pub fn new(writer: &'a mut dyn Write) -> Self {
        Self {
            writer,
            accumulator: 0,
            bits: 0,
            value: None,
        }
    }
}

impl<'a> Drop for Base91Decoder<'a> {
    fn drop(&mut self) {
        self.flush().ok();
    }
}

impl<'a> std::io::Write for Base91Decoder<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        for b in buf {
            let d = DECTAB[usize::from(*b)];

            if d == INV {
                // non-alphabet; skip
                continue;
            }

            if let Some(value) = self.value.take() {
                let value = (value as u64) + (d as u64) * 91;
                self.accumulator |= value << self.bits;
                self.bits += if (value & 8191) > 88 { 13 } else { 14 };

                loop {
                    let out: [u8; 1] = [(self.accumulator & 0xff) as u8];
                    self.writer.write_all(&out)?;
                    self.accumulator >>= 8;
                    self.bits -= 8;

                    if self.bits < 8 {
                        break;
                    }
                }
            } else {
                // Starting next value
                self.value = Some(d);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(value) = self.value.take() {
            let out: [u8; 1] = [(self.accumulator & 0xff) as u8 | (value << self.bits)];
            self.writer.write_all(&out)?;
        }
        self.bits = 0;
        self.accumulator = 0;
        self.writer.flush()
    }
}

/// A convenience function that wraps Base91Decoder; it decodes a slice of data
/// and returns a vector holding the unencoded binary data.
pub fn decode(buf: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(buf.len());
    {
        let mut writer = Base91Decoder::new(&mut result);
        writer.write_all(buf).unwrap();
        writer.flush().unwrap();
    }
    result
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test() {
        assert_eq!(encode(b"hello\n"), b"TPwJh>UA");
        assert_eq!(decode(b"TPwJh>UA"), b"hello\n");
    }

    #[test]
    fn test_bin() {
        for reps in 0..=4 {
            let mut bin = Vec::with_capacity(256);
            for i in 0..=255u8 {
                for _ in 0..reps {
                    bin.push(i);
                }
            }

            let encoded = encode(&bin);
            eprintln!("encoded as {}", String::from_utf8(encoded.clone()).unwrap());
            let decoded = decode(&encoded);

            assert_eq!(decoded, bin);
        }
    }
}
