//! Emotecraft's network message on `emotecraft:emote` (`EmotePacket`, `NetData`) and its
//! sub-packets (`DiscoveryPacket`, `NewAnimPacket`, `PlayerDataPacket`, `StopPacket`,
//! `EmoteHeaderPacket`, `SongPacket`, `EmoteIconPacket`, `RemoveEmotesPacket`), with the version
//! each side agreed on and the optional parts dropped past the size limit. A stored emote (the
//! `.emotecraft` file, a bound emote in the config) is the same message with purpose FILE.
//!
//! This Source Code Form is a port of Emotecraft (https://github.com/KosmX/emotes), GPL-3.0.

use crate::anim::{Animation, Extra};
use crate::buf::{Reader, Result, Writer, err};
use crate::opus::OpusSound;
use std::collections::BTreeMap;
use std::sync::Arc;

pub const CHANNEL: &str = "emotecraft:emote";
pub const STREAM_CHANNEL: &str = "emotecraft:stream";
pub const NETWORKING_VERSION: u8 = 8;
pub const MAX_PACKET_SIZE: usize = 1_048_576;

pub const LEGACY_ANIMATION_FORMAT: u8 = 0;
pub const NEW_ANIMATION_FORMAT: u8 = 0x99;
pub const NBS_CONFIG: u8 = 3;
pub const SERVER_TRACK_EMOTE_PLAY: u8 = 0x80;
pub const DISCOVERY_PACKET: u8 = 8;
pub const HEADER_PACKET: u8 = 0x11;
pub const ICON_PACKET: u8 = 0x12;
pub const PLAYER_DATA_PACKET: u8 = 1;
pub const STOP_PACKET: u8 = 10;
pub const REMOVE_EMOTE_PACKET: u8 = 5;

const OPUS_VERSION: u8 = 3;
const NBS_VERSION: u8 = 2;

/// `PacketTask`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Task {
    #[default]
    Unknown,
    Stream,
    Config,
    Stop,
    File,
    Remove,
}

impl Task {
    pub fn id(self) -> u8 {
        match self {
            Task::Unknown => 0,
            Task::Stream => 1,
            Task::Config => 8,
            Task::Stop => 10,
            Task::File => 0x10,
            Task::Remove => 2,
        }
    }
    fn of(id: u8) -> Task {
        match id {
            1 => Task::Stream,
            8 => Task::Config,
            10 => Task::Stop,
            0x10 => Task::File,
            2 => Task::Remove,
            _ => Task::Unknown,
        }
    }
    /// `playerBound`.
    pub fn player_bound(self) -> bool {
        matches!(self, Task::Stream | Task::Stop)
    }
}

/// Which side reads a message (`PacketBound`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    Server,
    Client,
}

/// Sub-packet id → version.
pub type Versions = BTreeMap<u8, u8>;

/// `EmotePacket.defaultVersions`.
pub fn default_versions() -> Versions {
    [
        (NEW_ANIMATION_FORMAT, crate::binary::CURRENT_VERSION),
        (LEGACY_ANIMATION_FORMAT, 4),
        (PLAYER_DATA_PACKET, 1),
        (DISCOVERY_PACKET, NETWORKING_VERSION),
        (STOP_PACKET, 1),
        (NBS_CONFIG, OPUS_VERSION),
        (HEADER_PACKET, 2),
        (ICON_PACKET, 0x12),
        (REMOVE_EMOTE_PACKET, 0),
    ]
    .into_iter()
    .collect()
}

/// `NetData`: what a message says.
#[derive(Clone, Debug, Default)]
pub struct NetData {
    pub purpose: Task,
    pub stop_emote: Option<u128>,
    pub remove_emotes: Vec<u128>,
    pub emote: Option<Arc<Animation>>,
    pub tick: f32,
    pub versions_updated: bool,
    pub versions: Versions,
    pub player: Option<u128>,
    pub forced: bool,
    pub size_limit: usize,
    pub strict_size_limit: bool,
    /// Sub-packets left out for the size (`skippedPackets`).
    pub skipped: Vec<u8>,
}

