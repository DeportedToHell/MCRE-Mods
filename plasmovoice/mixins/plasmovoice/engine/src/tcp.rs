//! Plasmo Voice's "TCP" packets: the ones that travel as Minecraft custom payloads on the
//! `plasmo:voice/v2` channel, `[u8 id][body]` (`PacketTcpCodec`), ids in registration order.
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use crate::md5::name_uuid;
use crate::wire::{Error, Reader, Result, Uuid, Writer};

/// The channel every packet travels on (`BaseVoiceServer.CHANNEL_STRING`).
pub const CHANNEL: &str = "plasmo:voice/v2";
/// The flag channel a client registers to say it has the mod (`FLAG_CHANNEL_STRING`).
pub const FLAG_CHANNEL: &str = "plasmo:voice/v2/installed";

// ---- data ---------------------------------------------------------------------------------

/// `Pos3d`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pos3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Pos3d {
    fn write(&self, w: &mut Writer) {
        w.f64(self.x);
        w.f64(self.y);
        w.f64(self.z);
    }
    fn read(r: &mut Reader<'_>) -> Result<Pos3d> {
        Ok(Pos3d { x: r.f64()?, y: r.f64()?, z: r.f64()? })
    }
}

/// `CodecInfo`: a codec's name and parameters (in the order they came; Java keeps a HashMap).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CodecInfo {
    pub name: String,
    pub params: Vec<(String, String)>,
}

impl CodecInfo {
    pub fn param(&self, key: &str) -> Option<&str> {
        self.params.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
    fn write(&self, w: &mut Writer) {
        w.utf(&self.name);
        w.i32(self.params.len() as i32);
        for (k, v) in &self.params {
            w.utf(k);
            w.utf(v);
        }
    }
    fn read(r: &mut Reader<'_>) -> Result<CodecInfo> {
        let name = r.utf()?;
        let n = r.safe_i32(0, 128, "codec params")?;
        let mut params = Vec::with_capacity(n as usize);
        for _ in 0..n {
            params.push((r.utf()?, r.utf()?));
        }
        Ok(CodecInfo { name, params })
    }
}

fn write_opt_codec(w: &mut Writer, c: &Option<CodecInfo>) {
    w.bool(c.is_some());
    if let Some(c) = c {
        c.write(w);
    }
}
fn read_opt_codec(r: &mut Reader<'_>) -> Result<Option<CodecInfo>> {
    if r.bool()? { Ok(Some(CodecInfo::read(r)?)) } else { Ok(None) }
}

/// `CaptureInfo`.
#[derive(Clone, Debug, PartialEq)]
pub struct CaptureInfo {
    pub sample_rate: i32,
    pub mtu_size: i32,
    pub encoder: Option<CodecInfo>,
}

/// `EncryptionInfo`: the algorithm and the server's key, encrypted with the client's RSA key.
#[derive(Clone, Debug, PartialEq)]
pub struct EncryptionInfo {
    pub algorithm: String,
    pub data: Vec<u8>,
}

/// `McGameProfile`.
#[derive(Clone, Debug, PartialEq)]
pub struct GameProfile {
    pub id: Uuid,
    pub name: String,
    /// name, value, signature ("" for none).
    pub properties: Vec<(String, String, String)>,
}

impl GameProfile {
    fn write(&self, w: &mut Writer) {
        w.uuid(self.id);
        w.utf(&self.name);
        w.i32(self.properties.len() as i32);
        for (n, v, s) in &self.properties {
            w.utf(n);
            w.utf(v);
            w.utf(s);
        }
    }
    fn read(r: &mut Reader<'_>) -> Result<GameProfile> {
        let id = r.uuid()?;
        let name = r.utf()?;
        let n = r.safe_i32(0, 100, "profile properties")?;
        let mut properties = Vec::with_capacity(n as usize);
        for _ in 0..n {
            properties.push((r.utf()?, r.utf()?, r.utf()?));
        }
        Ok(GameProfile { id, name, properties })
    }
}

/// `VoiceActivation`. Its id isn't sent: `generateId(name)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Activation {
    pub name: String,
    pub translation: String,
    pub icon: String,
    pub distances: Vec<i32>,
    pub default_distance: i32,
    pub proximity: bool,
    pub transitive: bool,
    pub stereo_supported: bool,
    pub encoder: Option<CodecInfo>,
    pub weight: i32,
}

