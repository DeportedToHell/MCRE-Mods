//! The voice chat server, as Plasmo Voice's with its defaults (`BaseVoiceServer`,
//! `PlayerChannelHandler`, `VoiceTcpServerConnectionManager`, `NettyPacketHandler`,
//! `NettyUdpServerConnection`, `NettyUdpKeepAlive`, `PlayerInfoRequestScheduler`,
//! `VoiceServerActivationManager` and the proximity activation's helper and player sources).
//!
//! It never decodes audio: a player's encrypted frames go out again, as their proximity source,
//! to every player near enough. The game tells it who joins and leaves (`join`, `quit`), passes it
//! the `plasmo:voice/v2` payloads players send (`handle_tcp`), sends what `take_tcp` gives, and
//! each tick says where players are (`update`). One thread serves UDP.
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use crate::tcp::{Activation, CaptureInfo, CodecInfo, Direction, EncryptionInfo, IconConfig, Packet, PlayerInfo, SourceInfo, SourceKind, SourceLine, PROXIMITY};
use crate::udp;
use crate::wire::Uuid;
use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// RSA encryption to a player's X.509 public key, PKCS#1 v1.5 (`Cipher.getInstance("RSA")`),
/// done by the game (Horizon's own RSA).
pub trait RsaEncrypt: Send + Sync {
    fn encrypt(&self, public_der: &[u8], data: &[u8]) -> Option<Vec<u8>>;
}

/// The server's settings, `VoiceServerConfig`'s defaults.
#[derive(Clone, Debug)]
pub struct Config {
    pub sample_rate: i32,
    pub mtu_size: i32,
    pub distances: Vec<i32>,
    pub default_distance: i32,
    pub max_extra_distance: i32,
    pub opus_mode: String,
    pub opus_bitrate: i32,
    pub keep_alive_timeout_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            sample_rate: 48_000,
            mtu_size: 1024,
            distances: vec![8, 16, 32],
            default_distance: 16,
            max_extra_distance: 16,
            opus_mode: "VOIP".into(),
            opus_bitrate: -1000,
            keep_alive_timeout_ms: 15_000,
        }
    }
}

/// Where a player is this tick: their world's name and their position.
#[derive(Clone, Debug, PartialEq)]
pub struct Position {
    pub world: String,
    pub pos: [f64; 3],
}

/// The Plasmo Voice version this server is (the client's major must match, 2).
const SERVER_VERSION: &str = crate::PROTOCOL_VERSION;
const ADDON_ID: &str = "plasmovoice";
const RETRY_DELAYS_MS: [u64; 5] = [1_000, 3_000, 5_000, 10_000, 15_000];
const STATE_BROADCAST_INTERVAL: Duration = Duration::from_millis(250);
const LANGUAGE_INTERVAL: Duration = Duration::from_millis(1_000);

/// `ProximityServerActivation`: the one activation and line a server has by default.
pub fn proximity_activation(config: &Config) -> Activation {
    Activation {
        name: PROXIMITY.into(),
        translation: "pv.activation.proximity".into(),
        icon: "plasmovoice:textures/icons/microphone.png".into(),
        distances: config.distances.clone(),
        default_distance: Activation::validate_default_distance(&config.distances, config.default_distance),
        proximity: true,
        transitive: true,
        stereo_supported: false,
        encoder: None,
        weight: 1,
    }
}

pub fn proximity_line() -> SourceLine {
    SourceLine {
        name: PROXIMITY.into(),
        translation: "pv.activation.proximity".into(),
        icon: "plasmovoice:textures/icons/speaker.png".into(),
        default_volume: 1.0,
        weight: 1,
        players: None,
    }
}

/// `Activation.checkDistance`.
fn check_distance(a: &Activation, distance: i32) -> bool {
    if a.distances.is_empty() {
        return true;
    }
    if a.distances.len() == 2 && a.distances[0] == -1 {
        return distance >= 1 && distance <= a.distances[1];
    }
    a.distances.contains(&distance)
}

/// `Activation.calculateAllowedDistance`.
fn allowed_distance(a: &Activation, distance: i32) -> i32 {
    if check_distance(a, distance) { distance } else { a.default_distance }
}