impl NetData {
    pub fn new(purpose: Task) -> NetData {
        NetData { purpose, versions: default_versions(), size_limit: MAX_PACKET_SIZE, strict_size_limit: true, ..NetData::default() }
    }
    /// `configureToStreamEmote`.
    pub fn stream(emote: Arc<Animation>, player: Option<u128>, tick: f32) -> NetData {
        NetData { emote: Some(emote), player, tick, ..NetData::new(Task::Stream) }
    }
    /// `configureToSendStop`.
    pub fn stop(emote: u128, player: Option<u128>) -> NetData {
        NetData { stop_emote: Some(emote), player, ..NetData::new(Task::Stop) }
    }
    /// `configureToSaveEmote`.
    pub fn file(emote: Arc<Animation>) -> NetData {
        NetData { emote: Some(emote), size_limit: i32::MAX as usize, ..NetData::new(Task::File) }
    }
    /// `configureToConfigExchange`, with `SERVER_TRACK_EMOTE_PLAY` for a server that tracks.
    pub fn config(tracking: bool) -> NetData {
        let mut d = NetData::new(Task::Config);
        if tracking {
            d.versions.insert(SERVER_TRACK_EMOTE_PLAY, 1);
        }
        d
    }
    pub fn remove(ids: Vec<u128>) -> NetData {
        NetData { remove_emotes: ids, ..NetData::new(Task::Remove) }
    }

    fn version_of(&self, id: u8, own: u8) -> u8 {
        self.versions.get(&id).map_or(own, |v| own.min(*v))
    }

    /// `SongPacket.getVer(config)`.
    fn song_version(&self) -> u8 {
        let Some(e) = &self.emote else { return 0 };
        let Some(&peer) = self.versions.get(&NBS_CONFIG) else { return 0 };
        let version = OPUS_VERSION.min(peer);
        if version >= OPUS_VERSION && e.sound.is_some() {
            return OPUS_VERSION;
        }
        if version < NBS_VERSION || e.nbs.is_none() {
            return 0;
        }
        if version == NBS_VERSION || self.purpose == Task::File { NBS_VERSION } else { 0 }
    }

