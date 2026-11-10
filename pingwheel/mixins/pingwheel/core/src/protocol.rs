//! Ping Wheel's packets (`nx.pingwheel.common.network`) on the byte layout of Minecraft's
//! `FriendlyByteBuf`, so a Horizon client and server talk to the Java mod (Fabric, Forge,
//! NeoForge) and to each other:
//!
//! - `ping-wheel-c2s:ping-location`: `writeUtf(channel, 128)`, `double x, y, z`,
//!   `boolean hasEntity`, [`UUID entity`], `int sequence`, `int dimension`;
//! - `ping-wheel-c2s:update-channel`: `writeUtf(channel, 128)`;
//! - `ping-wheel-s2c:ping-location`: the C2S fields, then `UUID author`.
//!
//! Numbers are big-endian, a boolean one byte, a UUID its two longs (most significant first), a
//! string a VarInt byte length and its UTF-8. Reading is `PacketHandler.readSafe`: a packet that
//! fails to read is "corrupt" (`None` here) and what is left of the buffer is skipped.

use crate::MAX_CHANNEL_LENGTH;

pub const PING_LOCATION_C2S: &str = "ping-wheel-c2s:ping-location";
pub const UPDATE_CHANNEL_C2S: &str = "ping-wheel-c2s:update-channel";
pub const PING_LOCATION_S2C: &str = "ping-wheel-s2c:ping-location";

/// Java's `String.hashCode`: `h = 31 * h + c` over the UTF-16 code units, wrapping. The packets
/// carry the level's id (`minecraft:overworld`) as this (`GameContext.updateDimension`).
pub fn java_hash(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(i32::from(c)))
}

/// `PingLocationC2SPacket`.
#[derive(Clone, Debug, PartialEq)]
pub struct PingC2S {
    pub channel: String,
    pub pos: [f64; 3],
    pub entity: Option<u128>,
    pub sequence: i32,
    pub dimension: i32,
}

/// `PingLocationS2CPacket`: what the server relays, with who pinged.
#[derive(Clone, Debug, PartialEq)]
pub struct PingS2C {
    pub channel: String,
    pub pos: [f64; 3],
    pub entity: Option<u128>,
    pub sequence: i32,
    pub dimension: i32,
    pub author: u128,
}

impl PingS2C {
    /// `PingLocationS2CPacket.fromClientPacket`.
    pub fn from_client(p: &PingC2S, author: u128) -> Self {
        Self { channel: p.channel.clone(), pos: p.pos, entity: p.entity, sequence: p.sequence, dimension: p.dimension, author }
    }
}

/// Why a string can't be written (`EncoderException`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TooLong;

// ---- writing --------------------------------------------------------------------------------

/// `VarInt.write`.
pub fn write_var_int(out: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        if v & !0x7F == 0 {
            out.push(v as u8);
            return;
        }
        out.push((v & 0x7F) as u8 | 0x80);
        v >>= 7;
    }
}