/// The built-in proximity activation's and line's name (`VoiceActivation.PROXIMITY_NAME`).
pub const PROXIMITY: &str = "proximity";

impl Activation {
    /// `VoiceActivation.generateId`.
    pub fn id_of(name: &str) -> Uuid {
        name_uuid(format!("{name}_activation").as_bytes())
    }
    pub fn id(&self) -> Uuid {
        Activation::id_of(&self.name)
    }
    /// `validateDefaultDistance`.
    pub fn validate_default_distance(distances: &[i32], default: i32) -> i32 {
        if distances.is_empty() {
            return 0;
        }
        if distances.len() == 2 && distances[0] == -1 {
            return if default >= 1 && default <= distances[1] { default } else { distances[1] / 2 };
        }
        if distances.contains(&default) { default } else { distances[distances.len() / 2] }
    }
    fn write(&self, w: &mut Writer) {
        w.utf(&self.name);
        w.utf(&self.translation);
        w.utf(&self.icon);
        w.i32(self.distances.len() as i32);
        for d in &self.distances {
            w.i32(*d);
        }
        w.i32(self.default_distance);
        w.bool(self.proximity);
        w.bool(self.transitive);
        w.bool(self.stereo_supported);
        write_opt_codec(w, &self.encoder);
        w.i32(self.weight);
    }
    fn read(r: &mut Reader<'_>) -> Result<Activation> {
        let name = r.utf()?;
        let translation = r.utf()?;
        let icon = r.utf()?;
        let n = r.safe_i32(0, 64, "distances")?;
        let mut distances = Vec::with_capacity(n as usize);
        for _ in 0..n {
            distances.push(r.i32()?);
        }
        let default_distance = Activation::validate_default_distance(&distances, r.i32()?);
        Ok(Activation {
            name,
            translation,
            icon,
            distances,
            default_distance,
            proximity: r.bool()?,
            transitive: r.bool()?,
            stereo_supported: r.bool()?,
            encoder: read_opt_codec(r)?,
            weight: r.i32()?,
        })
    }
}

/// `VoiceSourceLine`. Its id isn't sent: `generateId(name)`.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceLine {
    pub name: String,
    pub translation: String,
    pub icon: String,
    pub default_volume: f64,
    pub weight: i32,
    pub players: Option<Vec<GameProfile>>,
}

impl SourceLine {
    /// `VoiceSourceLine.generateId`.
    pub fn id_of(name: &str) -> Uuid {
        name_uuid(format!("{name}_line").as_bytes())
    }
    pub fn id(&self) -> Uuid {
        SourceLine::id_of(&self.name)
    }
    fn write(&self, w: &mut Writer) {
        w.utf(&self.name);
        w.utf(&self.translation);
        w.utf(&self.icon);
        w.f64(self.default_volume);
        w.i32(self.weight);
        w.bool(self.players.is_some());
        if let Some(p) = &self.players {
            w.i32(p.len() as i32);
            for g in p {
                g.write(w);
            }
        }
    }
    fn read(r: &mut Reader<'_>) -> Result<SourceLine> {
        let name = r.utf()?;
        let translation = r.utf()?;
        let icon = r.utf()?;
        let default_volume = r.f64()?;
        let weight = r.i32()?;
        let players = if r.bool()? {
            let n = r.safe_i32(0, i32::from(i16::MAX), "line players")?;
            let mut p = Vec::with_capacity(n as usize);
            for _ in 0..n {
                p.push(GameProfile::read(r)?);
            }
            Some(p)
        } else {
            None
        };
        Ok(SourceLine { name, translation, icon, default_volume, weight, players })
    }
}

/// `VoicePlayerInfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerInfo {
    pub id: Uuid,
    pub nick: String,
    pub muted: bool,
    pub voice_disabled: bool,
    pub microphone_muted: bool,
}