    /// `EmotePacket.write`: the message for `target`. Optional parts past the size limit are
    /// left out (and listed in `skipped`); past it with a strict limit, an error.
    pub fn write(&mut self, target: Bound) -> Result<Vec<u8>> {
        if self.purpose == Task::Unknown {
            return err("Can't send packet without any purpose...");
        }
        self.skipped.clear();
        let mut w = Writer::new();
        w.i32(i32::from(*self.versions.get(&DISCOVERY_PACKET).unwrap_or(&NETWORKING_VERSION)));
        w.u8(self.purpose.id());
        let count_at = w.len();
        w.u8(0);
        let mut count = 0u8;
        let to_client = target == Bound::Client;
        let size_limit = self.size_limit;
        // NetHashMap's order: discovery, the animation, the player, stop, header, song, icon, remove.
        let mut sub = |w: &mut Writer, id: u8, version: u8, optional: bool, body: &dyn Fn(&mut Writer)| -> bool {
            let start = w.len();
            w.u8(id);
            w.u8(version);
            let size_at = w.len();
            w.i32(0);
            body(w);
            let size = w.len() - size_at - 4;
            w.set_i32(size_at, size as i32);
            if optional && w.len() > size_limit {
                w.data.truncate(start);
                log::warn!("[Emotecraft] Writing sub-packet {id} skipped!");
                return false;
            }
            count += 1;
            true
        };
        if self.purpose == Task::Config {
            let versions = self.versions.clone();
            sub(&mut w, DISCOVERY_PACKET, self.version_of(DISCOVERY_PACKET, NETWORKING_VERSION), false, &|w| {
                w.i32(versions.len() as i32);
                for (id, v) in &versions {
                    w.u8(*id);
                    w.u8(*v);
                }
            });
        }
        let animation = self.emote.clone().filter(|_| self.stop_emote.is_none());
        if let Some(e) = &animation
            && self.versions.contains_key(&NEW_ANIMATION_FORMAT)
        {
            let v = self.version_of(NEW_ANIMATION_FORMAT, crate::binary::CURRENT_VERSION);
            let tick = self.tick;
            sub(&mut w, NEW_ANIMATION_FORMAT, v, false, &|w| {
                w.f32(tick);
                crate::binary::write(w, v, e);
            });
        } else if animation.is_some() {
            log::warn!("[Emotecraft] The other side only reads the legacy animation format, which this port does not write");
        }
        if let Some(p) = self.player {
            let forced = self.forced;
            sub(&mut w, PLAYER_DATA_PACKET, self.version_of(PLAYER_DATA_PACKET, 1), false, &|w| {
                w.uuid(p);
                w.u8(u8::from(forced));
            });
        }
        if let Some(s) = self.stop_emote {
            sub(&mut w, STOP_PACKET, self.version_of(STOP_PACKET, 1), false, &|w| w.uuid(s));
        }
        if let Some(e) = self.emote.clone()
            && self.purpose == Task::File
            && to_client
        {
            let v = self.version_of(HEADER_PACKET, 2);
            sub(&mut w, HEADER_PACKET, v, false, &|w| {
                let s = |k: &str| e.str(k).unwrap_or_default().to_owned();
                write_cstring(w, &s("name"));
                write_cstring(w, &s("description"));
                write_cstring(w, &s("author"));
                if v >= 2 {
                    write_cstring(w, &s("folderpath"));
                    let bages: Vec<String> = match e.extra.get("bages") {
                        Some(Extra::List(l)) => l.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect(),
                        _ => Vec::new(),
                    };
                    w.i32(bages.len() as i32);
                    for b in &bages {
                        write_cstring(w, b);
                    }
                }
            });
        }
        let song = self.song_version();
        if song != 0
            && let Some(e) = self.emote.clone()
        {
            let file = self.purpose == Task::File;
            let wrote = sub(&mut w, NBS_CONFIG, song, true, &|w| {
                if song == OPUS_VERSION {
                    if let Some(s) = &e.sound {
                        w.varint(s.pre_skip | (s.output_gain << 16));
                        w.varint(s.end_trim);
                        w.varint(s.loop_start + 1);
                        w.varint(s.packet_count() as i32);
                        for i in 0..s.packet_count() {
                            let p = s.packet(i);
                            w.varint(p.len() as i32);
                            w.bytes(p);
                        }
                    }
                    if file && let Some(n) = &e.nbs {
                        w.bytes(n);
                    }
                } else if let Some(n) = &e.nbs {
                    w.bytes(n);
                }
            });
            if !wrote {
                self.skipped.push(NBS_CONFIG);
            }
        }
        if let Some(e) = self.emote.clone()
            && self.purpose == Task::File
            && to_client
            && let Some(icon) = &e.icon
        {
            let v = self.version_of(ICON_PACKET, 0x12);
            if !sub(&mut w, ICON_PACKET, v, true, &|w| {
                w.i32(icon.len() as i32);
                w.bytes(icon);
            }) {
                self.skipped.push(ICON_PACKET);
            }
        }
        if self.purpose == Task::Remove && !self.remove_emotes.is_empty() && to_client {
            let ids = self.remove_emotes.clone();
            sub(&mut w, REMOVE_EMOTE_PACKET, self.version_of(REMOVE_EMOTE_PACKET, 0), false, &|w| {
                w.varint(ids.len() as i32);
                for id in &ids {
                    w.uuid(*id);
                }
            });
        }
        if self.strict_size_limit && w.len() > self.size_limit {
            return err(format!("Can't send emote, packet's size ({}) is bigger than max allowed ({})!", w.len(), self.size_limit));
        }
        w.data[count_at] = count;
        Ok(w.data)
    }

