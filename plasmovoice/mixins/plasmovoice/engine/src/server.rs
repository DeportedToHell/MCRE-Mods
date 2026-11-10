//! The voice chat server, as Plasmo Voice's (`BaseVoiceServer`, `PlayerChannelHandler`,
//! `VoiceTcpServerConnectionManager`, `NettyPacketHandler`, `NettyUdpServerConnection`,
//! `NettyUdpKeepAlive`, `PlayerInfoRequestScheduler`, `ModRequiredKickHandler`,
//! `VoiceServerActivationManager`, the proximity activation's helper and player sources,
//! `VoiceMuteManager` and the commands in `server_commands`).
//!
//! It never decodes audio: a player's encrypted frames go out again, as their proximity source,
//! to every player near enough. The game tells it who joins and leaves (`join`, `quit`), passes it
//! the `plasmo:voice/v2` payloads players send (`handle_tcp`), sends what `take_tcp` gives, tells
//! players what `take_tells` gives, kicks whom `take_kicks` names, and each tick says where players
//! are (`update`) and what they may do (`set_player`). One thread serves UDP.
//!
//! Its files are Plasmo Voice's, in the server's Plasmo Voice folder: `config.toml`
//! (`server_config`), `voice_mutes.json` (`mutes`) and `languages/` (`server_lang`).
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use crate::mutes::{MuteDuration, MuteInfo, MuteStorage};
use crate::server_config::{ServerConfig, parse_uuid, random_uuid};
use crate::server_lang::Languages;
use crate::tcp::{
    Activation, CaptureInfo, CodecInfo, Direction, EncryptionInfo, IconConfig, Packet, PlayerInfo, Pos3d, SourceInfo, SourceKind, SourceLine,
    PROXIMITY,
};
use crate::udp;
use crate::wire::Uuid;
use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// RSA encryption to a player's X.509 public key, PKCS#1 v1.5 (`Cipher.getInstance("RSA")`),
/// done by the game (Horizon's own RSA).
pub trait RsaEncrypt: Send + Sync {
    fn encrypt(&self, public_der: &[u8], data: &[u8]) -> Option<Vec<u8>>;
}

/// Where a player is this tick: their world's name and their position.
#[derive(Clone, Debug, PartialEq)]
pub struct Position {
    pub world: String,
    pub pos: [f64; 3],
}

/// What a player may do, as the server's permissions answer (`Permission`'s defaults: everyone
/// talks in proximity and may use freecam; the mod requirement is bypassed by operators).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Perms {
    pub proximity: bool,
    pub allow_freecam: bool,
    pub bypass_mod_requirement: bool,
}

impl Default for Perms {
    fn default() -> Self {
        Perms { proximity: true, allow_freecam: true, bypass_mod_requirement: false }
    }
}

/// The Plasmo Voice version this server is (the client's major must match, 2).
const SERVER_VERSION: &str = crate::PROTOCOL_VERSION;
const ADDON_ID: &str = "plasmovoice";
const RETRY_DELAYS_MS: [u64; 5] = [1_000, 3_000, 5_000, 10_000, 15_000];
const STATE_BROADCAST_INTERVAL: Duration = Duration::from_millis(250);
const LANGUAGE_INTERVAL: Duration = Duration::from_millis(1_000);
/// `VoiceMuteManager`'s expiry check.
const MUTE_TICK: Duration = Duration::from_secs(5);
/// `VoicePlayer.visualizeDistance`'s default colour.
const VISUALIZE_COLOR: i32 = 0x00a000;
const MODRINTH_LINK: &str = "https://modrinth.com/plugin/plasmo-voice";

/// `ProximityServerActivation`: the one activation and line a server has by default.
pub fn proximity_activation(config: &ServerConfig) -> Activation {
    let v = &config.voice;
    Activation {
        name: PROXIMITY.into(),
        translation: "pv.activation.proximity".into(),
        icon: "plasmovoice:textures/icons/microphone.png".into(),
        distances: v.distances.clone(),
        default_distance: Activation::validate_default_distance(&v.distances, v.default_distance),
        proximity: true,
        transitive: true,
        stereo_supported: false,
        encoder: None,
        weight: v.activation_weights.get(PROXIMITY).copied().unwrap_or(1),
    }
}