impl PlayerInfo {
    fn write(&self, w: &mut Writer) {
        w.uuid(self.id);
        w.utf(&self.nick);
        w.bool(self.muted);
        w.bool(self.voice_disabled);
        w.bool(self.microphone_muted);
    }
    fn read(r: &mut Reader<'_>) -> Result<PlayerInfo> {
        Ok(PlayerInfo { id: r.uuid()?, nick: r.utf()?, muted: r.bool()?, voice_disabled: r.bool()?, microphone_muted: r.bool()? })
    }
}

/// `PlayerIconVisibility`.
pub const ICON_VISIBILITY: [&str; 5] =
    ["HIDE_NOT_INSTALLED", "HIDE_VOICE_CHAT_DISABLED", "HIDE_SERVER_MUTED", "HIDE_CLIENT_MUTED", "HIDE_SOURCE_ICON"];

/// `PlayerIconConfig` (since 2.1.7).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IconConfig {
    /// Names from `ICON_VISIBILITY`.
    pub hidden: Vec<String>,
    pub offset: Pos3d,
}

/// A source's kind and its own fields (`SourceInfo.Type`).
#[derive(Clone, Debug, PartialEq)]
pub enum SourceKind {
    Player(PlayerInfo),
    Entity(i32),
    Static { pos: Pos3d, look: Pos3d },
    Direct { sender: Option<GameProfile>, relative: Option<Pos3d>, camera_relative: bool },
}

/// `SourceInfo`.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceInfo {
    pub addon_id: String,
    pub id: Uuid,
    pub name: Option<String>,
    pub state: i8,
    pub decoder: Option<CodecInfo>,
    pub stereo: bool,
    pub line_id: Uuid,
    pub icon_visible: bool,
    pub angle: i32,
    pub kind: SourceKind,
}

impl SourceInfo {
    fn write(&self, w: &mut Writer) {
        w.utf(match self.kind {
            SourceKind::Player(_) => "PLAYER",
            SourceKind::Entity(_) => "ENTITY",
            SourceKind::Static { .. } => "STATIC",
            SourceKind::Direct { .. } => "DIRECT",
        });
        w.utf(&self.addon_id);
        w.uuid(self.id);
        w.nullable_utf(self.name.as_deref());
        w.i8(self.state);
        write_opt_codec(w, &self.decoder);
        w.bool(self.stereo);
        w.uuid(self.line_id);
        w.bool(self.icon_visible);
        w.i32(self.angle);
        match &self.kind {
            SourceKind::Player(p) => p.write(w),
            SourceKind::Entity(e) => w.i32(*e),
            SourceKind::Static { pos, look } => {
                pos.write(w);
                look.write(w);
            }
            SourceKind::Direct { sender, relative, camera_relative } => {
                w.bool(sender.is_some());
                if let Some(s) = sender {
                    s.write(w);
                }
                w.bool(relative.is_some());
                if let Some(p) = relative {
                    p.write(w);
                }
                w.bool(*camera_relative);
            }
        }
    }
    fn read(r: &mut Reader<'_>) -> Result<SourceInfo> {
        let ty = r.utf()?;
        let addon_id = r.utf()?;
        let id = r.uuid()?;
        let name = r.nullable_utf()?;
        let state = r.i8()?;
        let decoder = read_opt_codec(r)?;
        let stereo = r.bool()?;
        let line_id = r.uuid()?;
        let icon_visible = r.bool()?;
        let angle = r.i32()?;
        let kind = match ty.as_str() {
            "PLAYER" => SourceKind::Player(PlayerInfo::read(r)?),
            "ENTITY" => SourceKind::Entity(r.i32()?),
            "STATIC" => SourceKind::Static { pos: Pos3d::read(r)?, look: Pos3d::read(r)? },
            "DIRECT" => {
                let sender = if r.bool()? { Some(GameProfile::read(r)?) } else { None };
                let relative = if r.bool()? { Some(Pos3d::read(r)?) } else { None };
                SourceKind::Direct { sender, relative, camera_relative: r.bool()? }
            }
            _ => return Err(Error::Unknown("source type")),
        };
        Ok(SourceInfo { addon_id, id, name, state, decoder, stereo, line_id, icon_visible, angle, kind })
    }
}

// ---- packets ------------------------------------------------------------------------------