    /// `new EmotePacket(buf, target)`.
    pub fn read(data: &[u8], target: Bound, playback: bool) -> Result<NetData> {
        let mut r = Reader::new(data);
        if r.remaining() < 6 {
            return err("Invalid packet header");
        }
        if r.i32()? > i32::from(NETWORKING_VERSION) {
            return err("Can't read newer version");
        }
        let mut d = NetData { purpose: Task::of(r.u8()?), versions: Versions::new(), size_limit: MAX_PACKET_SIZE, strict_size_limit: true, ..NetData::default() };
        let count = r.u8()?;
        let mut extra: BTreeMap<String, Extra> = BTreeMap::new();
        let mut icon: Option<Vec<u8>> = None;
        let mut sound: Option<OpusSound> = None;
        let mut nbs: Option<Vec<u8>> = None;
        let mut animation: Option<Animation> = None;
        for _ in 0..count {
            if r.remaining() < 6 {
                return err("Invalid sub-packet header");
            }
            let id = r.u8()?;
            let version = r.u8()?;
            let size = r.i32()?;
            if size < 0 || size as usize > r.remaining() {
                return err(format!("Invalid sub-packet size: {size}"));
            }
            let body = r.take(size as usize)?;
            let mut s = Reader::new(body);
            let to_client_only = matches!(id, HEADER_PACKET | ICON_PACKET | REMOVE_EMOTE_PACKET);
            if to_client_only && target != Bound::Client {
                continue;
            }
            let res: Result<()> = (|| {
                match id {
                    DISCOVERY_PACKET => {
                        let n = s.i32()?;
                        let mut map = Versions::new();
                        for _ in 0..n.max(0) {
                            let k = s.u8()?;
                            map.insert(k, s.u8()?);
                        }
                        d.versions = map;
                        d.versions_updated = true;
                    }
                    NEW_ANIMATION_FORMAT => {
                        d.tick = s.f32()?;
                        animation = Some(crate::binary::read(&mut s, version)?);
                    }
                    PLAYER_DATA_PACKET => {
                        d.player = Some(s.uuid()?);
                        if version >= 1 {
                            d.forced = s.u8()? != 0;
                        }
                    }
                    STOP_PACKET => d.stop_emote = Some(s.uuid()?),
                    HEADER_PACKET => {
                        for k in ["name", "description", "author"] {
                            extra.insert(k.into(), Extra::Str(read_cstring(&mut s)?));
                        }
                        if version >= 2 {
                            extra.insert("folderpath".into(), Extra::Str(read_cstring(&mut s)?));
                            let n = s.i32()?;
                            let mut list = Vec::new();
                            for _ in 0..n.max(0) {
                                list.push(Extra::Str(read_cstring(&mut s)?));
                            }
                            extra.insert("bages".into(), Extra::List(list));
                        }
                    }
                    NBS_CONFIG => match version {
                        OPUS_VERSION => {
                            let skip_gain = s.varint()?;
                            let pre_skip = skip_gain & 0xffff;
                            let gain = skip_gain >> 16;
                            let end_trim = s.varint()?;
                            let loop_start = s.varint()? - 1;
                            let count = s.varint()?;
                            if count < 0 || count as usize > s.remaining() / 2 {
                                return err(format!("Invalid Opus packet count: {count}"));
                            }
                            let mut data = Vec::new();
                            let mut offsets = Vec::with_capacity(count as usize + 1);
                            for _ in 0..count {
                                let n = s.varint()?;
                                if n <= 0 || n as usize > s.remaining() {
                                    return err(format!("Invalid Opus packet size: {n}"));
                                }
                                offsets.push(data.len());
                                data.extend_from_slice(s.take(n as usize)?);
                            }
                            offsets.push(data.len());
                            let ls = (loop_start >= 0).then_some(loop_start);
                            sound = Some(OpusSound::new(pre_skip, end_trim, gain, None, ls, data, offsets, Vec::new()).map_err(crate::buf::Error)?);
                            if d.purpose == Task::File && s.remaining() > 0 {
                                nbs = Some(s.rest().to_vec());
                            }
                        }
                        NBS_VERSION => {
                            if d.purpose != Task::Stream || !playback {
                                nbs = Some(s.rest().to_vec());
                            }
                        }
                        v => log::warn!("[Emotecraft] Dropping the sound of an emote: sub-packet version {v} is no longer supported"),
                    },
                    ICON_PACKET => {
                        let n = s.i32()?;
                        if n > 0 {
                            if n as usize > s.remaining() {
                                return err(format!("Invalid icon packet size: {n}"));
                            }
                            icon = Some(s.take(n as usize)?.to_vec());
                        }
                    }
                    REMOVE_EMOTE_PACKET => {
                        let n = s.varint()?;
                        d.remove_emotes = (0..n.max(0)).map(|_| s.uuid()).collect::<Result<_>>()?;
                    }
                    _ => {}
                }
                Ok(())
            })();
            if let Err(e) = res {
                if matches!(id, NBS_CONFIG | ICON_PACKET) {
                    log::warn!("[Emotecraft] Invalid sub-packet {id} received: {e}");
                } else {
                    return err(format!("Invalid sub-packet {id} received: {e}"));
                }
            }
        }
        if let Some(mut a) = animation {
            a.extra.extend(extra);
            if let Some(i) = icon {
                a.icon = Some(Arc::new(i));
            }
            if let Some(s) = sound {
                a.sound = Some(Arc::new(s));
            }
            if let Some(n) = nbs {
                a.nbs = Some(Arc::new(n));
            }
            d.emote = Some(Arc::new(a));
        }
        // `prepareAndValidate`.
        let valid = match d.purpose {
            Task::Unknown => false,
            Task::Stop => d.stop_emote.is_some(),
            Task::Stream => d.emote.is_some(),
            Task::Config => d.versions_updated,
            _ => true,
        } && (d.emote.is_none() || d.stop_emote.is_none());
        if !valid {
            return err("no valid data");
        }
        Ok(d)
    }
}