pub fn proximity_line(config: &ServerConfig) -> SourceLine {
    SourceLine {
        name: PROXIMITY.into(),
        translation: "pv.activation.proximity".into(),
        icon: "plasmovoice:textures/icons/speaker.png".into(),
        default_volume: 1.0,
        weight: config.voice.source_line_weights.get(PROXIMITY).copied().unwrap_or(1),
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

pub(crate) fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// `NettyUdpServerConnection`.
pub(crate) struct Connection {
    addr: SocketAddr,
    sent_keep_alive: Instant,
    last_received: Instant,
}

/// The player's proximity source (`VoiceServerPlayerSource`).
pub(crate) struct Source {
    id: Uuid,
    state: i32,
    dirty: bool,
}

/// `BaseVoicePlayer` with what the handlers keep about it.
pub(crate) struct Player {
    pub(crate) name: String,
    secret: Uuid,
    public_key: Option<Vec<u8>>,
    voice_disabled: bool,
    microphone_muted: bool,
    distances: HashMap<Uuid, i32>,
    active: HashSet<Uuid>,
    last_sequence: HashMap<Uuid, i64>,
    pub(crate) connection: Option<Connection>,
    source: Option<Source>,
    pub(crate) position: Option<Position>,
    /// `PlayerInfoRequestScheduler`: the last request and how many were retried.
    request: Option<(Instant, usize)>,
    last_state_broadcast: Option<Instant>,
    state_pending: bool,
    /// The language the Plasmo Voice client asked for (`LanguageRequestPacket`).
    language: Option<String>,
    last_language: Option<Instant>,
    language_pending: bool,
    /// The game client's own language (`ClientInformation`), for the server's messages.
    pub(crate) mc_language: Option<String>,
    perms: Perms,
    /// `ModRequiredKickHandler`: when a player who sent nothing is kicked.
    kick_at: Option<Instant>,
}

impl Player {
    /// The language the server's messages to this player are in.
    pub(crate) fn text_language(&self) -> Option<&str> {
        self.mc_language.as_deref().or(self.language.as_deref())
    }
}

pub(crate) struct Shared {
    pub(crate) config: ServerConfig,
    pub(crate) langs: Languages,
    pub(crate) mutes: MuteStorage,
    activation: Activation,
    line: SourceLine,
    key: [u8; 16],
    /// The UDP port bound, or none when the UDP server couldn't start.
    port: Option<u16>,
    pub(crate) players: HashMap<Uuid, Player>,
    /// Names of the players seen (this run, and the server's `usercache.json`).
    pub(crate) profiles: HashMap<Uuid, String>,
    out: Vec<(Uuid, Vec<u8>)>,
    /// UDP connections the thread opened, whose Config (it needs RSA) `update` sends.
    opened: Vec<Uuid>,
    pub(crate) tells: Vec<(Uuid, String)>,
    kicks: Vec<(Uuid, String)>,
    last_mute_tick: Instant,
}

impl Shared {
    fn send(&mut self, to: Uuid, p: &Packet) {
        self.out.push((to, p.encode()));
    }
    fn connected(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.players.iter().filter(|(_, p)| p.connection.is_some()).map(|(id, _)| *id)
    }
    /// `broadcast`: to every player with voice chat that `filter` lets through.
    fn broadcast_if(&mut self, p: &Packet, filter: impl Fn(&Player) -> bool) {
        let to: Vec<Uuid> = self.players.iter().filter(|(_, p)| p.connection.is_some() && filter(p)).map(|(id, _)| *id).collect();
        let data = p.encode();
        for id in to {
            self.out.push((id, data.clone()));
        }
    }
    fn broadcast(&mut self, p: &Packet) {
        self.broadcast_if(p, |_| true);
    }
    pub(crate) fn request_info(&mut self, id: Uuid) {
        self.send(id, &Packet::PlayerInfoRequest);
    }
    /// `createPlayerInfo`: muted is the server's mute.
    fn info(&self, id: Uuid) -> Option<PlayerInfo> {
        let p = self.players.get(&id)?;
        Some(PlayerInfo {
            id,
            nick: p.name.clone(),
            muted: self.mutes.get(id).is_some(),
            voice_disabled: p.voice_disabled,
            microphone_muted: p.microphone_muted,
        })
    }
    pub(crate) fn tr(&self, language: Option<&str>, key: &str, args: &[&str]) -> String {
        self.langs.tr(language, key, args)
    }
    fn player_language(&self, id: Uuid) -> Option<String> {
        self.players.get(&id).and_then(|p| p.text_language().map(str::to_owned))
    }
    /// A message to a player, in their language.
    fn tell(&mut self, id: Uuid, key: &str, args: &[&str]) {
        let text = self.tr(self.player_language(id).as_deref(), key, args);
        self.tells.push((id, text));
    }
    /// `VoiceServerProximitySource.getListeners`: UDP-connected players in the speaker's world
    /// within `min(distance + max_extra, distance * 2)`, the speaker excluded.
    fn listeners(&self, speaker: Uuid, distance: i16) -> Vec<(Uuid, Uuid, SocketAddr)> {
        let Some(at) = self.players.get(&speaker).and_then(|p| p.position.clone()) else { return Vec::new() };
        let d = i32::from(distance);
        let range = f64::from((d + self.config.voice.max_extra_audio_broadcast_distance).min(d * 2));
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
            kind: SourceKind::Player(self.info(owner)?),
        })
    }
    fn permissions(&self, id: Uuid) -> Vec<(String, bool)> {
        let freecam = self.players.get(&id).is_none_or(|p| p.perms.allow_freecam);
        vec![("pv.allow_freecam".into(), freecam)]
    }
    /// `sendConfigInfo`.
    fn send_config(&mut self, to: Uuid, rsa: &dyn RsaEncrypt) {
        if self.port.is_none() {
            return;
        }
        let Some(p) = self.players.get(&to) else { return };
        let Some(key) = p.public_key.clone() else { return };
        let proximity = p.perms.proximity;
        let Some(data) = rsa.encrypt(&key, &self.key) else {
            log::error!(target: crate::LOG, "[Plasmo Voice] Failed to encode encryption data for {}", p.name);
            return;
        };
        let Some(server_id) = parse_uuid(&self.config.server_id) else { return };
        let v = &self.config.voice;
        let packet = Packet::Config {
            server_id,
            capture: CaptureInfo {
                sample_rate: v.sample_rate,
                mtu_size: v.mtu_size,
                encoder: Some(CodecInfo {
                    name: "opus".into(),
                    params: vec![("mode".into(), v.opus_mode.clone()), ("bitrate".into(), v.opus_bitrate.to_string())],
                }),
            },
            encryption: Some(EncryptionInfo { algorithm: "AES/CBC/PKCS5Padding".into(), data }),
            source_lines: vec![self.line.clone()],
            activations: if proximity { vec![self.activation.clone()] } else { Vec::new() },
            permissions: self.permissions(to),
            icon_config: Some(IconConfig { hidden: v.icon_visibility.clone(), offset: Pos3d { x: 0.0, y: v.icon_y_offset, z: 0.0 } }),
        };
        self.send(to, &packet);
    }
    fn send_player_list(&mut self, to: Uuid) {
        let players = self.connected().filter_map(|id| self.info(id)).collect();
        self.send(to, &Packet::PlayerList { players });
    }
    pub(crate) fn broadcast_info(&mut self, id: Uuid) {
        if let Some(info) = self.info(id) {
            self.broadcast(&Packet::PlayerInfoUpdate { player: info });
        }
    }
    /// `removeConnection`: the player is told to everyone as gone, and its source dropped.
    pub(crate) fn disconnect(&mut self, id: Uuid) {
        let Some(p) = self.players.get_mut(&id) else { return };
        let Some(c) = p.connection.take() else { return };
        p.source = None;
        p.active.clear();
        log::info!(target: crate::LOG, "[Plasmo Voice] {} disconnected ({})", p.name, c.addr);
        self.broadcast(&Packet::PlayerDisconnect { player: id });
    }
    fn send_language(&mut self, id: Uuid) {
        let Some(p) = self.players.get_mut(&id) else { return };
        let Some(name) = p.language.clone() else { return };
        p.last_language = Some(Instant::now());
        p.language_pending = false;
        // `getClientLanguage`: the `[client]` texts of the player's language, Plasmo Voice's own
        // names of the proximity line for the languages the port doesn't carry.
        let mut entries = self.langs.client(&name);
        if !self.langs.has(&name)
            && self.config.forced_language.is_none()
            && let Some(e) = entries.iter_mut().find(|(k, _)| k == "pv.activation.proximity")
        {
            e.1 = proximity_name(&name).to_owned();
        }
        self.send(id, &Packet::Language { name, entries });
    }
    /// `VoiceMuteManager.mute`, for a player on the server.
    pub(crate) fn mute(&mut self, target: Uuid, by: Option<Uuid>, duration: MuteDuration, reason: Option<String>, silent: bool) {
        let now = now_millis();
        let muted_to = match duration {
            MuteDuration::Permanent => 0,
            MuteDuration::Time { amount, unit: crate::mutes::DurationUnit::Timestamp } => amount * 1_000,
            MuteDuration::Time { amount, unit } => amount * unit.millis() + now,
        };
        let info = MuteInfo { player: target, muted_by: by, muted_at: now, muted_to, reason: reason.clone(), silent: Some(silent) };
        if let Err(e) = self.mutes.put(info, now) {
            log::error!(target: crate::LOG, "[Plasmo Voice] Failed to save json mute storage: {e}");
        }
        self.broadcast_info(target);
        if !silent {
            let lang = self.player_language(target);
            let reason = self.mute_reason(lang.as_deref(), reason.as_deref());
            match duration {
                MuteDuration::Permanent => self.tell(target, "pv.mutes.permanently_muted", &[&reason]),
                MuteDuration::Time { amount, unit } => {
                    let d = self.duration_text(lang.as_deref(), amount, unit, now);
                    self.tell(target, "pv.mutes.temporarily_muted", &[&d, &reason]);
                }
            }
        }
    }
    /// `VoiceMuteManager.unmute`.
    pub(crate) fn unmute(&mut self, target: Uuid, silent: bool) -> Option<MuteInfo> {
        let gone = match self.mutes.remove(target, now_millis()) {
            Ok(g) => g?,
            Err(e) => {
                log::error!(target: crate::LOG, "[Plasmo Voice] Failed to save json mute storage: {e}");
                return None;
            }
        };
        if self.players.contains_key(&target) {
            self.broadcast_info(target);
            if !silent {
                self.tell(target, "pv.mutes.unmuted", &[]);
            }
        }
        Some(gone)
    }
    /// `formatMuteReason`.
    pub(crate) fn mute_reason(&self, language: Option<&str>, reason: Option<&str>) -> String {
        match reason {
            Some(r) => r.to_owned(),
            None => self.tr(language, "pv.mutes.empty_reason", &[]),
        }
    }
    /// `MuteDurationUnit.translate`.
    pub(crate) fn duration_text(&self, language: Option<&str>, amount: i64, unit: crate::mutes::DurationUnit, now: i64) -> String {
        let n = if unit == crate::mutes::DurationUnit::Timestamp { (amount * 1_000 - now) / 1_000 } else { amount };
        self.tr(language, unit.key(), &[&n.to_string()])
    }
    /// `VoiceMuteManager.tick`: mutes that ran out end.
    fn expire_mutes(&mut self) {
        let now = now_millis();
        let expired: Vec<(Uuid, Option<bool>)> = self.mutes.all().filter(|m| !m.valid(now)).map(|m| (m.player, m.silent)).collect();
        for (id, silent) in expired {
            let silent = silent == Some(true) || !self.config.notify_unmuted;
            self.unmute(id, silent);
        }
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

/// The UDP thread (`NettyUdpServer`).
struct Udp {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Udp {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Where the server keeps its files, and the Minecraft server's port for `host.port = 0`.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// The server's Plasmo Voice folder (`config/plasmovoice/server`); None keeps everything in
    /// memory with the defaults.
    pub folder: Option<PathBuf>,
    /// The Minecraft server's port (`server-port`), or 0 where it has none (single player, a
    /// LAN world): `host.port = 0` then binds any free port.
    pub minecraft_port: u16,
}

/// A Plasmo Voice server.
pub struct Server {
    pub(crate) shared: Arc<Mutex<Shared>>,
    pub(crate) rsa: Arc<dyn RsaEncrypt>,
    options: Options,
    udp: Option<Udp>,
}

/// `loadConfig`: `config.toml` read (defaults when missing) and written back at once.
fn load_config(folder: Option<&Path>) -> Result<ServerConfig, String> {
    let Some(dir) = folder else { return Ok(ServerConfig::default()) };
    let path = dir.join("config.toml");
    let config = match std::fs::read_to_string(&path) {
        Ok(text) => ServerConfig::parse(&text).map_err(|e| format!("Failed to load config {}: {e}", path.display()))?,
        Err(_) => ServerConfig::default(),
    };
    let _ = std::fs::create_dir_all(dir);
    let text = config.write();
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str())
        && let Err(e) = std::fs::write(&path, text)
    {
        log::error!(target: crate::LOG, "[Plasmo Voice] Failed to save {}: {e}", path.display());
    }
    Ok(config)
}

fn load_languages(folder: Option<&Path>, config: &ServerConfig) -> Languages {
    let dir = folder.map(|d| d.join("languages"));
    let (langs, problems) = Languages::load(dir.as_deref(), &config.default_language, config.forced_language.as_deref());
    for p in problems {
        log::error!(target: crate::LOG, "[Plasmo Voice] {p}");
    }
    langs
}

impl Server {
    /// `BaseVoiceServer.onInitialize`: the config, the mutes, the languages, then the UDP server
    /// on `host.ip`:`host.port` (0: the Minecraft server's port, or any free one without it). A
    /// broken `config.toml` leaves the defaults in use and the file as it is.
    pub fn start(options: Options, rsa: Arc<dyn RsaEncrypt>) -> Server {
        let config = load_config(options.folder.as_deref()).unwrap_or_else(|e| {
            log::error!(target: crate::LOG, "[Plasmo Voice] {e}");
            ServerConfig::default()
        });
        let mutes = MuteStorage::load(options.folder.as_ref().map(|d| d.join("voice_mutes.json")), now_millis()).unwrap_or_else(|e| {
            log::error!(target: crate::LOG, "[Plasmo Voice] Failed to initialize mute storage: {e}");
            MuteStorage::default()
        });
        let langs = load_languages(options.folder.as_deref(), &config);
        let mut key = [0u8; 16];
        // `BaseVoiceServer`: the key is the bytes of a random UUID.
        key.copy_from_slice(&random_uuid().0.to_be_bytes());
        let shared = Arc::new(Mutex::new(Shared {
            activation: proximity_activation(&config),
            line: proximity_line(&config),
            config,
            langs,
            mutes,
            key,
            port: None,
            players: HashMap::new(),
            profiles: HashMap::new(),
            out: Vec::new(),
            opened: Vec::new(),
            tells: Vec::new(),
            kicks: Vec::new(),
            last_mute_tick: Instant::now(),
        }));
        let mut server = Server { shared, rsa, options, udp: None };
        server.start_udp();
        server
    }

    /// `startUdpServer`.
    fn start_udp(&mut self) {
        self.udp = None;
        let (ip, mut port) = {
            let s = lock(&self.shared);
            (s.config.host.ip.clone(), s.config.host.port)
        };
        if port == 0 {
            port = self.options.minecraft_port;
        }
        let bound = UdpSocket::bind((ip.as_str(), port)).and_then(|socket| {
            socket.set_read_timeout(Some(Duration::from_millis(100)))?;
            let port = socket.local_addr()?.port();
            Ok((socket, port))
        });
        let (socket, port) = match bound {
            Ok(b) => b,
            Err(e) => {
                log::error!(target: crate::LOG, "[Plasmo Voice] Failed to start the udp server on {ip}:{port}: {e}");
                lock(&self.shared).port = None;
                return;
            }
        };
        lock(&self.shared).port = Some(port);
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (shared, stop) = (self.shared.clone(), stop.clone());
            std::thread::Builder::new().name("plasmovoice-server".into()).spawn(move || udp_loop(&shared, &stop, &socket))
        };
        match thread {
            Ok(t) => {
                log::info!(target: crate::LOG, "[Plasmo Voice] UDP server is started on {ip}:{port}");
                self.udp = Some(Udp { stop, thread: Some(t) });
            }
            Err(e) => {
                log::error!(target: crate::LOG, "[Plasmo Voice] Failed to start the udp server: {e}");
                lock(&self.shared).port = None;
            }
        }
    }

    /// The UDP port bound, 0 when the UDP server isn't running.
    pub fn port(&self) -> u16 {
        lock(&self.shared).port.unwrap_or(0)
    }

    /// The settings in use.
    pub fn config(&self) -> ServerConfig {
        lock(&self.shared).config.clone()
    }

    /// Names of players who aren't on (the server's `usercache.json`), for `/vmutelist` and
    /// `/vunmute`.
    pub fn remember(&mut self, profiles: impl IntoIterator<Item = (Uuid, String)>) {
        lock(&self.shared).profiles.extend(profiles);
    }

    /// A player joined: they are asked for their info, again after 1, 3, 5, 10 and 15 s.
    pub fn join(&mut self, id: Uuid, name: &str) {
        let mut s = lock(&self.shared);
        s.profiles.insert(id, name.to_owned());
        let required = s.config.voice.client_mod_required && s.port.is_some();
        let timeout = Duration::from_millis(s.config.voice.client_mod_required_check_timeout_ms.max(0) as u64);
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
                mc_language: None,
                perms: Perms::default(),
                kick_at: required.then(|| Instant::now() + timeout),
            },
        );
        s.request_info(id);
    }

    pub fn quit(&mut self, id: Uuid) {
        let mut s = lock(&self.shared);
        s.disconnect(id);
        s.players.remove(&id);
    }

    /// The player's game language and permissions, as the game reads them now. A permission
    /// that changed is told to their client (`onPermissionUpdate`).
    pub fn set_player(&mut self, id: Uuid, mc_language: Option<String>, perms: Perms) {
        let mut s = lock(&self.shared);
        let Some(p) = s.players.get_mut(&id) else { return };
        if mc_language.is_some() {
            p.mc_language = mc_language;
        }
        let old = std::mem::replace(&mut p.perms, perms);
        let voice = p.connection.is_some();
        if !voice || old == perms {
            return;
        }
        if old.proximity != perms.proximity {
            let packet = if perms.proximity {
                Packet::ActivationRegister(s.activation.clone())
            } else {
                Packet::ActivationUnregister { activation: s.activation.id() }
            };
            s.send(id, &packet);
        }
        if old.allow_freecam != perms.allow_freecam {
            let permissions = s.permissions(id);
            s.send(id, &Packet::ConfigPlayerInfo { permissions });
        }
    }

    /// Each server tick: positions, and the retries, rate-limited replies, mute expiries and
    /// kicks that are due.
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
            if let Some(p) = s.players.get(&id)
                && let Some(c) = &p.connection
            {
                log::info!(target: crate::LOG, "[Plasmo Voice] {} ({}) connected", p.name, c.addr);
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
            let kick = p.kick_at.is_some_and(|t| now >= t);
            if kick {
                p.kick_at = None;
            }
            let kick = kick && !p.perms.bypass_mod_requirement;
            if request {
                s.request_info(id);
            }
            if state_due && s.players.get(&id).is_some_and(|p| p.connection.is_some()) {
                s.broadcast_info(id);
            }
            if language_due {
                s.send_language(id);
            }
            if kick {
                let lang = s.player_language(id);
                let text = s.tr(lang.as_deref(), "pv.error.mod_missing_kick_message", &[]);
                s.kicks.push((id, text));
            }
        }
        if now.duration_since(s.last_mute_tick) >= MUTE_TICK {
            s.last_mute_tick = now;
            s.expire_mutes();
        }
    }

    /// A `plasmo:voice/v2` payload from a player (`PlayerChannelHandler`).
    pub fn handle_tcp(&mut self, from: Uuid, data: &[u8]) {
        let Ok(Some(packet)) = Packet::decode(data, Direction::Server) else { return };
        let mut s = lock(&self.shared);
        if s.port.is_none() {
            return;
        }
        let min_version = s.config.voice.client_mod_min_version.clone();
        let Some(p) = s.players.get_mut(&from) else { return };
        // Any packet ends the info request retries and the mod requirement's wait.
        p.request = None;
        p.kick_at = None;
        let has_voice = p.connection.is_some();
        match packet {
            Packet::PlayerInfo { voice_disabled, microphone_muted, version, public_key, minecraft_version } => {
                let (Some(client), Some(server)) = (semver(&version), semver(SERVER_VERSION)) else { return };
                let min = semver(&min_version).unwrap_or((2, 0, 0));
                if client.0 != server.0 || client < min {
                    // `ServerVersionUtil.suggestSupportedVersion`.
                    let ok = !minecraft_version.is_empty()
                        && minecraft_version.len() <= 32
                        && minecraft_version.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c));
                    let link = if ok { format!("{MODRINTH_LINK}/versions?g={minecraft_version}") } else { MODRINTH_LINK.to_owned() };
                    let lang = s.player_language(from);
                    let click = s.tr(lang.as_deref(), "pv.error.version_not_supported_click", &[]);
                    s.tell(from, "pv.error.version_not_supported", &[&format!("{click} ({link})")]);
                    return;
                }
                p.public_key = Some(public_key);
                p.voice_disabled = voice_disabled;
                p.microphone_muted = microphone_muted;
                let secret = p.secret;
                // `connect`: `[host.public]`'s address when set, its port 0 meaning the host's.
                let host = &s.config.host;
                let ip = host.public.as_ref().map_or(host.ip.clone(), |p| p.ip.clone());
                let mut port = host.public.as_ref().map_or(host.port, |p| p.port);
                if port == 0 {
                    port = host.port;
                }
                if port == 0 {
                    port = s.port.unwrap_or(0);
                }
                s.send(from, &Packet::Connection { secret, ip, port: i32::from(port) });
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
                let mut visualize = None;
                if let Some(p) = s.players.get_mut(&from) {
                    for (activation, distance) in distances {
                        if activation == known {
                            // `onActivationDistanceChange`: shown when it changed from a known one.
                            if p.distances.insert(activation, distance).is_some() {
                                visualize = Some(distance);
                            }
                        }
                    }
                }
                if let Some(radius) = visualize {
                    s.send(from, &Packet::DistanceVisualize { radius, color: VISUALIZE_COLOR, position: None });
                }
            }
            Packet::PlayerAudioEnd { sequence, activation, distance } => {
                if !has_voice || p.microphone_muted || !p.perms.proximity || activation != s.activation.id() || s.mutes.get(from).is_some() {
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

    /// Chat lines for players (the mute notices, a version too old), each in their language.
    pub fn take_tells(&mut self) -> Vec<(Uuid, String)> {
        std::mem::take(&mut lock(&self.shared).tells)
    }

    /// Players to kick (`client_mod_required`), with the reason in their language.
    pub fn take_kicks(&mut self) -> Vec<(Uuid, String)> {
        std::mem::take(&mut lock(&self.shared).kicks)
    }

    /// Players with voice chat (for a player list).
    pub fn players(&self) -> Vec<PlayerInfo> {
        let s = lock(&self.shared);
        s.connected().filter_map(|id| s.info(id)).collect()
    }

    /// Whether the player is muted on the server.
    pub fn muted(&self, id: Uuid) -> bool {
        lock(&self.shared).mutes.get(id).is_some()
    }

    /// `loadConfig(true)` and the reload command's Config to everyone with voice chat: the file
    /// read again (an error keeps what was in use), the languages, the proximity activation
    /// registered again with the new distances, and the UDP server restarted when `[host]`
    /// changed (its players then asked for their info again).
    pub fn reload(&mut self) -> Result<(), String> {
        let config = load_config(self.options.folder.as_deref())?;
        let langs = load_languages(self.options.folder.as_deref(), &config);
        let restart = {
            let mut s = lock(&self.shared);
            let restart = s.config.host != config.host || s.config.voice.reuse_port != config.voice.reuse_port || s.port.is_none();
            s.langs = langs;
            // `ProximityServerActivation.register`: the old activation and line unregistered,
            // the new activation registered.
            let (old_activation, old_line) = (s.activation.id(), s.line.id());
            let ids: Vec<Uuid> = s.players.keys().copied().collect();
            for id in &ids {
                if let Some(p) = s.players.get_mut(id) {
                    p.distances.remove(&old_activation);
                    p.active.remove(&old_activation);
                }
            }
            s.broadcast_if(&Packet::ActivationUnregister { activation: old_activation }, |p| p.perms.proximity);
            s.broadcast(&Packet::SourceLineUnregister { line: old_line });
            for id in &ids {
                if let Some(p) = s.players.get_mut(id) {
                    p.source = None;
                }
            }
            s.activation = proximity_activation(&config);
            s.line = proximity_line(&config);
            s.config = config;
            let activation = s.activation.clone();
            s.broadcast_if(&Packet::ActivationRegister(activation), |p| p.perms.proximity);
            restart
        };
        if restart {
            let connected: Vec<Uuid> = lock(&self.shared).connected().collect();
            self.udp = None;
            {
                let mut s = lock(&self.shared);
                for id in &connected {
                    s.disconnect(*id);
                }
            }
            self.start_udp();
            let mut s = lock(&self.shared);
            if s.port.is_some() {
                for id in connected {
                    s.request_info(id);
                }
            }
        }
        let mut s = lock(&self.shared);
        let voice: Vec<Uuid> = s.connected().collect();
        for id in voice {
            s.send_config(id, &*self.rsa);
        }
        Ok(())
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
    log::info!(target: crate::LOG, "[Plasmo Voice] UDP server is stopped");
}

/// `NettyUdpKeepAlive.tick`.
fn keep_alive(s: &mut Shared, socket: &UdpSocket) {
    let now = Instant::now();
    let timeout = Duration::from_millis(s.config.voice.keep_alive_timeout_ms.max(0) as u64);
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
    let muted = s.mutes.get(id).is_some();
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
    if let udp::Packet::PlayerAudio { sequence, data, activation, distance, stereo: _ } = d.packet {
        // `NettyUdpServerConnection.handle`: muted on the server or by themselves, nothing goes
        // out; `onPlayerSpeak`: the activation must be one the player may use.
        if muted || p.microphone_muted || !p.perms.proximity || activation != s.activation.id() {
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
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// No real RSA in tests: the "encrypted" key is the key.
    pub(crate) struct Plain;
    impl RsaEncrypt for Plain {
        fn encrypt(&self, _: &[u8], data: &[u8]) -> Option<Vec<u8>> {
            Some(data.to_vec())
        }
    }

    pub(crate) struct Fake {
        pub(crate) id: Uuid,
        pub(crate) sock: UdpSocket,
        pub(crate) secret: Uuid,
    }

    pub(crate) fn local() -> Server {
        let mut s = Server::start(Options::default(), Arc::new(Plain));
        // Tests talk to it on loopback, whatever `host.ip` binds.
        assert!(s.port() != 0);
        s.remember([]);
        s
    }

    pub(crate) fn tcp_to(server: &mut Server, id: Uuid) -> Vec<Packet> {
        let out = server.take_tcp();
        let mut mine = Vec::new();
        for (to, data) in out {
            if to == id {
                mine.push(Packet::decode(&data, Direction::Client).unwrap().unwrap());
            }
        }
        mine
    }

    pub(crate) fn connect(server: &mut Server, id: Uuid, name: &str) -> Fake {
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

    pub(crate) fn at(world: &str, x: f64) -> Position {
        Position { world: world.into(), pos: [x, 64.0, 0.0] }
    }

    pub(crate) fn speak(f: &Fake, sequence: i64) {
        let audio = udp::Packet::PlayerAudio { sequence, data: vec![9; 40], activation: Activation::id_of(PROXIMITY), distance: 16, stereo: false };
        f.sock.send(&audio.encode(f.secret, 0)).unwrap();
    }

    /// Whether `f` hears audio within the socket's timeout.
    pub(crate) fn hears(f: &Fake) -> Option<udp::Packet> {
        let mut buf = [0u8; 2048];
        loop {
            match f.sock.recv(&mut buf) {
                Ok(n) => {
                    let d = udp::Packet::decode(&buf[..n]).unwrap().unwrap();
                    if let udp::Packet::SourceAudio { .. } = d.packet {
                        assert_eq!(d.secret, f.secret);
                        return Some(d.packet);
                    }
                }
                Err(_) => return None,
            }
        }
    }

    #[test]
    fn relays_to_players_in_range() {
        let mut server = local();
        let a = connect(&mut server, Uuid(1), "alice");
        let b = connect(&mut server, Uuid(2), "bob");
        let c = connect(&mut server, Uuid(3), "carol");
        server.update(&[(a.id, at("overworld", 0.0)), (b.id, at("overworld", 10.0)), (c.id, at("overworld", 100.0))]);
        server.take_tcp();
        // Alice speaks at 16 blocks: Bob (10 away) hears, Carol (100 away) doesn't.
        speak(&a, 1);
        let got = hears(&b).expect("bob hears");
        let udp::Packet::SourceAudio { sequence, data, state, distance, source } = got else { unreachable!() };
        assert_eq!((sequence, data, state, distance), (1, vec![9; 40], 1, 16));
        // Bob was told whose source it is.
        let info = tcp_to(&mut server, b.id);
        assert!(info.iter().any(|p| matches!(p, Packet::SourceInfo(i) if i.id == source && matches!(&i.kind, SourceKind::Player(p) if p.nick == "alice"))));
        c.sock.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
        assert!(hears(&c).is_none(), "carol heard");
        // The end goes over TCP to the listeners.
        server.handle_tcp(a.id, &Packet::PlayerAudioEnd { sequence: 2, activation: Activation::id_of(PROXIMITY), distance: 16 }.encode());
        assert!(tcp_to(&mut server, b.id).iter().any(|p| matches!(p, Packet::SourceAudioEnd { source: s, sequence: 2 } if *s == source)));
    }

    #[test]
    fn without_the_permission_nobody_hears() {
        let mut server = local();
        let a = connect(&mut server, Uuid(1), "alice");
        let b = connect(&mut server, Uuid(2), "bob");
        server.update(&[(a.id, at("overworld", 0.0)), (b.id, at("overworld", 1.0))]);
        server.take_tcp();
        server.set_player(a.id, None, Perms { proximity: false, ..Perms::default() });
        assert!(tcp_to(&mut server, a.id).iter().any(|p| matches!(p, Packet::ActivationUnregister { .. })));
        b.sock.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
        speak(&a, 1);
        assert!(hears(&b).is_none());
        server.set_player(a.id, None, Perms::default());
        speak(&a, 2);
        assert!(hears(&b).is_some());
    }

    #[test]
    fn old_or_other_major_clients_are_ignored() {
        let mut server = local();
        server.join(Uuid(1), "x");
        server.take_tcp();
        for version in ["1.0.9", "3.0.0", "garbage"] {
            let info = Packet::PlayerInfo { voice_disabled: false, microphone_muted: false, minecraft_version: "26.3".into(), version: version.into(), public_key: vec![1] };
            server.handle_tcp(Uuid(1), &info.encode());
            assert!(server.take_tcp().is_empty(), "{version}");
        }
        // A 1.x client is told where to get a supported version.
        let tells = server.take_tells();
        assert!(tells.iter().any(|(id, t)| *id == Uuid(1) && t.contains("not supported")), "{tells:?}");
    }

    #[test]
    fn public_host_in_the_connection_packet() {
        let dir = std::env::temp_dir().join(format!("pv-public-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.toml"), "[host]\nip = \"127.0.0.1\"\n[host.public]\nip = \"voice.example.org\"\nport = 0\n").unwrap();
        let mut server = Server::start(Options { folder: Some(dir.clone()), minecraft_port: 0 }, Arc::new(Plain));
        server.join(Uuid(1), "x");
        server.take_tcp();
        let info = Packet::PlayerInfo { voice_disabled: false, microphone_muted: false, minecraft_version: "26.3".into(), version: "2.2.0".into(), public_key: vec![1] };
        server.handle_tcp(Uuid(1), &info.encode());
        // host.port 0 and public.port 0: the port bound.
        let port = i32::from(server.port());
        assert!(matches!(&tcp_to(&mut server, Uuid(1))[..], [Packet::Connection { ip, port: p, .. }] if ip == "voice.example.org" && *p == port));
        // The file was written back whole, as Plasmo Voice does.
        let text = std::fs::read_to_string(dir.join("config.toml")).unwrap();
        assert!(text.starts_with("# Used to store server-related config file on the client\n"));
        assert!(text.contains("[host.public]\nip = \"voice.example.org\"\nport = 0\n"));
        assert!(dir.join("languages").join("ru_ru.toml").is_file());
        drop(server);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn distances_and_versions() {
        let a = proximity_activation(&ServerConfig::default());
        assert_eq!(allowed_distance(&a, 32), 32);
        assert_eq!(allowed_distance(&a, 31), 16);
        assert_eq!(semver("2.1.8-SNAPSHOT"), Some((2, 1, 8)));
    }
}