/// The TCP packets, by their ids (1-based, in `PacketTcpCodec`'s order).
#[derive(Clone, Debug, PartialEq)]
pub enum Packet {
    /// 1 S→C: the UDP secret and address ("0.0.0.0" = the Minecraft server's; port 0 = its port).
    Connection { secret: Uuid, ip: String, port: i32 },
    /// 2 S→C.
    PlayerInfoRequest,
    /// 3 S→C.
    Config {
        server_id: Uuid,
        capture: CaptureInfo,
        encryption: Option<EncryptionInfo>,
        source_lines: Vec<SourceLine>,
        activations: Vec<Activation>,
        permissions: Vec<(String, bool)>,
        /// None from servers older than 2.1.7.
        icon_config: Option<IconConfig>,
    },
    /// 4 S→C.
    ConfigPlayerInfo { permissions: Vec<(String, bool)> },
    /// 5 C→S.
    LanguageRequest { language: String },
    /// 6 S→C.
    Language { name: String, entries: Vec<(String, String)> },
    /// 7 S→C.
    PlayerList { players: Vec<PlayerInfo> },
    /// 8 S→C.
    PlayerInfoUpdate { player: PlayerInfo },
    /// 9 S→C.
    PlayerDisconnect { player: Uuid },
    /// 10 C→S: the client's state, versions and X.509 RSA public key.
    PlayerInfo { voice_disabled: bool, microphone_muted: bool, minecraft_version: String, version: String, public_key: Vec<u8> },
    /// 11 C→S.
    PlayerState { voice_disabled: bool, microphone_muted: bool },
    /// 12 C→S.
    PlayerAudioEnd { sequence: i64, activation: Uuid, distance: i16 },
    /// 13 C→S.
    PlayerActivationDistances { distances: Vec<(Uuid, i32)> },
    /// 14 S→C.
    DistanceVisualize { radius: i32, color: i32, position: Option<Pos3d> },
    /// 15 C→S.
    SourceInfoRequest { source: Uuid },
    /// 16 S→C.
    SourceInfo(SourceInfo),
    /// 17 S→C.
    SelfSourceInfo { source: SourceInfo, player: Uuid, activation: Uuid, sequence: i64 },
    /// 18 S→C.
    SourceAudioEnd { source: Uuid, sequence: i64 },
    /// 19 S→C.
    ActivationRegister(Activation),
    /// 20 S→C.
    ActivationUnregister { activation: Uuid },
    /// 21 S→C.
    SourceLineRegister(SourceLine),
    /// 22 S→C.
    SourceLineUnregister { line: Uuid },
    /// 23 S→C.
    SourceLinePlayerAdd { line: Uuid, player: GameProfile },
    /// 24 S→C.
    SourceLinePlayerRemove { line: Uuid, player: Uuid },
    /// 25 S→C.
    SourceLinePlayersList { line: Uuid, players: Vec<GameProfile> },
    /// 26 S→C: a JSON text component.
    AnimatedActionBar { json: String },
}

/// Which way a packet goes (`PacketDirection`): `Client` packets are clientbound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Client,
    Server,
}

fn write_permissions(w: &mut Writer, p: &[(String, bool)]) {
    w.i32(p.len() as i32);
    for (k, v) in p {
        w.utf(k);
        w.bool(*v);
    }
}
fn read_permissions(r: &mut Reader<'_>) -> Result<Vec<(String, bool)>> {
    let n = r.safe_i32(0, i32::from(i8::MAX), "permissions")?;
    let mut p = Vec::with_capacity(n as usize);
    for _ in 0..n {
        p.push((r.utf()?, r.bool()?));
    }
    Ok(p)
}