fn random_uuid() -> Uuid {
    let mut b = [0u8; 16];
    let _ = getrandom::fill(&mut b);
    // Version 4, IETF variant, as `UUID.randomUUID`.
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    Uuid(u128::from_be_bytes(b))
}

fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// `NettyUdpServerConnection`.
struct Connection {
    addr: SocketAddr,
    sent_keep_alive: Instant,
    last_received: Instant,
}

/// The player's proximity source (`VoiceServerPlayerSource`).
struct Source {
    id: Uuid,
    state: i32,
    dirty: bool,
}

/// `BaseVoicePlayer` with what the handlers keep about it.
struct Player {
    name: String,
    secret: Uuid,
    public_key: Option<Vec<u8>>,
    voice_disabled: bool,
    microphone_muted: bool,
    distances: HashMap<Uuid, i32>,
    active: HashSet<Uuid>,
    last_sequence: HashMap<Uuid, i64>,
    connection: Option<Connection>,
    source: Option<Source>,
    position: Option<Position>,
    /// `PlayerInfoRequestScheduler`: the last request and how many were retried.
    request: Option<(Instant, usize)>,
    last_state_broadcast: Option<Instant>,
    state_pending: bool,
    language: Option<String>,
    last_language: Option<Instant>,
    language_pending: bool,
}

impl Player {
    fn info(&self, id: Uuid) -> PlayerInfo {
        PlayerInfo { id, nick: self.name.clone(), muted: false, voice_disabled: self.voice_disabled, microphone_muted: self.microphone_muted }
    }
}

struct Shared {
    config: Config,
    activation: Activation,
    line: SourceLine,
    server_id: Uuid,
    key: [u8; 16],
    port: u16,
    players: HashMap<Uuid, Player>,
    out: Vec<(Uuid, Vec<u8>)>,
    /// UDP connections the thread opened, whose Config (it needs RSA) `update` sends.
    opened: Vec<Uuid>,
}

impl Shared {
    fn send(&mut self, to: Uuid, p: &Packet) {
        self.out.push((to, p.encode()));
    }
    fn connected(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.players.iter().filter(|(_, p)| p.connection.is_some()).map(|(id, _)| *id)
    }
    /// `broadcast`: to every player with voice chat.
    fn broadcast(&mut self, p: &Packet) {
        let to: Vec<Uuid> = self.connected().collect();
        let data = p.encode();
        for id in to {
            self.out.push((id, data.clone()));
        }
    }
    fn request_info(&mut self, id: Uuid) {
        self.send(id, &Packet::PlayerInfoRequest);
    }
    /// `VoiceServerProximitySource.getListeners`: UDP-connected players in the speaker's world
    /// within `min(distance + 16, distance * 2)`, the speaker excluded.
    fn listeners(&self, speaker: Uuid, distance: i16) -> Vec<(Uuid, Uuid, SocketAddr)> {
        let Some(at) = self.players.get(&speaker).and_then(|p| p.position.clone()) else { return Vec::new() };
        let d = i32::from(distance);
        let range = f64::from((d + self.config.max_extra_distance).min(d * 2));
        let range2 = range * range;
        self.players
            .iter()
            .filter(|(id, _)| **id != speaker)
            .filter_map(|(id, p)| {
                let c = p.connection.as_ref()?;
                let pos = p.position.as_ref()?;
                let (dx, dy, dz) = (pos.pos[0] - at.pos[0], pos.pos[1] - at.pos[1], pos.pos[2] - at.pos[2]);
                (pos.world == at.world && dx * dx + dy * dy + dz * dz <= range2).then_some((*id, p.secret, c.addr))
            })
            .collect()
    }
    fn source_info(&self, owner: Uuid) -> Option<SourceInfo> {
        let p = self.players.get(&owner)?;
        let s = p.source.as_ref()?;
        Some(SourceInfo {
            addon_id: ADDON_ID.into(),
            id: s.id,
            name: None,
            state: s.state as i8,
            decoder: Some(CodecInfo { name: "opus".into(), params: Vec::new() }),
            stereo: false,
            line_id: self.line.id(),
            icon_visible: true,
            angle: 0,
            kind: SourceKind::Player(p.info(owner)),
        })
    }
    fn send_config(&mut self, to: Uuid, rsa: &dyn RsaEncrypt) {
        let Some(key) = self.players.get(&to).and_then(|p| p.public_key.clone()) else { return };
        let Some(data) = rsa.encrypt(&key, &self.key) else { return };
        let packet = Packet::Config {
            server_id: self.server_id,
            capture: CaptureInfo {
                sample_rate: self.config.sample_rate,
                mtu_size: self.config.mtu_size,
                encoder: Some(CodecInfo {
                    name: "opus".into(),
                    params: vec![("mode".into(), self.config.opus_mode.clone()), ("bitrate".into(), self.config.opus_bitrate.to_string())],
                }),
            },
            encryption: Some(EncryptionInfo { algorithm: "AES/CBC/PKCS5Padding".into(), data }),
            source_lines: vec![self.line.clone()],
            activations: vec![self.activation.clone()],
            permissions: vec![("pv.allow_freecam".into(), true)],
            icon_config: Some(IconConfig::default()),
        };
        self.send(to, &packet);
    }
    fn send_player_list(&mut self, to: Uuid) {
        let players = self.connected().filter_map(|id| Some(self.players.get(&id)?.info(id))).collect();
        self.send(to, &Packet::PlayerList { players });
    }
    fn broadcast_info(&mut self, id: Uuid) {
        if let Some(info) = self.players.get(&id).map(|p| p.info(id)) {
            self.broadcast(&Packet::PlayerInfoUpdate { player: info });
        }
    }
    /// `removeConnection`: the player is told to everyone as gone, and its source dropped.
    fn disconnect(&mut self, id: Uuid) {
        let Some(p) = self.players.get_mut(&id) else { return };
        if p.connection.take().is_none() {
            return;
        }
        p.source = None;
        p.active.clear();
        self.broadcast(&Packet::PlayerDisconnect { player: id });
    }
    fn send_language(&mut self, id: Uuid) {
        let Some(p) = self.players.get_mut(&id) else { return };
        let Some(name) = p.language.clone() else { return };
        p.last_language = Some(Instant::now());
        p.language_pending = false;
        // `[client.pv.activation]` of the server's en_us.toml, or of Crowdin's translation in the
        // player's language (`use_crowdin_translations`, on by default).
        let proximity = proximity_name(&name);
        let entries = vec![("pv.activation.proximity".to_owned(), proximity.to_owned())];
        self.send(id, &Packet::Language { name, entries });
    }
}