/// `CommonNetwork.writeString`: blank as a 0 length.
fn write_cstring(w: &mut Writer, s: &str) {
    if s.trim().is_empty() {
        w.i32(0);
    } else {
        w.istring(s);
    }
}

fn read_cstring(r: &mut Reader<'_>) -> Result<String> {
    r.istring()
}

/// `AnimationTypeAdapter.toBytes`: an emote as a stored message (a `.emotecraft` file).
pub fn emote_to_bytes(a: &Arc<Animation>) -> Vec<u8> {
    let mut d = NetData::file(a.clone());
    d.strict_size_limit = false;
    d.write(Bound::Client).unwrap_or_default()
}

/// `AnimationTypeAdapter.fromBytes`.
pub fn emote_from_bytes(b: &[u8]) -> Option<Arc<Animation>> {
    NetData::read(b, Bound::Client, false).ok()?.emote
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_and_file_round_trip() {
        let a = Arc::new(crate::loader::load_json(crate::tests::BUILT_IN_JSON[0]).unwrap().remove(0));
        let mut d = NetData::stream(a.clone(), Some(42), 7.5);
        let bytes = d.write(Bound::Server).unwrap();
        let back = NetData::read(&bytes, Bound::Server, false).unwrap();
        assert_eq!(back.purpose, Task::Stream);
        assert_eq!(back.player, Some(42));
        assert_eq!(back.tick, 7.5);
        assert_eq!(back.emote.as_ref().unwrap().uuid, a.uuid);
        let file = emote_to_bytes(&a);
        let b = emote_from_bytes(&file).unwrap();
        assert_eq!(b.str("name"), a.str("name"));
        assert_eq!(b.uuid, a.uuid);
    }

    #[test]
    fn config_exchange() {
        let mut d = NetData::config(true);
        let bytes = d.write(Bound::Client).unwrap();
        let back = NetData::read(&bytes, Bound::Client, false).unwrap();
        assert_eq!(back.purpose, Task::Config);
        assert_eq!(back.versions.get(&SERVER_TRACK_EMOTE_PLAY), Some(&1));
        assert_eq!(back.versions.get(&NEW_ANIMATION_FORMAT), Some(&6));
    }
}
