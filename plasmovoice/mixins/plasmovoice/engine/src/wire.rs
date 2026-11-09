//! Java's `DataOutput`/`DataInput` as Plasmo Voice writes its packets (Guava's
//! `ByteArrayDataOutput`): big-endian numbers, `writeUTF` strings (a u16 byte length and
//! *modified* UTF-8), UUIDs as two longs (`PacketUtil.writeUUID`).
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use std::fmt;

/// A UUID as Java keeps it: the most and least significant longs.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Uuid(pub u128);

impl Uuid {
    pub const NIL: Uuid = Uuid(0);
    pub fn from_halves(msb: i64, lsb: i64) -> Uuid {
        Uuid(((msb as u64 as u128) << 64) | lsb as u64 as u128)
    }
    pub fn msb(self) -> i64 {
        (self.0 >> 64) as u64 as i64
    }
    pub fn lsb(self) -> i64 {
        self.0 as u64 as i64
    }
    pub fn to_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }
    pub fn from_bytes(b: [u8; 16]) -> Uuid {
        Uuid(u128::from_be_bytes(b))
    }
}

impl fmt::Display for Uuid {
    /// `UUID.toString`: 8-4-4-4-12 hex digits.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let h = format!("{:032x}", self.0);
        write!(f, "{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
    }
}

impl fmt::Debug for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// What can go wrong reading a packet (`IOException` in Java).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The buffer ended before the field did.
    Eof,
    /// A length or count outside the bounds Plasmo Voice allows (`readSafeInt`, `readSafeUTF`).
    Bounds(&'static str),
    /// A string that isn't modified UTF-8.
    Utf,
    /// An unknown packet type, enum name or magic number.
    Unknown(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Eof => write!(f, "unexpected end of packet"),
            Error::Bounds(what) => write!(f, "{what} out of bounds"),
            Error::Utf => write!(f, "malformed string"),
            Error::Unknown(what) => write!(f, "unknown {what}"),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// `ByteArrayDataOutput`.
#[derive(Clone, Debug, Default)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Writer {
        Writer::default()
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn i8(&mut self, v: i8) {
        self.buf.push(v as u8);
    }
    pub fn bool(&mut self, v: bool) {
        self.buf.push(u8::from(v));
    }
    pub fn i16(&mut self, v: i16) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    pub fn i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    pub fn f64(&mut self, v: f64) {
        self.buf.extend_from_slice(&v.to_bits().to_be_bytes());
    }
    pub fn raw(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }
    /// `PacketUtil.writeUUID`.
    pub fn uuid(&mut self, u: Uuid) {
        self.i64(u.msb());
        self.i64(u.lsb());
    }
    /// `PacketUtil.writeBytes`: an int length, then the bytes.
    pub fn bytes(&mut self, b: &[u8]) {
        self.i32(b.len() as i32);
        self.raw(b);
    }
    /// `DataOutput.writeUTF`: modified UTF-8 behind a u16 length. Java throws past 65535
    /// bytes; this cuts the string there instead.
    pub fn utf(&mut self, s: &str) {
        let mut enc = modified_utf8(s);
        enc.truncate(u16::MAX as usize);
        self.u16(enc.len() as u16);
        self.raw(&enc);
    }
    /// `PacketUtil.writeNullableString`.
    pub fn nullable_utf(&mut self, s: Option<&str>) {
        self.bool(s.is_some());
        if let Some(s) = s {
            self.utf(s);
        }
    }
}

/// Java's modified UTF-8: each UTF-16 unit on its own, NUL as two bytes.
pub fn modified_utf8(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for c in s.encode_utf16() {
        match c {
            0x0001..=0x007F => out.push(c as u8),
            0x0000 | 0x0080..=0x07FF => {
                out.push(0xC0 | (c >> 6) as u8);
                out.push(0x80 | (c & 0x3F) as u8);
            }
            _ => {
                out.push(0xE0 | (c >> 12) as u8);
                out.push(0x80 | ((c >> 6) & 0x3F) as u8);
                out.push(0x80 | (c & 0x3F) as u8);
            }
        }
    }
    out
}

/// `ByteArrayDataInput`.
#[derive(Clone, Debug)]
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Reader<'a> {
        Reader { buf, pos: 0 }
    }
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }
    pub fn rest(&mut self) -> &'a [u8] {
        let r = &self.buf[self.pos..];
        self.pos = self.buf.len();
        r
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.remaining() < n {
            return Err(Error::Eof);
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut a = [0; N];
        a.copy_from_slice(self.take(N)?);
        Ok(a)
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn i8(&mut self) -> Result<i8> {
        Ok(self.u8()? as i8)
    }
    /// `readBoolean`: any non-zero byte is true.
    pub fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }
    pub fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_be_bytes(self.array()?))
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.array()?))
    }
    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.array()?))
    }
    pub fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.array()?))
    }
    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_bits(u64::from_be_bytes(self.array()?)))
    }
    pub fn uuid(&mut self) -> Result<Uuid> {
        let msb = self.i64()?;
        let lsb = self.i64()?;
        Ok(Uuid::from_halves(msb, lsb))
    }
    /// `PacketUtil.readSafeInt`.
    pub fn safe_i32(&mut self, min: i32, max: i32, what: &'static str) -> Result<i32> {
        let v = self.i32()?;
        if v < min || v > max {
            return Err(Error::Bounds(what));
        }
        Ok(v)
    }
    /// `PacketUtil.readBytes`.
    pub fn bytes(&mut self, min: i32, max: i32, what: &'static str) -> Result<Vec<u8>> {
        let n = self.safe_i32(min, max, what)? as usize;
        Ok(self.take(n)?.to_vec())
    }
    pub fn raw(&mut self, n: usize) -> Result<&'a [u8]> {
        self.take(n)
    }
    /// `DataInput.readUTF`.
    pub fn utf(&mut self) -> Result<String> {
        let n = self.u16()? as usize;
        decode_modified_utf8(self.take(n)?)
    }
    /// `PacketUtil.readSafeUTF`: at most `max` UTF-16 units.
    pub fn safe_utf(&mut self, max: usize, what: &'static str) -> Result<String> {
        let s = self.utf()?;
        if s.encode_utf16().count() > max {
            return Err(Error::Bounds(what));
        }
        Ok(s)
    }
    pub fn nullable_utf(&mut self) -> Result<Option<String>> {
        if self.bool()? { Ok(Some(self.utf()?)) } else { Ok(None) }
    }
}