impl Packet {
    pub fn id(&self) -> u8 {
        match self {
            Packet::Connection { .. } => 1,
            Packet::PlayerInfoRequest => 2,
            Packet::Config { .. } => 3,
            Packet::ConfigPlayerInfo { .. } => 4,
            Packet::LanguageRequest { .. } => 5,
            Packet::Language { .. } => 6,
            Packet::PlayerList { .. } => 7,
            Packet::PlayerInfoUpdate { .. } => 8,
            Packet::PlayerDisconnect { .. } => 9,
            Packet::PlayerInfo { .. } => 10,
            Packet::PlayerState { .. } => 11,
            Packet::PlayerAudioEnd { .. } => 12,
            Packet::PlayerActivationDistances { .. } => 13,
            Packet::DistanceVisualize { .. } => 14,
            Packet::SourceInfoRequest { .. } => 15,
            Packet::SourceInfo(_) => 16,
            Packet::SelfSourceInfo { .. } => 17,
            Packet::SourceAudioEnd { .. } => 18,
            Packet::ActivationRegister(_) => 19,
            Packet::ActivationUnregister { .. } => 20,
            Packet::SourceLineRegister(_) => 21,
            Packet::SourceLineUnregister { .. } => 22,
            Packet::SourceLinePlayerAdd { .. } => 23,
            Packet::SourceLinePlayerRemove { .. } => 24,
            Packet::SourceLinePlayersList { .. } => 25,
            Packet::AnimatedActionBar { .. } => 26,
        }
    }

    /// The direction a packet id is registered for.
    pub fn direction_of(id: u8) -> Option<Direction> {
        match id {
            5 | 10..=13 | 15 => Some(Direction::Server),
            1..=26 => Some(Direction::Client),
            _ => None,
        }
    }

