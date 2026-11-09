//! Plasmo Voice's UDP packets (`PacketUdpCodec`): `i32 magic | u8 type | uuid secret |
//! i64 timestamp | body`. The audio in them is AES-encrypted Opus (`crypto`).
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use crate::wire::{Error, Reader, Result, Uuid, Writer};

/// `PacketUdpCodec.MAGIC_NUMBER`.
pub const MAGIC: i32 = 0x4e90_04e9;

/// The UDP packets, by type (1 to 4; the addons' 0x100 can't be written in a byte, so it never travels).
#[derive(Clone, Debug, PartialEq)]
pub enum Packet {
    /// 1, both ways. The client adds the Minecraft server's address while it isn't connected yet.
    Ping { time: i64, server: Option<(String, u16)> },
    /// 2 C→S: one frame of the player's voice for an activation.
    PlayerAudio { sequence: i64, data: Vec<u8>, activation: Uuid, distance: i16, stereo: bool },
    /// 3 S→C: one frame of a source's audio.
    SourceAudio { sequence: i64, data: Vec<u8>, source: Uuid, state: i8, distance: i16 },
    /// 4 S→C: the server's copy of what the player itself sent (for the "self" source).
    SelfAudioInfo { source: Uuid, sequence: i64, data: Option<Vec<u8>>, distance: i16 },
}

/// A decoded datagram.
#[derive(Clone, Debug, PartialEq)]
pub struct Datagram {
    pub secret: Uuid,
    pub timestamp: i64,
    pub packet: Packet,
}

impl Packet {
    pub fn kind(&self) -> u8 {
        match self {
            Packet::Ping { .. } => 1,
            Packet::PlayerAudio { .. } => 2,
            Packet::SourceAudio { .. } => 3,
            Packet::SelfAudioInfo { .. } => 4,
        }
    }

    /// `PacketUdpCodec.encode`, stamped with `timestamp` (`System.currentTimeMillis`).
    pub fn encode(&self, secret: Uuid, timestamp: i64) -> Vec<u8> {
        let mut w = Writer::new();
        w.i32(MAGIC);
        w.u8(self.kind());
        w.uuid(secret);
        w.i64(timestamp);
        match self {
            Packet::Ping { time, server } => {
                w.i64(*time);
                if let Some((ip, port)) = server
                    && *port > 0
                {
                    w.utf(ip);
                    w.u16(*port);
                }
            }
            Packet::PlayerAudio { sequence, data, activation, distance, stereo } => {
                w.i64(*sequence);
                w.bytes(data);
                w.uuid(*activation);
                w.i16(*distance);
                w.bool(*stereo);
            }
            Packet::SourceAudio { sequence, data, source, state, distance } => {
                w.i64(*sequence);
                w.bytes(data);
                w.uuid(*source);
                w.i8(*state);
                w.i16(*distance);
            }
            Packet::SelfAudioInfo { source, sequence, data, distance } => {
                w.uuid(*source);
                w.i64(*sequence);
                w.bool(data.is_some());
                if let Some(d) = data {
                    w.bytes(d);
                }
                w.i16(*distance);
            }
        }
        w.into_bytes()
    }

    /// `PacketUdpCodec.decode`: none for a datagram that isn't Plasmo Voice's.
    pub fn decode(bytes: &[u8]) -> Result<Option<Datagram>> {
        let mut r = Reader::new(bytes);
        if r.i32().ok() != Some(MAGIC) {
            return Ok(None);
        }
        let kind = r.u8()?;
        let secret = r.uuid()?;
        let timestamp = r.i64()?;
        let r = &mut r;
        let packet = match kind {
            1 => {
                let time = r.i64()?;
                // Optional: whatever fails to read is left out, as the original ignores it.
                let mut rest = r.clone();
                let server = (|| -> Result<(String, u16)> { Ok((rest.safe_utf(255, "server ip")?, rest.u16()?)) })().ok();
                Packet::Ping { time, server }
            }
            2 => Packet::PlayerAudio {
                sequence: r.i64()?,
                data: r.bytes(1, 2048, "audio")?,
                activation: r.uuid()?,
                distance: r.i16()?,
                stereo: r.bool()?,
            },
            3 => Packet::SourceAudio {
                sequence: r.i64()?,
                data: r.bytes(1, 2048, "audio")?,
                source: r.uuid()?,
                state: r.i8()?,
                distance: r.i16()?,
            },
            4 => Packet::SelfAudioInfo {
                source: r.uuid()?,
                sequence: r.i64()?,
                data: if r.bool()? { Some(r.bytes(1, 2048, "audio")?) } else { None },
                distance: r.i16()?,
            },
            _ => return Err(Error::Unknown("udp packet type")),
        };
        Ok(Some(Datagram { secret, timestamp, packet }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_layout() {
        let b = Packet::Ping { time: 5, server: None }.encode(Uuid(0x11), 1000);
        assert_eq!(&b[..4], &[0x4e, 0x90, 0x04, 0xe9]);
        assert_eq!(b[4], 1);
        assert_eq!(b.len(), 4 + 1 + 16 + 8 + 8);
    }

    #[test]
    fn round_trips() {
        for p in [
            Packet::Ping { time: 5, server: Some(("play.example.net".into(), 25565)) },
            Packet::Ping { time: 6, server: None },
            Packet::PlayerAudio { sequence: 1, data: vec![1, 2, 3], activation: Uuid(4), distance: 16, stereo: false },
            Packet::SourceAudio { sequence: 2, data: vec![9; 40], source: Uuid(8), state: 1, distance: 32 },
            Packet::SelfAudioInfo { source: Uuid(8), sequence: 3, data: None, distance: 8 },
        ] {
            let d = Packet::decode(&p.encode(Uuid(42), 77)).unwrap().unwrap();
            assert_eq!(d, Datagram { secret: Uuid(42), timestamp: 77, packet: p });
        }
    }

    #[test]
    fn foreign_datagrams_are_ignored() {
        assert_eq!(Packet::decode(&[1, 2, 3, 4, 5]).unwrap(), None);
        assert_eq!(Packet::decode(&[]).unwrap(), None);
    }
}