/// Modified UTF-8 back to a string (lone surrogates become U+FFFD).
pub fn decode_modified_utf8(b: &[u8]) -> Result<String> {
    let mut units = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c & 0x80 == 0 {
            units.push(u16::from(c));
            i += 1;
        } else if c & 0xE0 == 0xC0 {
            let c2 = *b.get(i + 1).ok_or(Error::Utf)?;
            if c2 & 0xC0 != 0x80 {
                return Err(Error::Utf);
            }
            units.push((u16::from(c & 0x1F) << 6) | u16::from(c2 & 0x3F));
            i += 2;
        } else if c & 0xF0 == 0xE0 {
            let (c2, c3) = (*b.get(i + 1).ok_or(Error::Utf)?, *b.get(i + 2).ok_or(Error::Utf)?);
            if c2 & 0xC0 != 0x80 || c3 & 0xC0 != 0x80 {
                return Err(Error::Utf);
            }
            units.push((u16::from(c & 0x0F) << 12) | (u16::from(c2 & 0x3F) << 6) | u16::from(c3 & 0x3F));
            i += 3;
        } else {
            return Err(Error::Utf);
        }
    }
    Ok(String::from_utf16_lossy(&units))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf_matches_java() {
        // Java: new DataOutputStream(...).writeUTF("a\u0000é€😀")
        let mut w = Writer::new();
        w.utf("a\u{0}é€😀");
        assert_eq!(
            w.buf,
            [0, 14, b'a', 0xC0, 0x80, 0xC3, 0xA9, 0xE2, 0x82, 0xAC, 0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]
        );
        let mut r = Reader::new(&w.buf);
        assert_eq!(r.utf().unwrap(), "a\u{0}é€😀");
    }

    #[test]
    fn uuid_round_trip_and_text() {
        let u = Uuid::from_halves(-0x0123_4567_89ab_cdef, 0x7edc_ba98_7654_3210);
        let mut w = Writer::new();
        w.uuid(u);
        assert_eq!(Reader::new(&w.buf).uuid().unwrap(), u);
        assert_eq!(Uuid(0x0011_2233_4455_6677_8899_aabb_ccdd_eeff).to_string(), "00112233-4455-6677-8899-aabbccddeeff");
    }

    #[test]
    fn numbers_are_big_endian() {
        let mut w = Writer::new();
        w.i32(0x0102_0304);
        w.i16(-2);
        w.f64(1.5);
        assert_eq!(w.buf, [1, 2, 3, 4, 0xFF, 0xFE, 0x3F, 0xF8, 0, 0, 0, 0, 0, 0]);
    }
}