/// `FriendlyByteBuf.writeUtf(value, maxLength)`: at most `max` UTF-16 units and `max * 3` bytes.
pub fn write_utf(out: &mut Vec<u8>, value: &str, max: usize) -> Result<(), TooLong> {
    if value.encode_utf16().count() > max || value.len() > max * 3 {
        return Err(TooLong);
    }
    write_var_int(out, value.len() as i32);
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

/// `FriendlyByteBuf.writeUUID`.
pub fn write_uuid(out: &mut Vec<u8>, uuid: u128) {
    out.extend_from_slice(&uuid.to_be_bytes());
}

fn write_location(out: &mut Vec<u8>, channel: &str, pos: [f64; 3], entity: Option<u128>, sequence: i32, dimension: i32) -> Result<(), TooLong> {
    write_utf(out, channel, MAX_CHANNEL_LENGTH)?;
    for v in pos {
        out.extend_from_slice(&v.to_be_bytes());
    }
    out.push(u8::from(entity.is_some()));
    if let Some(e) = entity {
        write_uuid(out, e);
    }
    out.extend_from_slice(&sequence.to_be_bytes());
    out.extend_from_slice(&dimension.to_be_bytes());
    Ok(())
}

impl PingC2S {
    pub fn encode(&self) -> Result<Vec<u8>, TooLong> {
        let mut out = Vec::new();
        write_location(&mut out, &self.channel, self.pos, self.entity, self.sequence, self.dimension)?;
        Ok(out)
    }
    /// `readSafe`: None for a corrupt packet.
    pub fn decode(data: &[u8]) -> Option<Self> {
        let mut r = Reader { data, at: 0 };
        let channel = r.utf(MAX_CHANNEL_LENGTH)?;
        let pos = [r.f64()?, r.f64()?, r.f64()?];
        let entity = if r.bool()? { Some(r.uuid()?) } else { None };
        Some(Self { channel, pos, entity, sequence: r.i32()?, dimension: r.i32()? })
    }
}

impl PingS2C {
    pub fn encode(&self) -> Result<Vec<u8>, TooLong> {
        let mut out = Vec::new();
        write_location(&mut out, &self.channel, self.pos, self.entity, self.sequence, self.dimension)?;
        write_uuid(&mut out, self.author);
        Ok(out)
    }
    pub fn decode(data: &[u8]) -> Option<Self> {
        let mut r = Reader { data, at: 0 };
        let channel = r.utf(MAX_CHANNEL_LENGTH)?;
        let pos = [r.f64()?, r.f64()?, r.f64()?];
        let entity = if r.bool()? { Some(r.uuid()?) } else { None };
        let (sequence, dimension) = (r.i32()?, r.i32()?);
        Some(Self { channel, pos, entity, sequence, dimension, author: r.uuid()? })
    }
}

/// `UpdateChannelC2SPacket`.
pub fn encode_update_channel(channel: &str) -> Result<Vec<u8>, TooLong> {
    let mut out = Vec::new();
    write_utf(&mut out, channel, MAX_CHANNEL_LENGTH)?;
    Ok(out)
}

pub fn decode_update_channel(data: &[u8]) -> Option<String> {
    Reader { data, at: 0 }.utf(MAX_CHANNEL_LENGTH)
}

/// The payload of `minecraft:register`: channel names, each ended by a NUL (`\0`-separated as
/// Bukkit and Fabric read it).
pub fn register_payload(channels: &[&str]) -> Vec<u8> {
    channels.join("\0").into_bytes()
}

/// The channels in a `minecraft:register` payload.
pub fn register_channels(data: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(data).split('\0').filter(|s| !s.is_empty()).map(str::to_owned).collect()
}

// ---- reading --------------------------------------------------------------------------------

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let end = self.at.checked_add(n)?;
        let s = self.data.get(self.at..end)?;
        self.at = end;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    /// `readBoolean`: any byte but 0 is true.
    fn bool(&mut self) -> Option<bool> {
        self.u8().map(|b| b != 0)
    }
    fn i32(&mut self) -> Option<i32> {
        self.take(4).map(|b| i32::from_be_bytes(b.try_into().expect("4")))
    }
    fn f64(&mut self) -> Option<f64> {
        self.take(8).map(|b| f64::from_be_bytes(b.try_into().expect("8")))
    }
    fn uuid(&mut self) -> Option<u128> {
        self.take(16).map(|b| u128::from_be_bytes(b.try_into().expect("16")))
    }
    /// `VarInt.read`: at most 5 bytes.
    fn var_int(&mut self) -> Option<i32> {
        let mut value: u32 = 0;
        for i in 0..5 {
            let b = self.u8()?;
            value |= u32::from(b & 0x7F) << (7 * i);
            if b & 0x80 == 0 {
                return Some(value as i32);
            }
        }
        None
    }
    /// `Utf8String.read(buf, maxLength)`: the byte length at most `max * 3`, not negative, all
    /// there, and the string (bad UTF-8 replaced, as Java decodes it) at most `max` UTF-16 units.
    fn utf(&mut self, max: usize) -> Option<String> {
        let len = self.var_int()?;
        if len < 0 || len as usize > max * 3 {
            return None;
        }
        let s = String::from_utf8_lossy(self.take(len as usize)?).into_owned();
        (s.encode_utf16().count() <= max).then_some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A buffer as Java's `FriendlyByteBuf` writes it, field by field, by hand.
    fn java_c2s() -> Vec<u8> {
        let mut b = vec![
            0x05, b'r', b'e', b'd', b'-', b'1', // writeUtf("red-1", 128): VarInt 5, UTF-8
        ];
        b.extend_from_slice(&[0x40, 0x59, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // writeDouble(100.0)
        b.extend_from_slice(&[0xC0, 0x50, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00]); // writeDouble(-65.0)
        b.extend_from_slice(&[0x3F, 0xE0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // writeDouble(0.5)
        b.push(0x01); // writeBoolean(true)
        // writeUUID(123e4567-e89b-12d3-a456-426614174000): two longs, most significant first.
        b.extend_from_slice(&[0x12, 0x3E, 0x45, 0x67, 0xE8, 0x9B, 0x12, 0xD3]);
        b.extend_from_slice(&[0xA4, 0x56, 0x42, 0x66, 0x14, 0x17, 0x40, 0x00]);
        b.extend_from_slice(&[0x00, 0x00, 0x00, 0x07]); // writeInt(7)
        b.extend_from_slice(&[0x41, 0xD0, 0xE9, 0xB1]); // writeInt("minecraft:overworld".hashCode())
        b
    }

    const UUID: u128 = 0x123e4567_e89b_12d3_a456_426614174000;
    const AUTHOR: u128 = 0x069a79f4_44e9_4726_a5be_fca90e38aaf5;

    #[test]
    fn java_string_hash_codes() {
        // Values Java's String.hashCode gives.
        assert_eq!(java_hash("minecraft:overworld"), 1_104_210_353);
        assert_eq!(java_hash("minecraft:the_nether"), -1_526_768_685);
        assert_eq!(java_hash("minecraft:the_end"), 1_731_133_248);
        assert_eq!(java_hash("hello"), 99_162_322);
        assert_eq!(java_hash("Aa"), java_hash("BB"));
        assert_eq!(java_hash("Aa"), 2112);
        assert_eq!(java_hash(""), 0);
        // UTF-16 units: a surrogate pair counts as two.
        assert_eq!(java_hash("\u{1F600}"), 1_772_899);
        assert_eq!(1_104_210_353i32.to_be_bytes(), [0x41, 0xD0, 0xE9, 0xB1]);
    }

    #[test]
    fn c2s_matches_java_layout() {
        let p = PingC2S { channel: "red-1".into(), pos: [100.0, -65.0, 0.5], entity: Some(UUID), sequence: 7, dimension: java_hash("minecraft:overworld") };
        assert_eq!(p.encode().unwrap(), java_c2s());
        assert_eq!(PingC2S::decode(&java_c2s()), Some(p));
    }

    #[test]
    fn c2s_without_entity() {
        let mut b = vec![0x00]; // writeUtf(""): the default channel
        b.extend_from_slice(&1.25f64.to_be_bytes());
        b.extend_from_slice(&64.0f64.to_be_bytes());
        b.extend_from_slice(&(-3.5f64).to_be_bytes());
        b.push(0x00); // no entity: no UUID follows
        b.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]); // writeInt(-1)
        b.extend_from_slice(&[0xA4, 0xFF, 0x5B, 0xD3]); // the_nether's hash
        let p = PingC2S { channel: String::new(), pos: [1.25, 64.0, -3.5], entity: None, sequence: -1, dimension: java_hash("minecraft:the_nether") };
        assert_eq!(p.encode().unwrap(), b);
        assert_eq!(PingC2S::decode(&b), Some(p));
    }

    #[test]
    fn s2c_matches_java_layout() {
        let mut b = java_c2s();
        b.extend_from_slice(&[0x06, 0x9A, 0x79, 0xF4, 0x44, 0xE9, 0x47, 0x26, 0xA5, 0xBE, 0xFC, 0xA9, 0x0E, 0x38, 0xAA, 0xF5]);
        let c2s = PingC2S::decode(&java_c2s()).unwrap();
        let s2c = PingS2C::from_client(&c2s, AUTHOR);
        assert_eq!(s2c.encode().unwrap(), b);
        assert_eq!(PingS2C::decode(&b), Some(s2c));
    }

    #[test]
    fn update_channel_layout() {
        assert_eq!(encode_update_channel("").unwrap(), vec![0x00]);
        assert_eq!(encode_update_channel("abc").unwrap(), vec![0x03, b'a', b'b', b'c']);
        // UTF-8 bytes counted in the VarInt: "ключ" is 4 chars, 8 bytes.
        assert_eq!(encode_update_channel("ключ").unwrap(), [&[0x08][..], "ключ".as_bytes()].concat());
        assert_eq!(decode_update_channel(&[0x03, b'a', b'b', b'c']), Some("abc".into()));
        // A 200-byte channel needs a two-byte VarInt (0xC8 0x01).
        let long = "é".repeat(100);
        let enc = encode_update_channel(&long).unwrap();
        assert_eq!(&enc[..2], &[0xC8, 0x01]);
        assert_eq!(decode_update_channel(&enc), Some(long));
    }

    #[test]
    fn channel_limits_are_javas() {
        // 128 characters is the most.
        assert!(encode_update_channel(&"a".repeat(128)).is_ok());
        assert_eq!(encode_update_channel(&"a".repeat(129)), Err(TooLong));
        // A reader refuses more than 128 * 3 bytes...
        let mut b = Vec::new();
        write_var_int(&mut b, 385);
        b.extend(std::iter::repeat_n(b'a', 385));
        assert_eq!(decode_update_channel(&b), None);
        // ...and more than 128 characters even when the bytes fit.
        let mut b = Vec::new();
        write_var_int(&mut b, 129);
        b.extend(std::iter::repeat_n(b'a', 129));
        assert_eq!(decode_update_channel(&b), None);
    }

    #[test]
    fn corrupt_packets_read_as_none() {
        let full = java_c2s();
        for cut in 0..full.len() {
            assert_eq!(PingC2S::decode(&full[..cut]), None, "cut at {cut}");
        }
        // Trailing bytes are skipped (readSafe's finally), not refused.
        let mut more = full.clone();
        more.extend_from_slice(&[1, 2, 3]);
        assert!(PingC2S::decode(&more).is_some());
        // A negative VarInt length.
        assert_eq!(decode_update_channel(&[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]), None);
        // An S2C without its author.
        assert_eq!(PingS2C::decode(&full), None);
    }

    #[test]
    fn var_ints() {
        let enc = |v: i32| {
            let mut b = Vec::new();
            write_var_int(&mut b, v);
            b
        };
        assert_eq!(enc(0), [0x00]);
        assert_eq!(enc(127), [0x7F]);
        assert_eq!(enc(128), [0x80, 0x01]);
        assert_eq!(enc(255), [0xFF, 0x01]);
        assert_eq!(enc(-1), [0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
    }

    #[test]
    fn register_payloads() {
        assert_eq!(register_payload(&[PING_LOCATION_C2S, UPDATE_CHANNEL_C2S]), b"ping-wheel-c2s:ping-location\0ping-wheel-c2s:update-channel".to_vec());
        assert_eq!(register_channels(b"a:b\0c:d\0"), vec!["a:b".to_owned(), "c:d".to_owned()]);
    }
}