    /// `PacketTcpCodec.encode`.
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u8(self.id());
        match self {
            Packet::Connection { secret, ip, port } => {
                w.uuid(*secret);
                w.utf(ip);
                w.i32(*port);
            }
            Packet::PlayerInfoRequest => {}
            Packet::Config { server_id, capture, encryption, source_lines, activations, permissions, icon_config } => {
                w.uuid(*server_id);
                w.i32(capture.sample_rate);
                w.i32(capture.mtu_size);
                write_opt_codec(&mut w, &capture.encoder);
                w.bool(encryption.is_some());
                if let Some(e) = encryption {
                    w.utf(&e.algorithm);
                    w.bytes(&e.data);
                }
                w.i32(source_lines.len() as i32);
                for l in source_lines {
                    l.write(&mut w);
                }
                w.i32(activations.len() as i32);
                for a in activations {
                    a.write(&mut w);
                }
                write_permissions(&mut w, permissions);
                let icons = icon_config.clone().unwrap_or_default();
                w.i32(icons.hidden.len() as i32);
                for h in &icons.hidden {
                    w.utf(h);
                }
                icons.offset.write(&mut w);
            }
            Packet::ConfigPlayerInfo { permissions } => write_permissions(&mut w, permissions),
            Packet::LanguageRequest { language } => w.utf(language),
            Packet::Language { name, entries } => {
                w.utf(name);
                w.i32(entries.len() as i32);
                for (k, v) in entries {
                    w.utf(k);
                    w.utf(v);
                }
            }
            Packet::PlayerList { players } => {
                w.i32(players.len() as i32);
                for p in players {
                    p.write(&mut w);
                }
            }
            Packet::PlayerInfoUpdate { player } => player.write(&mut w),
            Packet::PlayerDisconnect { player } => w.uuid(*player),
            Packet::PlayerInfo { voice_disabled, microphone_muted, minecraft_version, version, public_key } => {
                w.bool(*voice_disabled);
                w.bool(*microphone_muted);
                w.utf(minecraft_version);
                w.utf(version);
                w.bytes(public_key);
            }
            Packet::PlayerState { voice_disabled, microphone_muted } => {
                w.bool(*voice_disabled);
                w.bool(*microphone_muted);
            }
            Packet::PlayerAudioEnd { sequence, activation, distance } => {
                w.i64(*sequence);
                w.uuid(*activation);
                w.i16(*distance);
            }
            Packet::PlayerActivationDistances { distances } => {
                w.i32(distances.len() as i32);
                for (id, d) in distances {
                    w.uuid(*id);
                    w.i32(*d);
                }
            }
            Packet::DistanceVisualize { radius, color, position } => {
                w.i32(*radius);
                w.i32(*color);
                w.bool(position.is_some());
                if let Some(p) = position {
                    p.write(&mut w);
                }
            }
            Packet::SourceInfoRequest { source } => w.uuid(*source),
            Packet::SourceInfo(s) => s.write(&mut w),
            Packet::SelfSourceInfo { source, player, activation, sequence } => {
                source.write(&mut w);
                w.uuid(*player);
                w.uuid(*activation);
                w.i64(*sequence);
            }
            Packet::SourceAudioEnd { source, sequence } => {
                w.uuid(*source);
                w.i64(*sequence);
            }
            Packet::ActivationRegister(a) => a.write(&mut w),
            Packet::ActivationUnregister { activation } => w.uuid(*activation),
            Packet::SourceLineRegister(l) => l.write(&mut w),
            Packet::SourceLineUnregister { line } => w.uuid(*line),
            Packet::SourceLinePlayerAdd { line, player } => {
                w.uuid(*line);
                player.write(&mut w);
            }
            Packet::SourceLinePlayerRemove { line, player } => {
                w.uuid(*line);
                w.uuid(*player);
            }
            Packet::SourceLinePlayersList { line, players } => {
                w.uuid(*line);
                w.i32(players.len() as i32);
                for p in players {
                    p.write(&mut w);
                }
            }
            Packet::AnimatedActionBar { json } => w.utf(json),
        }
        w.into_bytes()
    }

    /// `PacketTcpCodec.decode` for packets going `dir` (`None`: the type isn't registered for it).
    pub fn decode(data: &[u8], dir: Direction) -> Result<Option<Packet>> {
        let mut r = Reader::new(data);
        let id = r.u8()?;
        if Packet::direction_of(id) != Some(dir) {
            return Ok(None);
        }
        let r = &mut r;
        let p = match id {
            1 => Packet::Connection { secret: r.uuid()?, ip: r.utf()?, port: r.i32()? },
            2 => Packet::PlayerInfoRequest,
            3 => {
                let server_id = r.uuid()?;
                let capture = CaptureInfo { sample_rate: r.i32()?, mtu_size: r.i32()?, encoder: read_opt_codec(r)? };
                let encryption = if r.bool()? {
                    Some(EncryptionInfo { algorithm: r.utf()?, data: r.bytes(1, 2048, "encryption data")? })
                } else {
                    None
                };
                let n = r.safe_i32(0, i32::from(i8::MAX), "source lines")?;
                let mut source_lines = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    source_lines.push(SourceLine::read(r)?);
                }
                let n = r.safe_i32(0, i32::from(i8::MAX), "activations")?;
                let mut activations = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    activations.push(Activation::read(r)?);
                }
                let permissions = read_permissions(r)?;
                // Since 2.1.7; an older server ends here (the original catches the failure).
                let mut rest = r.clone();
                let icon_config = (|| -> Result<IconConfig> {
                    let n = rest.safe_i32(0, ICON_VISIBILITY.len() as i32, "icon visibility")?;
                    let mut hidden = Vec::with_capacity(n as usize);
                    for _ in 0..n {
                        let name = rest.utf()?;
                        if !ICON_VISIBILITY.contains(&name.as_str()) {
                            return Err(Error::Unknown("icon visibility"));
                        }
                        hidden.push(name);
                    }
                    Ok(IconConfig { hidden, offset: Pos3d::read(&mut rest)? })
                })()
                .ok();
                if icon_config.is_some() {
                    *r = rest;
                }
                Packet::Config { server_id, capture, encryption, source_lines, activations, permissions, icon_config }
            }
            4 => Packet::ConfigPlayerInfo { permissions: read_permissions(r)? },
            5 => Packet::LanguageRequest { language: r.safe_utf(32, "language")? },
            6 => {
                let name = r.utf()?;
                let n = r.safe_i32(0, i32::from(i16::MAX), "language entries")?;
                let mut entries = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    entries.push((r.utf()?, r.utf()?));
                }
                Packet::Language { name, entries }
            }
            7 => {
                let n = r.safe_i32(0, i32::from(i16::MAX), "players")?;
                let mut players = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    players.push(PlayerInfo::read(r)?);
                }
                Packet::PlayerList { players }
            }
            8 => Packet::PlayerInfoUpdate { player: PlayerInfo::read(r)? },
            9 => Packet::PlayerDisconnect { player: r.uuid()? },
            10 => Packet::PlayerInfo {
                voice_disabled: r.bool()?,
                microphone_muted: r.bool()?,
                minecraft_version: r.safe_utf(64, "minecraft version")?,
                version: r.safe_utf(64, "version")?,
                public_key: r.bytes(1, 2048, "public key")?,
            },
            11 => Packet::PlayerState { voice_disabled: r.bool()?, microphone_muted: r.bool()? },
            12 => Packet::PlayerAudioEnd { sequence: r.i64()?, activation: r.uuid()?, distance: r.i16()? },
            13 => {
                let n = r.safe_i32(0, i32::from(i8::MAX), "activation distances")?;
                let mut distances = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    distances.push((r.uuid()?, r.i32()?));
                }
                Packet::PlayerActivationDistances { distances }
            }
            14 => Packet::DistanceVisualize {
                radius: r.i32()?,
                color: r.i32()?,
                position: if r.bool()? { Some(Pos3d::read(r)?) } else { None },
            },
            15 => Packet::SourceInfoRequest { source: r.uuid()? },
            16 => Packet::SourceInfo(SourceInfo::read(r)?),
            17 => Packet::SelfSourceInfo { source: SourceInfo::read(r)?, player: r.uuid()?, activation: r.uuid()?, sequence: r.i64()? },
            18 => Packet::SourceAudioEnd { source: r.uuid()?, sequence: r.i64()? },
            19 => Packet::ActivationRegister(Activation::read(r)?),
            20 => Packet::ActivationUnregister { activation: r.uuid()? },
            21 => Packet::SourceLineRegister(SourceLine::read(r)?),
            22 => Packet::SourceLineUnregister { line: r.uuid()? },
            23 => Packet::SourceLinePlayerAdd { line: r.uuid()?, player: GameProfile::read(r)? },
            24 => Packet::SourceLinePlayerRemove { line: r.uuid()?, player: r.uuid()? },
            25 => {
                let line = r.uuid()?;
                let n = r.i32()?;
                if n < 0 {
                    return Err(Error::Bounds("line players"));
                }
                let mut players = Vec::new();
                for _ in 0..n {
                    players.push(GameProfile::read(r)?);
                }
                Packet::SourceLinePlayersList { line, players }
            }
            26 => Packet::AnimatedActionBar { json: r.utf()? },
            _ => return Ok(None),
        };
        Ok(Some(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(p: Packet) {
        let dir = Packet::direction_of(p.id()).unwrap();
        let bytes = p.encode();
        assert_eq!(Packet::decode(&bytes, dir).unwrap(), Some(p));
    }

    #[test]
    fn proximity_ids_match_java() {
        // From Java 8: UUID.nameUUIDFromBytes("proximity_activation".getBytes("UTF-8")) and "_line".
        assert_eq!(Activation::id_of(PROXIMITY).to_string(), "4aec07ba-d109-3345-9a0a-92022a116cd0");
        assert_eq!(SourceLine::id_of(PROXIMITY).to_string(), "d9392647-f6b2-332d-9adf-30a6400828d6");
    }

    #[test]
    fn default_distance_validation() {
        assert_eq!(Activation::validate_default_distance(&[8, 16, 32], 16), 16);
        assert_eq!(Activation::validate_default_distance(&[8, 16, 32], 10), 16);
        assert_eq!(Activation::validate_default_distance(&[-1, 100], 40), 40);
        assert_eq!(Activation::validate_default_distance(&[-1, 100], 0), 50);
        assert_eq!(Activation::validate_default_distance(&[], 7), 0);
    }

    #[test]
    fn packets_round_trip() {
        let u = |n: u128| Uuid(n);
        let codec = Some(CodecInfo { name: "opus".into(), params: vec![("mode".into(), "VOIP".into()), ("bitrate".into(), "-1000".into())] });
        let profile = GameProfile { id: u(9), name: "Steve".into(), properties: vec![("textures".into(), "abc".into(), String::new())] };
        let player = PlayerInfo { id: u(5), nick: "Alex".into(), muted: false, voice_disabled: true, microphone_muted: false };
        let activation = Activation {
            name: PROXIMITY.into(),
            translation: "pv.activation.proximity".into(),
            icon: "plasmovoice:textures/icons/microphone.png".into(),
            distances: vec![8, 16, 32],
            default_distance: 16,
            proximity: true,
            transitive: true,
            stereo_supported: false,
            encoder: codec.clone(),
            weight: 1,
        };
        let line = SourceLine {
            name: PROXIMITY.into(),
            translation: "pv.activation.proximity".into(),
            icon: "plasmovoice:textures/icons/speaker.png".into(),
            default_volume: 1.0,
            weight: 1,
            players: Some(vec![profile.clone()]),
        };
        let source = SourceInfo {
            addon_id: "pv".into(),
            id: u(77),
            name: None,
            state: 3,
            decoder: codec.clone(),
            stereo: false,
            line_id: line.id(),
            icon_visible: true,
            angle: 360,
            kind: SourceKind::Player(player.clone()),
        };
        for p in [
            Packet::Connection { secret: u(1), ip: "0.0.0.0".into(), port: 0 },
            Packet::PlayerInfoRequest,
            Packet::Config {
                server_id: u(2),
                capture: CaptureInfo { sample_rate: 48000, mtu_size: 1024, encoder: codec.clone() },
                encryption: Some(EncryptionInfo { algorithm: "AES/CBC/PKCS5Padding".into(), data: vec![1; 256] }),
                source_lines: vec![line.clone()],
                activations: vec![activation.clone()],
                permissions: vec![("pv.activation.proximity".into(), true)],
                icon_config: Some(IconConfig { hidden: vec!["HIDE_NOT_INSTALLED".into()], offset: Pos3d { x: 0.0, y: 0.5, z: 0.0 } }),
            },
            Packet::ConfigPlayerInfo { permissions: vec![("pv.allow_freecam".into(), false)] },
            Packet::LanguageRequest { language: "ru_ru".into() },
            Packet::Language { name: "ru_ru".into(), entries: vec![("gui.plasmovoice.title".into(), "Plasmo Voice".into())] },
            Packet::PlayerList { players: vec![player.clone()] },
            Packet::PlayerInfoUpdate { player: player.clone() },
            Packet::PlayerDisconnect { player: u(5) },
            Packet::PlayerInfo { voice_disabled: false, microphone_muted: true, minecraft_version: "26.3".into(), version: "2.2.0".into(), public_key: vec![3; 294] },
            Packet::PlayerState { voice_disabled: true, microphone_muted: false },
            Packet::PlayerAudioEnd { sequence: 42, activation: activation.id(), distance: 16 },
            Packet::PlayerActivationDistances { distances: vec![(activation.id(), 16)] },
            Packet::DistanceVisualize { radius: 16, color: 0x00a000, position: None },
            Packet::SourceInfoRequest { source: u(77) },
            Packet::SourceInfo(source.clone()),
            Packet::SelfSourceInfo { source: source.clone(), player: u(5), activation: activation.id(), sequence: 7 },
            Packet::SourceAudioEnd { source: u(77), sequence: 8 },
            Packet::ActivationRegister(activation.clone()),
            Packet::ActivationUnregister { activation: activation.id() },
            Packet::SourceLineRegister(line.clone()),
            Packet::SourceLineUnregister { line: line.id() },
            Packet::SourceLinePlayerAdd { line: line.id(), player: profile.clone() },
            Packet::SourceLinePlayerRemove { line: line.id(), player: u(9) },
            Packet::SourceLinePlayersList { line: line.id(), players: vec![profile] },
            Packet::AnimatedActionBar { json: "{\"text\":\"hi\"}".into() },
        ] {
            round_trip(p);
        }
    }

    #[test]
    fn config_from_an_old_server_has_no_icon_config() {
        let mut bytes = Packet::Config {
            server_id: Uuid(2),
            capture: CaptureInfo { sample_rate: 48000, mtu_size: 1024, encoder: None },
            encryption: None,
            source_lines: vec![],
            activations: vec![],
            permissions: vec![],
            icon_config: Some(IconConfig::default()),
        }
        .encode();
        // Drop the icon config (an i32 count and three doubles): what a 2.1.6 server sends.
        bytes.truncate(bytes.len() - 4 - 24);
        match Packet::decode(&bytes, Direction::Client).unwrap() {
            Some(Packet::Config { icon_config, .. }) => assert_eq!(icon_config, None),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn wrong_direction_is_ignored() {
        let bytes = Packet::PlayerInfoRequest.encode();
        assert_eq!(Packet::decode(&bytes, Direction::Server).unwrap(), None);
    }
}
