//! Big-endian byte buffers as Netty's `ByteBuf` reads and writes them, with Minecraft's VarInt
//! (`VarIntUtils`) and the two string forms the formats use: a VarInt length (`ProtocolUtils`)
//! and an int length (`LegacyAnimationBinary.putString`, `CommonNetwork`).

use std::fmt;

/// A malformed buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub fn err<T>(s: impl Into<String>) -> Result<T> {
    Err(Error(s.into()))
}

/// A reader over bytes.
pub struct Reader<'a> {
    data: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, pos: 0 }
    }
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.remaining() {
            return err(format!("buffer ends early: {n} wanted, {} left", self.remaining()));
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    pub fn rest(&mut self) -> &'a [u8] {
        let s = &self.data[self.pos.min(self.data.len())..];
        self.pos = self.data.len();
        s
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn i8(&mut self) -> Result<i8> {
        Ok(self.u8()? as i8)
    }
    pub fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }
    pub fn i32(&mut self) -> Result<i32> {
        let b = self.take(4)?;
        Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn i64(&mut self) -> Result<i64> {
        let b = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(i64::from_be_bytes(a))
    }
    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.i32()? as u32))
    }
    /// `VarIntUtils.readVarInt`.
    pub fn varint(&mut self) -> Result<i32> {
        let mut value: i32 = 0;
        for i in 0..5 {
            let b = self.u8()?;
            value |= i32::from(b & 0x7F) << (7 * i);
            if b & 0x80 == 0 {
                return Ok(value);
            }
        }
        err("Bad VarInt decoded")
    }
    /// A UUID as two longs.
    pub fn uuid(&mut self) -> Result<u128> {
        let hi = self.i64()? as u64;
        let lo = self.i64()? as u64;
        Ok((u128::from(hi) << 64) | u128::from(lo))
    }
    /// `ProtocolUtils.readString`: a VarInt length, then UTF-8; a length of 0 or less is null.
    pub fn vstring(&mut self) -> Result<Option<String>> {
        let n = self.varint()?;
        if n <= 0 {
            return Ok(None);
        }
        let b = self.take(n as usize)?;
        Ok(Some(String::from_utf8_lossy(b).into_owned()))
    }
    /// `LegacyAnimationBinary.getString`: an int length, then UTF-8.
    pub fn istring(&mut self) -> Result<String> {
        let n = self.i32()?;
        if n < 0 {
            return err("negative string length");
        }
        let b = self.take(n as usize)?;
        Ok(String::from_utf8_lossy(b).into_owned())
    }
}

/// A growing writer.
#[derive(Default, Clone)]
pub struct Writer {
    pub data: Vec<u8>,
}

impl Writer {
    pub fn new() -> Writer {
        Writer { data: Vec::new() }
    }
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    pub fn u8(&mut self, v: u8) {
        self.data.push(v);
    }
    pub fn i8(&mut self, v: i8) {
        self.data.push(v as u8);
    }
    pub fn bool(&mut self, v: bool) {
        self.data.push(u8::from(v));
    }
    pub fn i32(&mut self, v: i32) {
        self.data.extend_from_slice(&v.to_be_bytes());
    }
    pub fn i64(&mut self, v: i64) {
        self.data.extend_from_slice(&v.to_be_bytes());
    }
    pub fn f32(&mut self, v: f32) {
        self.i32(v.to_bits() as i32);
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.data.extend_from_slice(b);
    }
    pub fn set_i32(&mut self, at: usize, v: i32) {
        self.data[at..at + 4].copy_from_slice(&v.to_be_bytes());
    }
    pub fn varint(&mut self, v: i32) {
        let mut v = v as u32;
        loop {
            if v & !0x7F == 0 {
                self.data.push(v as u8);
                return;
            }
            self.data.push((v & 0x7F | 0x80) as u8);
            v >>= 7;
        }
    }
    pub fn uuid(&mut self, v: u128) {
        self.i64((v >> 64) as u64 as i64);
        self.i64(v as u64 as i64);
    }
    /// `ProtocolUtils.writeString`: null as a 0 length.
    pub fn vstring(&mut self, s: Option<&str>) {
        match s {
            None => self.varint(0),
            Some(s) => {
                self.varint(s.len() as i32);
                self.bytes(s.as_bytes());
            }
        }
    }
    /// `LegacyAnimationBinary.putString`.
    pub fn istring(&mut self, s: &str) {
        self.i32(s.len() as i32);
        self.bytes(s.as_bytes());
    }
}

/// `VarIntUtils`' size of a VarInt (`OpusSound.varIntSize`).
pub fn varint_size(v: i32) -> usize {
    ((31 - ((v | 1) as u32).leading_zeros()) / 7 + 1) as usize
}

/// A UUID as Java prints it.
pub fn uuid_string(u: u128) -> String {
    let h = format!("{u:032x}");
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// `UUID.fromString`.
pub fn parse_uuid(s: &str) -> Option<u128> {
    let parts: Vec<&str> = s.trim().split('-').collect();
    if parts.len() != 5 {
        return None;
    }
    // UUID.fromString takes each part as a hex number of up to its width.
    let widths = [8usize, 4, 4, 4, 12];
    let mut out: u128 = 0;
    for (p, w) in parts.iter().zip(widths) {
        if p.is_empty() || p.len() > 16 {
            return None;
        }
        let v = u64::from_str_radix(p, 16).ok()?;
        let mask = if w == 12 { (1u64 << 48) - 1 } else { (1u64 << (w * 4)) - 1 };
        out = (out << (w * 4)) | u128::from(v & mask);
    }
    Some(out)
}

/// Base64 (RFC 4648, with padding), as `java.util.Base64`.
pub fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        })
    };
    let bytes: Vec<u8> = s.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        let mut count = 0;
        for &c in chunk {
            if c == b'=' {
                break;
            }
            n = (n << 6) | val(c)?;
            count += 1;
        }
        n <<= 6 * (4 - count);
        if count >= 2 {
            out.push((n >> 16) as u8);
        }
        if count >= 3 {
            out.push((n >> 8) as u8);
        }
        if count >= 4 {
            out.push(n as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_round_trip() {
        for v in [0, 1, 127, 128, 255, 16383, 16384, 2097151, 2097152, i32::MAX, -1, i32::MIN] {
            let mut w = Writer::new();
            w.varint(v);
            assert_eq!(w.len(), if v < 0 { 5 } else { varint_size(v) });
            assert_eq!(Reader::new(&w.data).varint().unwrap(), v);
        }
    }

    #[test]
    fn uuids_and_base64() {
        let u = parse_uuid("ebfb1e69-330a-4970-8bca-f5625c90681a").unwrap();
        assert_eq!(uuid_string(u), "ebfb1e69-330a-4970-8bca-f5625c90681a");
        let data = b"Emotecraft!";
        assert_eq!(base64_encode(data), "RW1vdGVjcmFmdCE=");
        assert_eq!(base64_decode(&base64_encode(data)).unwrap(), data);
    }
}