/// `client.pv.activation.proximity` by language: en_us.toml and Crowdin's server.toml files.
fn proximity_name(language: &str) -> &'static str {
    match language.to_lowercase().as_str() {
        "cs_cz" => "Blízkost",
        "es_es" => "Proximidad",
        "fr_fr" => "Proximité",
        "ja_jp" => "近くで聞こえる",
        "ko_kr" => "근거리",
        "pl_pl" => "Zbliżeniowy",
        "pt_br" => "Proximidade",
        "ru_ru" => "Локальный",
        "sr_sp" => "Домет",
        "tr_tr" => "Yakınlık",
        "tt_ru" => "Якынлык",
        "uk_ua" => "Наближення",
        "vi_vn" => "Mọi người",
        "zh_cn" | "zh_tw" => "附近",
        _ => "Proximity",
    }
}

fn lock(m: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A Plasmo Voice server.
pub struct Server {
    shared: Arc<Mutex<Shared>>,
    rsa: Arc<dyn RsaEncrypt>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    /// Binds UDP to `port` on every interface (Plasmo's `host.port` 0 = the Minecraft server's
    /// port), or to any free port when that one is taken or zero.
    pub fn start(port: u16, config: Config, rsa: Arc<dyn RsaEncrypt>) -> std::io::Result<Server> {
        let socket = UdpSocket::bind(("0.0.0.0", port)).or_else(|_| UdpSocket::bind(("0.0.0.0", 0)))?;
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;
        let port = socket.local_addr()?.port();
        let mut key = [0u8; 16];
        // `BaseVoiceServer`: the key is the bytes of a random UUID.
        key.copy_from_slice(&random_uuid().0.to_be_bytes());
        let shared = Arc::new(Mutex::new(Shared {
            activation: proximity_activation(&config),
            line: proximity_line(),
            config,
            server_id: random_uuid(),
            key,
            port,
            players: HashMap::new(),
            out: Vec::new(),
            opened: Vec::new(),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (shared, stop) = (shared.clone(), stop.clone());
            std::thread::Builder::new().name("plasmovoice-server".into()).spawn(move || udp_loop(&shared, &stop, &socket))?
        };
        Ok(Server { shared, rsa, stop, thread: Some(thread) })
    }

    pub fn port(&self) -> u16 {
        lock(&self.shared).port
    }

    /// A player joined: they are asked for their info, again after 1, 3, 5, 10 and 15 s.
    pub fn join(&mut self, id: Uuid, name: &str) {
        let mut s = lock(&self.shared);
        s.players.insert(
            id,
            Player {
                name: name.into(),
                secret: random_uuid(),
                public_key: None,
                voice_disabled: false,
                microphone_muted: false,
                distances: HashMap::new(),
                active: HashSet::new(),
                last_sequence: HashMap::new(),
                connection: None,
                source: None,
                position: None,
                request: Some((Instant::now(), 0)),
                last_state_broadcast: None,
                state_pending: false,
                language: None,
                last_language: None,
                language_pending: false,
            },
        );
        s.request_info(id);
    }

    pub fn quit(&mut self, id: Uuid) {
        let mut s = lock(&self.shared);
        s.disconnect(id);
        s.players.remove(&id);
    }

    /// Each server tick: positions, and the retries and rate-limited replies that are due.
    pub fn update(&mut self, positions: &[(Uuid, Position)]) {
        let mut s = lock(&self.shared);
        for (id, pos) in positions {
            if let Some(p) = s.players.get_mut(id) {
                p.position = Some(pos.clone());
            }
        }
        // `NettyPacketHandler`, for each new UDP connection: the Config, the player list, and
        // the player to everyone.
        for id in std::mem::take(&mut s.opened) {
            if s.players.get(&id).is_some_and(|p| p.connection.is_some()) {
                s.send_config(id, &*self.rsa);
                s.send_player_list(id);
                s.broadcast_info(id);
            }
        }
        let now = Instant::now();
        let ids: Vec<Uuid> = s.players.keys().copied().collect();
        for id in ids {
            let Some(p) = s.players.get_mut(&id) else { continue };
            let mut request = false;
            if let Some((last, attempt)) = p.request {
                if p.public_key.is_some() || attempt >= RETRY_DELAYS_MS.len() {
                    p.request = None;
                } else if now.duration_since(last) >= Duration::from_millis(RETRY_DELAYS_MS[attempt]) {
                    p.request = Some((now, attempt + 1));
                    request = true;
                }
            }
            let state_due = p.state_pending && p.last_state_broadcast.is_none_or(|t| now.duration_since(t) >= STATE_BROADCAST_INTERVAL);
            let language_due = p.language_pending && p.last_language.is_none_or(|t| now.duration_since(t) >= LANGUAGE_INTERVAL);
            if state_due {
                p.state_pending = false;
                p.last_state_broadcast = Some(now);
            }
            if request {
                s.request_info(id);
            }
            if state_due && s.players.get(&id).is_some_and(|p| p.connection.is_some()) {
                s.broadcast_info(id);
            }
            if language_due {
                s.send_language(id);
            }
        }
    }

    /// A `plasmo:voice/v2` payload from a player (`PlayerChannelHandler`).
    pub fn handle_tcp(&mut self, from: Uuid, data: &[u8]) {
        let Ok(Some(packet)) = Packet::decode(data, Direction::Server) else { return };
        let mut s = lock(&self.shared);
        let Some(p) = s.players.get_mut(&from) else { return };
        // Any packet ends the info request retries.
        p.request = None;
        let has_voice = p.connection.is_some();
        match packet {
            Packet::PlayerInfo { voice_disabled, microphone_muted, version, public_key, .. } => {
                let (Some(client), Some(server)) = (semver(&version), semver(SERVER_VERSION)) else { return };
                if client.0 != server.0 || client < (2, 0, 0) {
                    return;
                }
                p.public_key = Some(public_key);
                p.voice_disabled = voice_disabled;
                p.microphone_muted = microphone_muted;
                let secret = p.secret;
                let port = i32::from(s.port);
                s.send(from, &Packet::Connection { secret, ip: "0.0.0.0".into(), port });
            }
            Packet::PlayerState { voice_disabled, microphone_muted } => {
                if !has_voice || (p.voice_disabled == voice_disabled && p.microphone_muted == microphone_muted) {
                    return;
                }
                p.voice_disabled = voice_disabled;
                p.microphone_muted = microphone_muted;
                if p.last_state_broadcast.is_none_or(|t| t.elapsed() >= STATE_BROADCAST_INTERVAL) {
                    p.last_state_broadcast = Some(Instant::now());
                    s.broadcast_info(from);
                } else {
                    p.state_pending = true;
                }
            }
            Packet::PlayerActivationDistances { distances } => {
                let known = s.activation.id();
                if let Some(p) = s.players.get_mut(&from) {
                    for (activation, distance) in distances {
                        if activation == known {
                            p.distances.insert(activation, distance);
                        }
                    }
                }
            }
            Packet::PlayerAudioEnd { sequence, activation, distance } => {
                if !has_voice || p.microphone_muted || activation != s.activation.id() {
                    return;
                }
                let distance = allowed_distance(&s.activation, i32::from(distance));
                if !check_distance(&s.activation, distance) {
                    return;
                }
                let Some(p) = s.players.get_mut(&from) else { return };
                if !p.active.remove(&activation) {
                    return;
                }
                p.last_sequence.insert(activation, sequence);
                let Some(source) = p.source.as_ref().map(|x| x.id) else { return };
                let to: Vec<Uuid> = s.listeners(from, distance as i16).into_iter().map(|l| l.0).collect();
                for id in to {
                    s.send(id, &Packet::SourceAudioEnd { source, sequence });
                }
            }
            Packet::SourceInfoRequest { source } => {
                if !has_voice {
                    return;
                }
                let owner = s.players.iter().find(|(_, p)| p.source.as_ref().is_some_and(|x| x.id == source)).map(|(id, _)| *id);
                // The owner's own source is filtered out for them (`filterSelf`).
                if let Some(owner) = owner.filter(|o| *o != from)
                    && let Some(info) = s.source_info(owner)
                {
                    s.send(from, &Packet::SourceInfo(info));
                }
            }
            Packet::LanguageRequest { language } => {
                p.language = Some(language);
                if p.last_language.is_none_or(|t| t.elapsed() >= LANGUAGE_INTERVAL) {
                    s.send_language(from);
                } else {
                    p.language_pending = true;
                }
            }
            _ => {}
        }
    }

    /// Payloads to send on `plasmo:voice/v2`, to each player.
    pub fn take_tcp(&mut self) -> Vec<(Uuid, Vec<u8>)> {
        std::mem::take(&mut lock(&self.shared).out)
    }

    /// Players with voice chat (for a player list).
    pub fn players(&self) -> Vec<PlayerInfo> {
        let s = lock(&self.shared);
        s.connected().filter_map(|id| Some(s.players.get(&id)?.info(id))).collect()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn semver(v: &str) -> Option<(u32, u32, u32)> {
    let core = v.split(['-', '+']).next()?;
    let mut it = core.split('.').map(|x| x.parse::<u32>().ok());
    Some((it.next()??, it.next().flatten().unwrap_or(0), it.next().flatten().unwrap_or(0)))
}

/// The UDP side: new connections, pings, keep-alive and the relay.
fn udp_loop(shared: &Mutex<Shared>, stop: &AtomicBool, socket: &UdpSocket) {
    let mut buf = vec![0u8; 4096];
    let mut last_tick = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        if let Ok((n, from)) = socket.recv_from(&mut buf)
            && let Ok(Some(d)) = udp::Packet::decode(&buf[..n])
        {
            let mut s = lock(shared);
            handle_udp(&mut s, socket, from, d);
        }
        if last_tick.elapsed() >= Duration::from_millis(100) {
            last_tick = Instant::now();
            keep_alive(&mut lock(shared), socket);
        }
    }
}

/// `NettyUdpKeepAlive.tick`.
fn keep_alive(s: &mut Shared, socket: &UdpSocket) {
    let now = Instant::now();
    let timeout = Duration::from_millis(s.config.keep_alive_timeout_ms);
    let ids: Vec<Uuid> = s.connected().collect();
    for id in ids {
        let Some(p) = s.players.get_mut(&id) else { continue };
        let Some(c) = p.connection.as_mut() else { continue };
        if now.duration_since(c.last_received) > timeout {
            s.disconnect(id);
            s.request_info(id);
        } else if now >= c.sent_keep_alive + Duration::from_millis(1_000) {
            let mut r = [0u8; 2];
            let _ = getrandom::fill(&mut r);
            let jitter = 1_500 + u64::from(u16::from_le_bytes(r)) % 1_500;
            c.sent_keep_alive = now + Duration::from_millis(jitter);
            let ping = udp::Packet::Ping { time: now_millis(), server: None };
            let _ = socket.send_to(&ping.encode(p.secret, now_millis()), c.addr);
        }
    }
}

fn handle_udp(s: &mut Shared, socket: &UdpSocket, from: SocketAddr, d: udp::Datagram) {
    let Some(id) = s.players.iter().find(|(_, p)| p.secret == d.secret).map(|(id, _)| *id) else { return };
    let Some(p) = s.players.get_mut(&id) else { return };
    match p.connection.as_mut() {
        Some(c) => {
            c.addr = from;
            c.last_received = Instant::now();
        }
        None => {
            // `NettyPacketHandler`: the first datagram with a known secret opens the connection;
            // the Config (it needs RSA) goes out from `Server::update` .
            p.connection = Some(Connection { addr: from, sent_keep_alive: Instant::now() - Duration::from_secs(10), last_received: Instant::now() });
            s.opened.push(id);
            return;
        }
    }
    match d.packet {
        udp::Packet::PlayerAudio { sequence, data, activation, distance, stereo: _ } => {
            if p.microphone_muted || activation != s.activation.id() {
                return;
            }
            let distance = allowed_distance(&s.activation, i32::from(distance)) as i16;
            let Some(p) = s.players.get_mut(&id) else { return };
            let last = p.last_sequence.get(&activation).copied().unwrap_or(0);
            if !p.active.contains(&activation) && (sequence > last || (sequence - last).abs() > 10) {
                p.active.insert(activation);
            }
            let source = p.source.get_or_insert_with(|| Source { id: random_uuid(), state: 1, dirty: true });
            let (source_id, state, dirty) = (source.id, source.state, std::mem::replace(&mut source.dirty, false));
            let listeners = s.listeners(id, distance);
            if dirty && let Some(info) = s.source_info(id) {
                for (to, _, _) in &listeners {
                    s.send(*to, &Packet::SourceInfo(info.clone()));
                }
            }
            let packet = udp::Packet::SourceAudio { sequence, data, source: source_id, state: state as i8, distance };
            let ts = now_millis();
            for (_, secret, addr) in listeners {
                let _ = socket.send_to(&packet.encode(secret, ts), addr);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No real RSA in tests: the "encrypted" key is the key.
    struct Plain;
    impl RsaEncrypt for Plain {
        fn encrypt(&self, _: &[u8], data: &[u8]) -> Option<Vec<u8>> {
            Some(data.to_vec())
        }
    }

    struct Fake {
        id: Uuid,
        sock: UdpSocket,
        secret: Uuid,
    }

    fn tcp_to(server: &mut Server, id: Uuid) -> Vec<Packet> {
        let out = server.take_tcp();
        let mut mine = Vec::new();
        for (to, data) in out {
            if to == id {
                mine.push(Packet::decode(&data, Direction::Client).unwrap().unwrap());
            }
        }
        mine
    }

    fn connect(server: &mut Server, id: Uuid, name: &str) -> Fake {
        server.join(id, name);
        assert!(matches!(tcp_to(server, id)[..], [Packet::PlayerInfoRequest]));
        let info = Packet::PlayerInfo {
            voice_disabled: false,
            microphone_muted: false,
            minecraft_version: "26.3".into(),
            version: "2.1.8".into(),
            public_key: vec![1, 2, 3],
        };
        server.handle_tcp(id, &info.encode());
        let packets = tcp_to(server, id);
        let Some(Packet::Connection { secret, ip, port }) = packets.into_iter().next() else { panic!("no connection packet") };
        assert_eq!(ip, "0.0.0.0");
        assert_eq!(port, i32::from(server.port()));
        let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        sock.set_read_timeout(Some(Duration::from_millis(500))).unwrap();
        sock.connect(("127.0.0.1", server.port())).unwrap();
        let ping = udp::Packet::Ping { time: 0, server: Some(("127.0.0.1".into(), 25565)) };
        sock.send(&ping.encode(secret, 0)).unwrap();
        // The thread opens the connection; the next update sends the Config.
        let mut config = None;
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(10));
            server.update(&[]);
            if let Some(c) = tcp_to(server, id).into_iter().find(|p| matches!(p, Packet::Config { .. })) {
                config = Some(c);
                break;
            }
        }
        let Some(Packet::Config { activations, source_lines, encryption, capture, .. }) = config else { panic!("no config") };
        assert_eq!(activations[0].id(), Activation::id_of(PROXIMITY));
        assert_eq!(source_lines[0].id(), SourceLine::id_of(PROXIMITY));
        assert_eq!(encryption.unwrap().data.len(), 16);
        assert_eq!(capture.sample_rate, 48_000);
        Fake { id, sock, secret }
    }

    fn at(world: &str, x: f64) -> Position {
        Position { world: world.into(), pos: [x, 64.0, 0.0] }
    }

    #[test]
    fn relays_to_players_in_range() {
        let mut server = Server::start(0, Config::default(), Arc::new(Plain)).unwrap();
        let a = connect(&mut server, Uuid(1), "alice");
        let b = connect(&mut server, Uuid(2), "bob");
        let c = connect(&mut server, Uuid(3), "carol");
        server.update(&[(a.id, at("overworld", 0.0)), (b.id, at("overworld", 10.0)), (c.id, at("overworld", 100.0))]);
        server.take_tcp();
        // Alice speaks at 16 blocks: Bob (10 away) hears, Carol (100 away) doesn't.
        let audio = udp::Packet::PlayerAudio { sequence: 1, data: vec![9; 40], activation: Activation::id_of(PROXIMITY), distance: 16, stereo: false };
        a.sock.send(&audio.encode(a.secret, 0)).unwrap();
        let mut buf = [0u8; 2048];
        let got = loop {
            let n = b.sock.recv(&mut buf).expect("bob hears");
            let d = udp::Packet::decode(&buf[..n]).unwrap().unwrap();
            if let udp::Packet::SourceAudio { .. } = d.packet {
                assert_eq!(d.secret, b.secret);
                break d.packet;
            }
        };
        let udp::Packet::SourceAudio { sequence, data, state, distance, source } = got else { unreachable!() };
        assert_eq!((sequence, data, state, distance), (1, vec![9; 40], 1, 16));
        // Bob was told whose source it is.
        let info = tcp_to(&mut server, b.id);
        assert!(info.iter().any(|p| matches!(p, Packet::SourceInfo(i) if i.id == source && matches!(&i.kind, SourceKind::Player(p) if p.nick == "alice"))));
        c.sock.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
        loop {
            match c.sock.recv(&mut buf) {
                Ok(n) => assert!(!matches!(udp::Packet::decode(&buf[..n]).unwrap().unwrap().packet, udp::Packet::SourceAudio { .. }), "carol heard"),
                Err(_) => break,
            }
        }
        // The end goes over TCP to the listeners.
        server.handle_tcp(a.id, &Packet::PlayerAudioEnd { sequence: 2, activation: Activation::id_of(PROXIMITY), distance: 16 }.encode());
        assert!(tcp_to(&mut server, b.id).iter().any(|p| matches!(p, Packet::SourceAudioEnd { source: s, sequence: 2 } if *s == source)));
    }

    #[test]
    fn old_or_other_major_clients_are_ignored() {
        let mut server = Server::start(0, Config::default(), Arc::new(Plain)).unwrap();
        server.join(Uuid(1), "x");
        server.take_tcp();
        for version in ["1.0.9", "3.0.0", "garbage"] {
            let info = Packet::PlayerInfo { voice_disabled: false, microphone_muted: false, minecraft_version: "".into(), version: version.into(), public_key: vec![] };
            server.handle_tcp(Uuid(1), &info.encode());
            assert!(server.take_tcp().is_empty(), "{version}");
        }
    }

    #[test]
    fn distances_and_versions() {
        let a = proximity_activation(&Config::default());
        assert_eq!(allowed_distance(&a, 32), 32);
        assert_eq!(allowed_distance(&a, 31), 16);
        assert_eq!(semver("2.1.8-SNAPSHOT"), Some((2, 1, 8)));
    }
}
