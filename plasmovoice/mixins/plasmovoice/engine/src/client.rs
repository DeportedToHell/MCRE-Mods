//! The voice chat client, as Plasmo Voice's (`ModServerConnection`, `NettyUdpClient`,
//! `VoiceAudioCapture`, `VoiceClientActivation`, `BaseClientAudioSource`, `StaticJitterBuffer`).
//!
//! The game hands it the `plasmo:voice/v2` payloads it receives (`handle_tcp`) and sends what
//! `take_tcp` gives, and each frame tells it where the listener and the players are (`update`).
//! Three threads do the rest: the UDP connection, the microphone and the speakers.
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use crate::audio::{Capture, Decoder, Encoder, Mix, Output};
use crate::crypto::AesKey;
use crate::dsp;
use crate::tcp::{Activation, Direction, Packet, PlayerInfo, SourceInfo, SourceKind, SourceLine};
use crate::udp;
use crate::wire::Uuid;
use std::collections::{BTreeMap, HashMap};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The client's RSA key pair, made by the game (Horizon's own RSA): the X.509 public key
/// (`PublicKey.getEncoded`) and PKCS#1 v1.5 decryption (`Cipher.getInstance("RSA")`).
pub trait Rsa: Send + Sync {
    fn public_der(&self) -> Vec<u8>;
    fn decrypt(&self, data: &[u8]) -> Option<Vec<u8>>;
}

/// How an activation starts (`ClientActivation.Type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivationType {
    PushToTalk,
    Voice,
    Inherit,
}

impl ActivationType {
    pub fn name(self) -> &'static str {
        match self {
            ActivationType::PushToTalk => "PUSH_TO_TALK",
            ActivationType::Voice => "VOICE",
            ActivationType::Inherit => "INHERIT",
        }
    }
    pub fn parse(s: &str) -> Option<ActivationType> {
        [ActivationType::PushToTalk, ActivationType::Voice, ActivationType::Inherit].into_iter().find(|t| t.name() == s)
    }
}

/// The player's own settings for one activation (`ConfigClientActivation`), by its name.
#[derive(Clone, Debug, PartialEq)]
pub struct ActivationSettings {
    pub kind: ActivationType,
    /// Voice activation switched off with its toggle key.
    pub toggle: bool,
    /// The distance chosen, none for the server's default.
    pub distance: Option<i32>,
}

impl Default for ActivationSettings {
    fn default() -> Self {
        ActivationSettings { kind: ActivationType::PushToTalk, toggle: false, distance: None }
    }
}

/// `OverlappingSourceTypes`: which of a player's two sources plays when they talk on both a
/// proximity (player) source and a direct one (an addon's radio, a broadcast) at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlap {
    MuteDirect,
    MuteProximity,
    Off,
}

impl Overlap {
    pub fn parse(s: &str) -> Overlap {
        match s {
            "MUTE_DIRECT" => Overlap::MuteDirect,
            "OFF" => Overlap::Off,
            _ => Overlap::MuteProximity,
        }
    }
}

/// The settings the engine reads (`VoiceClientConfig`), with Plasmo Voice's defaults (but the
/// activation threshold: -60 dB in this port, -30 dB in Plasmo Voice).
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub voice_disabled: bool,
    pub microphone_disabled: bool,
    pub activation_threshold: f64,
    pub input_device: String,
    pub output_device: String,
    pub microphone_volume: f64,
    /// `voice.noise_suppression`: RNNoise on the microphone (`NoiseSuppressionFilter`).
    pub noise_suppression: bool,
    /// `voice.stereo_capture`: the microphone opened in stereo; activations that take stereo
    /// get it, the rest the mix of both channels (`StereoToMonoFilter`).
    pub stereo_capture: bool,
    pub volume: f64,
    /// `voice.sound_occlusion`: blocks between a source and the player quieten it
    /// (`SoundOcclusion`, the game measures it: `World::occlusion`).
    pub sound_occlusion: bool,
    /// `voice.directional_sources` and `advanced.directional_sources_angle`: players and
    /// entities are quieter behind where they look, outside a cone of that angle.
    pub directional_sources: bool,
    pub directional_sources_angle: i32,
    /// `advanced.stereo_sources_to_mono`: stereo sources are mixed down and placed in the world.
    pub stereo_sources_to_mono: bool,
    /// `advanced.source_types_overlap`.
    pub source_types_overlap: Overlap,
    /// `advanced.camera_sound_listener`: the ears are the camera's (third person included), if
    /// the server allows it (`pv.allow_freecam`); else the player's.
    pub camera_sound_listener: bool,
    pub compressor_limiter: bool,
    pub compressor_threshold: i32,
    pub limiter_threshold: i32,
    pub exponential_volume_slider: bool,
    pub exponential_distance_gain: bool,
    pub panning: bool,
    pub jitter_packet_delay: i32,
    /// Per source line, by name (`voice.volumes`, `source_line_<name>`).
    pub line_volumes: HashMap<String, f64>,
    /// Per player, by UUID.
    pub player_volumes: HashMap<Uuid, f64>,
    pub muted_players: Vec<Uuid>,
    /// Source lines muted on the Volume tab (`voice.volumes.<line>.muted`).
    pub muted_lines: Vec<String>,
    pub activations: HashMap<String, ActivationSettings>,
    /// `voice.disable_input_device`: no microphone is opened at all.
    pub disable_input_device: bool,
    /// The settings' microphone test (`MicrophoneTestController`): the processed microphone
    /// plays back on the speakers and no activation sends.
    pub microphone_test: bool,
    /// The game's language, asked of the server for its texts (`LanguageRequestPacket`).
    pub language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            voice_disabled: false,
            microphone_disabled: false,
            activation_threshold: -60.0,
            input_device: String::new(),
            output_device: String::new(),
            microphone_volume: 1.0,
            noise_suppression: false,
            stereo_capture: false,
            volume: 1.0,
            sound_occlusion: false,
            directional_sources: false,
            directional_sources_angle: 145,
            stereo_sources_to_mono: false,
            source_types_overlap: Overlap::MuteProximity,
            camera_sound_listener: true,
            compressor_limiter: true,
            compressor_threshold: -10,
            limiter_threshold: -6,
            exponential_volume_slider: true,
            exponential_distance_gain: true,
            panning: true,
            jitter_packet_delay: 2,
            line_volumes: HashMap::new(),
            player_volumes: HashMap::new(),
            muted_players: Vec::new(),
            muted_lines: Vec::new(),
            activations: HashMap::new(),
            disable_input_device: false,
            microphone_test: false,
            language: "en_us".into(),
        }
    }
}

/// What the game knows this frame.
#[derive(Clone, Debug, Default)]
pub struct World {
    /// The player's eyes (`getListenerPosition`: what distances are measured from) and where
    /// they look, yaw and pitch in degrees (Minecraft's).
    pub listener: [f64; 3],
    pub listener_yaw: f32,
    pub listener_pitch: f32,
    /// The camera this frame (position, yaw, pitch): the ears with `camera_sound_listener`.
    pub camera: Option<([f64; 3], f32, f32)>,
    /// Players' eye positions by UUID, and entities' by id, and where they look (unit vectors,
    /// `Entity.getLookAngle`).
    pub players: HashMap<Uuid, [f64; 3]>,
    pub entities: HashMap<i32, [f64; 3]>,
    pub player_looks: HashMap<Uuid, [f64; 3]>,
    pub entity_looks: HashMap<i32, [f64; 3]>,
    /// Each source's occlusion (0..0.98, `SoundOcclusion.getOccludedPercent` between it and the
    /// player's eyes), by source id, measured by the game while sound occlusion is on.
    pub occlusion: HashMap<Uuid, f64>,
    /// The push-to-talk keys held, by activation name.
    pub held: Vec<String>,
    /// A screen with text input is open (chat, a sign): push to talk doesn't count.
    pub typing: bool,
    /// The game is in a world (sources are cleared when it isn't).
    pub in_world: bool,
    /// The game's own player, for the server's mute of it (`isServerMuted`).
    pub me: Option<Uuid>,
}

/// What the HUD shows.
#[derive(Clone, Debug, Default)]
pub struct Status {
    /// The UDP connection works (a Config came and the server answers pings).
    pub connected: bool,
    pub timed_out: bool,
    /// The activations sending now, by name, with their icons.
    pub talking: Vec<(String, String)>,
    pub server_muted: bool,
    /// Every source the client knows, with whether it is heard now.
    pub sources: Vec<SourceStatus>,
    /// The microphone's last processed frame level (dB, `AudioUtil.calculateHighestAudioLevel`)
    /// and how many frames came so far, for the settings' meter.
    pub level: f64,
    pub frames: u64,
    /// The microphone open now, and why none is.
    pub input_device: Option<String>,
    pub input_error: Option<String>,
    /// The server's id (`ServerInfo.serverId`) and its player icon settings (`PlayerIconConfig`).
    pub server_id: Option<Uuid>,
    pub icon_hidden: Vec<String>,
    pub icon_offset: [f64; 3],
    pub error: Option<String>,
    /// Where the voice UDP goes.
    pub address: Option<SocketAddr>,
    /// 20 ms frames played so far, the loudest sample put into the mix in the last second (0..1),
    /// and the speakers' name.
    pub played: u64,
    pub output_peak: f32,
    pub speakers: Option<String>,
    /// Players with the mod (PlayerList), with their states.
    pub players: Vec<PlayerInfo>,
}

/// A source as the HUD sees it (`ClientAudioSource`).
#[derive(Clone, Debug)]
pub struct SourceStatus {
    pub id: Uuid,
    pub kind: SourceKind,
    /// `SourceInfo.name`, an addon's label.
    pub name: Option<String>,
    pub line: String,
    pub line_icon: String,
    pub line_weight: i32,
    pub icon_visible: bool,
    /// `isActivated`: audio came within the last 500 ms.
    pub activated: bool,
}

// ---- shared state ---------------------------------------------------------------------------

#[derive(Default)]
struct Server {
    secret: Uuid,
    addr: Option<SocketAddr>,
    server_id: Uuid,
    sample_rate: i32,
    mtu: i32,
    codec_mode: String,
    bitrate: i32,
    key: Option<AesKey>,
    lines: Vec<SourceLine>,
    activations: Vec<Activation>,
    permissions: Vec<(String, bool)>,
    icons: Option<crate::tcp::IconConfig>,
    configured: bool,
    last_ping: Option<Instant>,
}

struct Source {
    info: Option<SourceInfo>,
    jitter: Jitter,
    decoder: Option<Decoder>,
    last_audio: Instant,
    distance: i16,
    state: i8,
    requested: bool,
    /// `lastOcclusion`: the occlusion heard last (-1 before any), which moves toward the
    /// measured one by 0.05 a frame.
    last_occlusion: f64,
    /// `activated`: audio plays since the last end; the first frame after one fades in.
    playing: bool,
}

/// A sphere of the voice distance to draw (`DistanceVisualizer.render`): its radius, colour
/// (RGB), centre (none: the player), and whether it is the one shown on joining.
#[derive(Clone, Debug, PartialEq)]
pub struct Visualize {
    pub radius: i32,
    pub color: i32,
    pub position: Option<[f64; 3]>,
    pub on_join: bool,
}

#[derive(Default)]
struct Shared {
    settings: Settings,
    world: World,
    server: Server,
    players: Vec<PlayerInfo>,
    sources: HashMap<Uuid, Source>,
    tcp_out: Vec<Vec<u8>>,
    /// Activations sending now, by id.
    talking: Vec<Uuid>,
    level: f64,
    frames: u64,
    input_device: Option<String>,
    input_error: Option<String>,
    /// The speakers' mix, for the microphone test's loopback.
    mix: Option<Arc<Mutex<Mix>>>,
    error: Option<String>,
    language: Vec<(String, String)>,
    played: u64,
    output_peak: (f32, Option<Instant>),
    speakers: Option<String>,
    visualize: Vec<Visualize>,
}

// ---- the jitter buffer ------------------------------------------------------------------------

/// A per-source jitter buffer after `StaticJitterBuffer`: packets wait `delay` frames, play in
/// sequence order, a missing one is concealed (at most five in a row), stale ones are dropped.
#[derive(Default)]
struct Jitter {
    queue: BTreeMap<i64, (Vec<u8>, Instant)>,
    end: Option<i64>,
    anchor: Option<(Instant, i64)>,
    last: i64,
    plc_in_a_row: u32,
    last_arrival: Option<Instant>,
}

enum Frame {
    /// A packet, and whether the stream's end comes right after it (it fades out).
    Audio(Vec<u8>, bool),
    Lost,
    End,
}

impl Jitter {
    fn reset(&mut self) {
        *self = Jitter { last: -1, ..Jitter::default() };
    }
    fn offer(&mut self, seq: i64, data: Vec<u8>, now: Instant) {
        if self.end.is_some_and(|e| seq > e) {
            self.reset();
        }
        if self.queue.len() >= 100 {
            return;
        }
        let restart = self.last_arrival.is_some_and(|t| now.duration_since(t) >= Duration::from_millis(200)) && seq > self.last && self.queue.is_empty();
        self.last_arrival = Some(now);
        if self.anchor.is_none() || restart {
            self.anchor = Some((now, seq));
            self.last = seq - 1;
            self.plc_in_a_row = 0;
        }
        self.queue.insert(seq, (data, now));
    }
    fn end(&mut self, seq: i64) {
        if self.last < seq {
            self.end = Some(seq);
        }
    }
    /// The next frame to play, once its time has come.
    fn poll(&mut self, delay_ms: u64, now: Instant) -> Option<Frame> {
        // Stale packets (over 500 ms old) never play.
        self.queue.retain(|_, (_, t)| now.duration_since(*t) < Duration::from_millis(500));
        let (anchor_time, anchor_seq) = self.anchor?;
        let next = self.last + 1;
        let due = anchor_time + Duration::from_millis(delay_ms + ((next - anchor_seq).max(0) as u64) * 20);
        if now < due {
            return None;
        }
        if let Some((data, _)) = self.queue.remove(&next) {
            self.last = next;
            self.plc_in_a_row = 0;
            return Some(Frame::Audio(data, self.end == Some(next + 1)));
        }
        if self.end.is_some_and(|e| next >= e) {
            self.reset();
            return Some(Frame::End);
        }
        // Drop anything older than what plays next.
        let stale: Vec<i64> = self.queue.range(..next).map(|(k, _)| *k).collect();
        for k in stale {
            self.queue.remove(&k);
        }
        if let Some((&head, _)) = self.queue.iter().next() {
            if self.plc_in_a_row >= 5 {
                // Too many lost: skip to what came.
                self.anchor = Some((now - Duration::from_millis(delay_ms), head));
                self.last = head - 1;
                self.plc_in_a_row = 0;
                return None;
            }
            self.last = next;
            self.plc_in_a_row += 1;
            return Some(Frame::Lost);
        }
        None
    }
}

// ---- the client -----------------------------------------------------------------------------

fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// A Plasmo Voice client for one server connection.
pub struct Client {
    shared: Arc<Mutex<Shared>>,
    rsa: Arc<dyn Rsa>,
    minecraft_version: String,
    /// The Minecraft server's host, for a `Connection` with ip "0.0.0.0", and its port.
    server_host: String,
    server_port: u16,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl Client {
    pub fn new(rsa: Arc<dyn Rsa>, minecraft_version: &str, server_host: &str, server_port: u16, settings: Settings) -> Client {
        let shared = Shared { settings, ..Shared::default() };
        Client {
            shared: Arc::new(Mutex::new(shared)),
            rsa,
            minecraft_version: minecraft_version.into(),
            server_host: server_host.into(),
            server_port,
            stop: Arc::new(AtomicBool::new(false)),
            threads: Vec::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Shared> {
        lock(&self.shared)
    }

    /// A `plasmo:voice/v2` payload from the server.
    pub fn handle_tcp(&mut self, data: &[u8]) {
        let packet = match Packet::decode(data, Direction::Client) {
            Ok(Some(p)) => p,
            Ok(None) => return,
            Err(e) => {
                self.lock().error = Some(format!("bad packet: {e}"));
                return;
            }
        };
        match packet {
            Packet::PlayerInfoRequest => {
                let mut s = self.lock();
                let reply = Packet::PlayerInfo {
                    voice_disabled: s.settings.voice_disabled,
                    microphone_muted: s.settings.microphone_disabled,
                    minecraft_version: self.minecraft_version.clone(),
                    version: crate::PROTOCOL_VERSION.into(),
                    public_key: self.rsa.public_der(),
                };
                s.tcp_out.push(reply.encode());
            }
            Packet::Connection { secret, ip, port } => {
                let host = if ip == "0.0.0.0" { self.server_host.clone() } else { ip };
                let port = if port == 0 { self.server_port } else { port as u16 };
                // IPv4 first: "localhost" resolves to ::1 first on Windows, and servers bind 0.0.0.0.
                let addr = (host.as_str(), port).to_socket_addrs().ok().and_then(|a| {
                    let all: Vec<SocketAddr> = a.collect();
                    all.iter().find(|a| a.is_ipv4()).or(all.first()).copied()
                });
                {
                    let mut s = self.lock();
                    s.server.secret = secret;
                    s.server.addr = addr;
                    s.server.configured = false;
                    // A new UDP client (`removeClient(RECONNECT)` then `connect`): it says hello
                    // again until the server answers (`/vrc`, a keep-alive timeout).
                    s.server.last_ping = None;
                    if addr.is_none() {
                        s.error = Some(format!("can't resolve {host}:{port}"));
                    }
                }
                if addr.is_some() {
                    self.start();
                }
            }
            Packet::Config { server_id, capture, encryption, source_lines, activations, permissions, icon_config } => {
                let mut s = self.lock();
                s.server.server_id = server_id;
                s.server.sample_rate = capture.sample_rate;
                s.server.mtu = capture.mtu_size;
                let codec = capture.encoder.clone().unwrap_or_default();
                s.server.codec_mode = codec.param("mode").unwrap_or("VOIP").to_owned();
                s.server.bitrate = codec.param("bitrate").and_then(|b| b.parse().ok()).unwrap_or(-1000);
                s.server.key = encryption.and_then(|e| self.rsa.decrypt(&e.data)).and_then(|k| AesKey::new(&k));
                s.server.lines = source_lines;
                s.server.activations = activations;
                s.server.permissions = permissions;
                s.server.icons = icon_config;
                s.server.configured = true;
                // LanguageRequestPacket and PlayerActivationDistancesPacket follow (`ModServerConnection`).
                let lang = Packet::LanguageRequest { language: s.settings.language.clone() }.encode();
                s.tcp_out.push(lang);
                let distances = activation_distances(&s);
                s.tcp_out.push(Packet::PlayerActivationDistances { distances }.encode());
                // `DistanceVisualizeOnJoinListener`: the proximity activation registered before
                // the UDP connection answered (a join, not a /vreload). The game puts the
                // player's distance on it and draws it if the settings say so.
                if s.server.last_ping.is_none()
                    && let Some(a) = s.server.activations.iter().find(|a| a.name == crate::tcp::PROXIMITY)
                {
                    let radius = distance_of(&s, a);
                    s.visualize.push(Visualize { radius, color: 0x00a000, position: None, on_join: true });
                }
            }
            Packet::DistanceVisualize { radius, color, position } => {
                self.lock().visualize.push(Visualize { radius, color, position: position.map(|p| [p.x, p.y, p.z]), on_join: false });
            }
            Packet::ConfigPlayerInfo { permissions } => self.lock().server.permissions = permissions,
            Packet::Language { entries, .. } => self.lock().language = entries,
            Packet::PlayerList { players } => self.lock().players = players,
            Packet::PlayerInfoUpdate { player } => {
                let mut s = self.lock();
                match s.players.iter_mut().find(|p| p.id == player.id) {
                    Some(p) => *p = player,
                    None => s.players.push(player),
                }
            }
            Packet::PlayerDisconnect { player } => self.lock().players.retain(|p| p.id != player),
            Packet::SourceInfo(info) => {
                let mut s = self.lock();
                let entry = s.sources.entry(info.id).or_insert_with(new_source);
                // A source that turned stereo or mono gets a new decoder (`update`).
                if entry.info.as_ref().is_some_and(|i| i.stereo != info.stereo) {
                    entry.decoder = None;
                }
                entry.state = info.state;
                entry.requested = false;
                entry.info = Some(info);
            }
            Packet::SelfSourceInfo { .. } => {}
            Packet::SourceAudioEnd { source, sequence } => {
                if let Some(src) = self.lock().sources.get_mut(&source) {
                    src.jitter.end(sequence);
                }
            }
            Packet::ActivationRegister(a) => {
                let mut s = self.lock();
                s.server.activations.retain(|x| x.name != a.name);
                s.server.activations.push(a);
            }
            Packet::ActivationUnregister { activation } => self.lock().server.activations.retain(|a| a.id() != activation),
            Packet::SourceLineRegister(l) => {
                let mut s = self.lock();
                s.server.lines.retain(|x| x.name != l.name);
                s.server.lines.push(l);
            }
            Packet::SourceLineUnregister { line } => self.lock().server.lines.retain(|l| l.id() != line),
            _ => {}
        }
    }

    /// The payloads to send to the server on `plasmo:voice/v2`.
    pub fn take_tcp(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.lock().tcp_out)
    }

    /// Each frame: positions, keys, settings.
    pub fn update(&mut self, world: World, settings: &Settings) {
        let mut s = self.lock();
        let state_changed = s.settings.voice_disabled != settings.voice_disabled || s.settings.microphone_disabled != settings.microphone_disabled;
        if state_changed && s.server.configured {
            let p = Packet::PlayerState { voice_disabled: settings.voice_disabled, microphone_muted: settings.microphone_disabled };
            s.tcp_out.push(p.encode());
        }
        let distances_changed = s.settings.activations != settings.activations;
        // The game's language changed: the server's texts in it (`LanguageChangedEvent`).
        if s.settings.language != settings.language && s.server.configured {
            s.tcp_out.push(Packet::LanguageRequest { language: settings.language.clone() }.encode());
        }
        s.settings = settings.clone();
        if distances_changed && s.server.configured {
            let distances = activation_distances(&s);
            s.tcp_out.push(Packet::PlayerActivationDistances { distances }.encode());
        }
        s.world = world;
    }

    pub fn status(&self) -> Status {
        let s = self.lock();
        let connected = s.server.configured && s.server.last_ping.is_some();
        let timed_out = s.server.last_ping.is_some_and(|t| t.elapsed() > Duration::from_secs(7));
        let me_muted = s.world.me.is_some_and(|me| s.players.iter().any(|p| p.id == me && p.muted));
        let talking = s
            .talking
            .iter()
            .filter_map(|id| s.server.activations.iter().find(|a| a.id() == *id))
            .map(|a| (a.name.clone(), a.icon.clone()))
            .collect();
        let sources = s
            .sources
            .iter()
            .filter_map(|(id, src)| {
                let info = src.info.as_ref()?;
                let line = s.server.lines.iter().find(|l| l.id() == info.line_id)?;
                Some(SourceStatus {
                    id: *id,
                    kind: info.kind.clone(),
                    name: info.name.clone(),
                    line: line.name.clone(),
                    line_icon: line.icon.clone(),
                    line_weight: line.weight,
                    icon_visible: info.icon_visible,
                    activated: src.last_audio.elapsed() < Duration::from_millis(500),
                })
            })
            .collect();
        let icons = s.server.icons.clone().unwrap_or_default();
        Status {
            connected,
            timed_out,
            talking,
            server_muted: me_muted,
            sources,
            level: s.level,
            frames: s.frames,
            input_device: s.input_device.clone(),
            input_error: s.input_error.clone(),
            server_id: s.server.configured.then_some(s.server.server_id),
            icon_hidden: icons.hidden,
            icon_offset: [icons.offset.x, icons.offset.y, icons.offset.z],
            error: s.error.clone(),
            address: s.server.addr,
            played: s.played,
            output_peak: s.output_peak.0,
            speakers: s.speakers.clone(),
            players: s.players.clone(),
        }
    }

    /// The distance spheres to draw since the last call (`DistanceVisualizePacket`, and the one
    /// of joining).
    pub fn take_visualize(&mut self) -> Vec<Visualize> {
        std::mem::take(&mut self.lock().visualize)
    }

    /// The server's activations and source lines (for the settings).
    pub fn activations(&self) -> Vec<Activation> {
        self.lock().server.activations.clone()
    }
    pub fn lines(&self) -> Vec<SourceLine> {
        self.lock().server.lines.clone()
    }
    /// A server-sent translation (`LanguagePacket`).
    pub fn translate(&self, key: &str) -> Option<String> {
        self.lock().language.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    }

    /// The UDP, capture and playback threads, once the server said where to connect.
    fn start(&mut self) {
        self.shutdown();
        self.stop = Arc::new(AtomicBool::new(false));
        let Some(addr) = lock(&self.shared).server.addr else { return };
        let socket = match UdpSocket::bind(if addr.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }).and_then(|s| {
            s.connect(addr)?;
            s.set_read_timeout(Some(Duration::from_millis(50)))?;
            Ok(s)
        }) {
            Ok(s) => s,
            Err(e) => {
                lock(&self.shared).error = Some(format!("udp: {e}"));
                return;
            }
        };
        let host = (self.server_host.clone(), self.server_port);
        for (name, f) in [("plasmovoice-udp", 0), ("plasmovoice-capture", 1), ("plasmovoice-playback", 2)] {
            let shared = self.shared.clone();
            let stop = self.stop.clone();
            let sock = socket.try_clone();
            let host = host.clone();
            let spawned = std::thread::Builder::new().name(name.into()).spawn(move || {
                let Ok(sock) = sock else { return };
                match f {
                    0 => udp_loop(&shared, &stop, &sock, host),
                    1 => capture_loop(&shared, &stop, &sock),
                    _ => playback_loop(&shared, &stop),
                }
            });
            if let Ok(t) = spawned {
                self.threads.push(t);
            }
        }
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn lock(m: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn new_source() -> Source {
    Source {
        info: None,
        jitter: Jitter { last: -1, ..Jitter::default() },
        decoder: None,
        last_audio: Instant::now() - Duration::from_secs(10),
        distance: 0,
        state: 0,
        requested: false,
        last_occlusion: -1.0,
        playing: false,
    }
}

/// The player's distance for an activation: their choice if the server lists it, else the server's default.
fn distance_of(s: &Shared, a: &Activation) -> i32 {
    let chosen = s.settings.activations.get(&a.name).and_then(|x| x.distance);
    match chosen {
        Some(d) if a.distances.contains(&d) || (a.distances.len() == 2 && a.distances[0] == -1 && d >= 1 && d <= a.distances[1]) => d,
        _ => a.default_distance,
    }
}

fn activation_distances(s: &Shared) -> Vec<(Uuid, i32)> {
    s.server.activations.iter().filter(|a| a.proximity).map(|a| (a.id(), distance_of(s, a))).collect()
}

// ---- UDP ------------------------------------------------------------------------------------

/// `NettyUdpClient`: pings until the server answers, answers its pings, takes in source audio.
fn udp_loop(shared: &Mutex<Shared>, stop: &AtomicBool, sock: &UdpSocket, host: (String, u16)) {
    let mut buf = vec![0u8; 4096];
    let mut last_hello = Instant::now() - Duration::from_secs(2);
    while !stop.load(Ordering::Relaxed) {
        // Until the first answer, a ping a second carrying the Minecraft server's address.
        {
            let s = lock(shared);
            if s.server.last_ping.is_none() && last_hello.elapsed() >= Duration::from_secs(1) {
                let p = udp::Packet::Ping { time: now_millis(), server: Some(host.clone()) };
                let _ = sock.send(&p.encode(s.server.secret, now_millis()));
                last_hello = Instant::now();
            }
            if s.server.last_ping.is_some_and(|t| t.elapsed() > Duration::from_secs(30)) {
                drop(s);
                lock(shared).error = Some("voice server timed out".into());
                return;
            }
        }
        let n = match sock.recv(&mut buf) {
            Ok(n) => n,
            Err(_) => continue,
        };
        let Ok(Some(d)) = udp::Packet::decode(&buf[..n]) else { continue };
        let mut s = lock(shared);
        match d.packet {
            udp::Packet::Ping { .. } => {
                s.server.last_ping = Some(Instant::now());
                let p = udp::Packet::Ping { time: now_millis(), server: None };
                let _ = sock.send(&p.encode(s.server.secret, now_millis()));
            }
            udp::Packet::SourceAudio { sequence, data, source, state, distance } => {
                s.server.last_ping.get_or_insert_with(Instant::now);
                // A muted line or player: the packet is dropped (`process` returns), so the
                // source isn't heard nor shown talking.
                let muted = s.sources.get(&source).and_then(|x| x.info.as_ref()).is_some_and(|info| source_muted(&s.settings, &s.server.lines, info));
                let mut request = false;
                let src = s.sources.entry(source).or_insert_with(new_source);
                if src.info.is_none() || src.state != state {
                    if !src.requested {
                        src.requested = true;
                        request = true;
                    }
                    if src.info.is_some() {
                        src.state = state;
                    }
                }
                src.distance = distance;
                if !muted {
                    src.last_audio = Instant::now();
                    src.jitter.offer(sequence, data, Instant::now());
                }
                if request {
                    s.tcp_out.push(Packet::SourceInfoRequest { source }.encode());
                }
            }
            _ => {}
        }
    }
}

/// `lineMute` (`BaseClientAudioSource`) and the player's mute (`ClientPlayerSource`, and
/// `ClientDirectSource` for its sender).
fn source_muted(settings: &Settings, lines: &[SourceLine], info: &SourceInfo) -> bool {
    let line_muted = lines.iter().find(|l| l.id() == info.line_id).is_some_and(|l| settings.muted_lines.contains(&l.name));
    let player = match &info.kind {
        SourceKind::Player(p) => Some(p.id),
        SourceKind::Direct { sender: Some(g), .. } => Some(g.id),
        _ => None,
    };
    line_muted || player.is_some_and(|p| settings.muted_players.contains(&p))
}

// ---- the microphone -------------------------------------------------------------------------

/// One activation's run-time state (`VoiceClientActivation`).
#[derive(Default)]
struct ActState {
    active: bool,
    last: Option<Instant>,
    sequence: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Result {
    NotActivated,
    Activated,
    End,
}

impl ActState {
    /// `handlePTT`: held (outside chat) or within 350 ms of letting go.
    fn ptt(&mut self, pressed: bool) -> Result {
        if pressed {
            self.active = true;
            self.last = Some(Instant::now());
        } else if self.active && self.last.is_none_or(|t| t.elapsed() > Duration::from_millis(350)) {
            self.active = false;
            return Result::End;
        }
        if self.active { Result::Activated } else { Result::NotActivated }
    }
    /// `handleVoice`: above the threshold, held 500 ms after.
    fn voice(&mut self, samples: &[i16], threshold: f64, toggle: bool, parent: Option<Result>) -> Result {
        if toggle {
            if self.active {
                self.active = false;
                return Result::End;
            }
            return Result::NotActivated;
        }
        if let Some(r) = parent {
            match r {
                Result::Activated => {
                    self.active = true;
                    self.last = Some(Instant::now());
                    return r;
                }
                Result::End => {
                    self.active = false;
                    return r;
                }
                Result::NotActivated => {}
            }
        }
        let recent = self.last.is_some_and(|t| t.elapsed() <= Duration::from_millis(500));
        let detected = dsp::contains_min_level(samples, threshold);
        if recent || detected {
            if detected {
                self.last = Some(Instant::now());
            }
            self.active = true;
            return Result::Activated;
        }
        if self.active {
            self.active = false;
            return Result::End;
        }
        Result::NotActivated
    }
    /// `handleInherit`.
    fn inherit(&mut self, parent: Result, toggle: bool) -> Result {
        if toggle {
            if self.active {
                self.active = false;
                return Result::End;
            }
            return Result::NotActivated;
        }
        self.active = parent == Result::Activated;
        if self.active {
            self.last = Some(Instant::now());
        }
        parent
    }
}

/// The microphone the settings ask for, opened again when they change (`VoiceDeviceManager
/// .reloadInputDevice`, also on Stereo Capture): none while `disable_input_device` is on.
struct Microphone {
    capture: Option<Capture>,
    /// The device asked for when it was opened ("" for the system's default), and in stereo.
    wanted: Option<(String, bool)>,
    failed_at: Option<Instant>,
}

impl Microphone {
    /// The next frame and its channels (1, or 2 with stereo capture).
    fn next(&mut self, shared: &Mutex<Shared>, rate: i32) -> Option<(Vec<i16>, usize)> {
        let (wanted, disabled) = {
            let s = lock(shared);
            ((s.settings.input_device.clone(), s.settings.stereo_capture), s.settings.disable_input_device)
        };
        if disabled {
            if self.capture.take().is_some() || self.wanted.is_some() {
                let mut s = lock(shared);
                s.input_device = None;
                s.input_error = None;
            }
            self.wanted = None;
            std::thread::sleep(Duration::from_millis(100));
            return None;
        }
        if self.wanted.as_ref() != Some(&wanted) {
            self.capture = None;
            self.failed_at = None;
            self.wanted = Some(wanted.clone());
        }
        let (device, stereo) = wanted;
        if self.capture.is_none() {
            // A device that failed is tried again a second later.
            if self.failed_at.is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
                std::thread::sleep(Duration::from_millis(100));
                return None;
            }
            let frame_size = (rate / 1000 * 20) as usize;
            match Capture::open(if device.is_empty() { None } else { Some(&device) }, rate as u32, frame_size, stereo) {
                Ok(c) => {
                    let mut s = lock(shared);
                    s.input_device = Some(c.device.clone());
                    s.input_error = None;
                    self.capture = Some(c);
                }
                Err(e) => {
                    let mut s = lock(shared);
                    s.input_device = None;
                    s.input_error = Some(e);
                    self.failed_at = Some(Instant::now());
                    return None;
                }
            }
        }
        let cap = self.capture.as_ref()?;
        match cap.frames.recv_timeout(Duration::from_millis(200)) {
            Ok(frame) => Some((frame, cap.channels)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                self.capture = None;
                self.failed_at = Some(Instant::now());
                None
            }
        }
    }
}

/// The microphone test's source (`ClientLoopbackSource`, relative): the processed frame at the
/// voice volume (cubed when the volume slider is exponential, at most 1: OpenAL's default
/// `AL_MAX_GAIN`), stereo when stereo capture is on and stereo sources aren't made mono.
const LOOPBACK: u128 = u128::MAX;

/// The filters of the microphone's frame (`InputDevice.processFilters`), run once each for its
/// mono and stereo forms as something asks for them (`EncodedCapture.getMono`, `getStereo`):
/// `StereoToMonoFilter` (mono only), `GainFilter`, `NoiseSuppressionFilter`.
struct Filters {
    gain: dsp::Gain,
    noise: Option<dsp::NoiseSuppression>,
}

impl Filters {
    fn run(&mut self, raw: &[i16], channels: usize, stereo: bool, settings: &Settings, rate: i32) -> Vec<i16> {
        let (mut samples, channels) = if channels == 2 && !stereo { (dsp::stereo_to_mono(raw), 1) } else { (raw.to_vec(), channels) };
        self.gain.process(&mut samples, settings.microphone_volume as f32);
        if let Some(ns) = self.noise.as_mut() {
            samples = ns.process(&samples, channels, rate);
        }
        samples
    }
}

/// `EncodedCapture`: the frame processed at most once in each form.
struct Processed<'a> {
    raw: &'a [i16],
    channels: usize,
    mono: Option<Vec<i16>>,
    stereo: Option<Vec<i16>>,
}

impl Processed<'_> {
    fn get(&mut self, stereo: bool, filters: &mut Filters, settings: &Settings, rate: i32) -> Vec<i16> {
        let (raw, channels) = (self.raw, self.channels);
        let slot = if stereo { &mut self.stereo } else { &mut self.mono };
        slot.get_or_insert_with(|| filters.run(raw, channels, stereo, settings, rate)).clone()
    }
}

/// `VoiceAudioCapture.run`: frames from the microphone through the activations, the filters
/// (`StereoToMonoFilter`, `GainFilter`, `NoiseSuppressionFilter`; the compressor and limiter
/// are commented out in Plasmo Voice), Opus and AES, to the server.
fn capture_loop(shared: &Mutex<Shared>, stop: &AtomicBool, sock: &UdpSocket) {
    // Wait for the server's Config (sample rate, codec, key).
    let mut waited = 0;
    let rate = loop {
        if stop.load(Ordering::Relaxed) || waited > 300 {
            return;
        }
        let r = lock(shared).server.sample_rate;
        if r > 0 {
            break r;
        }
        std::thread::sleep(Duration::from_millis(100));
        waited += 1;
    };
    let (mode, bitrate, mtu) = {
        let s = lock(shared);
        (s.server.codec_mode.clone(), s.server.bitrate, s.server.mtu)
    };
    let new_encoder = |stereo: bool| Encoder::new(rate, stereo, &mode, bitrate, mtu.max(64) as usize);
    let Some(mut mono_encoder) = new_encoder(false) else {
        lock(shared).error = Some("opus encoder".into());
        return;
    };
    let mut stereo_encoder: Option<Encoder> = None;
    let mut mic = Microphone { capture: None, wanted: None, failed_at: None };
    let mut filters = Filters { gain: dsp::Gain::default(), noise: None };
    let mut states: HashMap<Uuid, ActState> = HashMap::new();
    while !stop.load(Ordering::Relaxed) {
        let Some((raw, channels)) = mic.next(shared, rate) else { continue };
        let mut s = lock(shared);
        let settings = s.settings.clone();
        let key = s.server.key.clone();
        let secret = s.server.secret;
        let activations = {
            let mut a = s.server.activations.clone();
            // Heaviest first (`getActivations` order by weight).
            a.sort_by(|x, y| y.weight.cmp(&x.weight));
            a
        };
        let held = s.world.held.clone();
        let typing = s.world.typing;
        // The noise suppression's RNNoise states live while it is on (`toggle`).
        if settings.noise_suppression {
            filters.noise.get_or_insert_with(dsp::NoiseSuppression::default);
        } else {
            filters.noise = None;
        }
        let mut processed = Processed { raw: &raw, channels, mono: None, stereo: None };
        // The meter and the microphone test (`MicrophoneTestController.onAudioCaptureProcessed`):
        // the processed frame, stereo when stereo capture is on and stereo sources aren't mono.
        let test_stereo = channels == 2 && !settings.stereo_sources_to_mono;
        let heard = processed.get(test_stereo, &mut filters, &settings, rate);
        s.level = dsp::highest_level(&heard);
        s.frames += 1;
        if settings.microphone_test
            && let Some(mix) = s.mix.clone()
        {
            let mut volume = settings.volume as f32;
            if settings.exponential_volume_slider {
                volume = volume.powi(3);
            }
            let volume = volume.min(1.0);
            let stereo: Vec<f32> = if test_stereo {
                heard.iter().map(|&v| f32::from(v) / 32768.0 * volume).collect()
            } else {
                heard
                    .iter()
                    .flat_map(|&v| {
                        let f = f32::from(v) / 32768.0 * volume;
                        [f, f]
                    })
                    .collect()
            };
            if let Ok(mut m) = mix.lock() {
                m.push(LOOPBACK, &stereo);
            }
        }
        drop(s);
        let Some(parent) = activations.iter().find(|a| a.name == crate::tcp::PROXIMITY).cloned() else { continue };
        let send = |activation: &Activation, state: &mut ActState, encoded: Option<&[u8]>, stereo: bool, end: bool, s: &mut Shared| {
            let distance = distance_of(s, activation) as i16;
            if let Some(data) = encoded {
                state.sequence += 1;
                let p = udp::Packet::PlayerAudio { sequence: state.sequence, data: data.to_vec(), activation: activation.id(), distance, stereo };
                let _ = sock.send(&p.encode(secret, now_millis()));
            }
            if end {
                state.sequence += 1;
                s.tcp_out.push(Packet::PlayerAudioEnd { sequence: state.sequence, activation: activation.id(), distance }.encode());
            }
        };
        // `isFlushActivations` (the microphone test), the microphone or the voice chat off, or
        // a server mute: every active activation ends.
        let server_muted = s_server_muted(shared);
        let flush = settings.microphone_test || settings.microphone_disabled || settings.voice_disabled || server_muted;
        let mut s = lock(shared);
        if flush {
            for a in &activations {
                let st = states.entry(a.id()).or_default();
                if st.active {
                    st.active = false;
                    st.last = None;
                    send(a, st, None, false, true, &mut s);
                }
            }
            s.talking.clear();
            mono_encoder = new_encoder(false).unwrap_or(mono_encoder);
            stereo_encoder = None;
            continue;
        }
        // The frame, encoded once in each form for every activation that sends it
        // (`processActivation`: stereo when stereo capture is on and the activation takes it).
        let mut encoded: [Option<Option<Vec<u8>>>; 2] = [None, None];
        let mut encode = |stereo: bool, processed: &mut Processed<'_>| -> Option<Vec<u8>> {
            if let Some(e) = &encoded[usize::from(stereo)] {
                return e.clone();
            }
            let frame = processed.get(stereo, &mut filters, &settings, rate);
            let encoder = if stereo {
                if stereo_encoder.is_none() {
                    stereo_encoder = new_encoder(true);
                }
                stereo_encoder.as_mut()
            } else {
                Some(&mut mono_encoder)
            };
            let out = encoder.and_then(|e| e.encode(&frame)).map(|opus| match &key {
                Some(k) => k.encrypt(&opus),
                None => opus,
            });
            encoded[usize::from(stereo)] = Some(out.clone());
            out
        };
        let is_stereo = |a: &Activation| channels == 2 && a.stereo_supported;
        let config_of = |a: &Activation| settings.activations.get(&a.name).cloned().unwrap_or_default();
        let result_of = |a: &Activation, st: &mut ActState, parent_result: Option<Result>| -> Result {
            let c = config_of(a);
            match c.kind {
                ActivationType::PushToTalk => st.ptt(held.contains(&a.name) && !typing),
                ActivationType::Voice => st.voice(&raw, settings.activation_threshold, c.toggle, parent_result),
                ActivationType::Inherit => st.inherit(parent_result.unwrap_or(Result::NotActivated), c.toggle),
            }
        };
        let parent_state = states.entry(parent.id()).or_default();
        let parent_result = result_of(&parent, parent_state, None);
        let (mut prox_reached, mut other_reached) = (false, false);
        s.talking.clear();
        for a in activations.iter().filter(|a| a.name != parent.name) {
            let reached = if a.proximity { prox_reached } else { other_reached };
            let st = states.entry(a.id()).or_default();
            // `isDisabled() && !isActive()`: a voice activation toggled off.
            let c = config_of(a);
            if c.kind == ActivationType::Voice && c.toggle && !st.active {
                continue;
            }
            if reached {
                *st = ActState { sequence: st.sequence, ..ActState::default() };
                continue;
            }
            let r = result_of(a, st, Some(parent_result));
            if r != Result::NotActivated {
                let stereo = is_stereo(a);
                let data = encode(stereo, &mut processed);
                send(a, st, data.as_deref(), stereo, r == Result::End, &mut s);
                if r == Result::Activated {
                    s.talking.push(a.id());
                }
            }
            if r == Result::Activated && !a.transitive {
                if a.proximity {
                    prox_reached = true;
                } else {
                    other_reached = true;
                }
            }
        }
        let reached = if parent.proximity { prox_reached } else { other_reached };
        let st = states.entry(parent.id()).or_default();
        if !reached {
            if parent_result != Result::NotActivated {
                let stereo = is_stereo(&parent);
                let data = encode(stereo, &mut processed);
                send(&parent, st, data.as_deref(), stereo, parent_result == Result::End, &mut s);
                if parent_result == Result::Activated {
                    s.talking.push(parent.id());
                }
            }
        } else if st.active {
            st.active = false;
            send(&parent, st, None, false, true, &mut s);
        }
    }
    let mut s = lock(shared);
    s.input_device = None;
    s.talking.clear();
}

/// `isServerMuted`: the server's player list says this client's player is muted. The client
/// doesn't know its own UUID, so the game passes it in `World::me`.
fn s_server_muted(shared: &Mutex<Shared>) -> bool {
    let s = lock(shared);
    let Some(me) = s.world.me else { return false };
    s.players.iter().any(|p| p.id == me && p.muted)
}

// ---- the speakers ---------------------------------------------------------------------------

/// Every 10 ms: each source's due frames, decoded, at its volume and side, into the mix. The
/// speakers open again when the output device changes (`reloadOutputDevice`).
fn playback_loop(shared: &Mutex<Shared>, stop: &AtomicBool) {
    // The server's Config says the sample rate; it comes after the UDP connection opens.
    let rate = loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let r = lock(shared).server.sample_rate;
        if r > 0 {
            break r;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut out: Option<(String, Output)> = None;
    let mut failed_at: Option<Instant> = None;
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(10));
        let device = lock(shared).settings.output_device.clone();
        if out.as_ref().is_some_and(|(d, _)| *d != device) {
            out = None;
            failed_at = None;
            lock(shared).mix = None;
        }
        if out.is_none() && failed_at.is_none_or(|t| t.elapsed() > Duration::from_secs(1)) {
            match Output::open(if device.is_empty() { None } else { Some(&device) }, rate as u32) {
                Ok(o) => {
                    let mut s = lock(shared);
                    s.speakers = Some(o.device.clone());
                    s.mix = Some(o.mix.clone());
                    out = Some((device, o));
                }
                Err(e) => {
                    let mut s = lock(shared);
                    s.error = Some(format!("speakers: {e}"));
                    s.speakers = None;
                    failed_at = Some(Instant::now());
                }
            }
        }
        let mut s = lock(shared);
        let now = Instant::now();
        let delay = (s.settings.jitter_packet_delay.max(0) as u64) * 20;
        let key = s.server.key.clone();
        let world = s.world.clone();
        let settings = s.settings.clone();
        let lines = s.server.lines.clone();
        let sample_rate = s.server.sample_rate;
        let allow_freecam = s.server.permissions.iter().find(|(k, _)| k == "pv.allow_freecam").is_none_or(|(_, v)| *v);
        let ears = Ears::of(&world, &settings, allow_freecam);
        // `isActivated` of each kind of source, for `source_types_overlap` (`shouldWrite`).
        let activated = |src: &Source| src.last_audio.elapsed() < Duration::from_millis(500);
        let talking_players: Vec<Uuid> = s
            .sources
            .values()
            .filter(|x| activated(x))
            .filter_map(|x| match x.info.as_ref().map(|i| &i.kind) {
                Some(SourceKind::Player(p)) => Some(p.id),
                _ => None,
            })
            .collect();
        let talking_directly: Vec<Uuid> = s
            .sources
            .values()
            .filter(|x| activated(x))
            .filter_map(|x| match x.info.as_ref().map(|i| &i.kind) {
                Some(SourceKind::Direct { sender: Some(g), .. }) => Some(g.id),
                _ => None,
            })
            .collect();
        let frame_size = (sample_rate / 1000 * 20) as usize;
        let mut frames: Vec<(u128, Vec<f32>)> = Vec::new();
        for (id, src) in s.sources.iter_mut() {
            let Some(info) = src.info.clone() else { continue };
            while let Some(frame) = src.jitter.poll(delay, now) {
                let decoder = src.decoder.get_or_insert_with(|| Decoder::new(sample_rate, info.stereo).expect("opus decoder"));
                let (pcm, last) = match frame {
                    Frame::Audio(data, last) => {
                        let plain = match &key {
                            Some(k) => k.decrypt(&data),
                            None => Some(data),
                        };
                        (decoder.decode(plain.as_deref()), last)
                    }
                    // `processPlc`: Opus conceals a lost mono packet; a stereo one is silence.
                    Frame::Lost if decoder.channels() == 2 => (vec![0.0; frame_size * 2], false),
                    Frame::Lost => (decoder.decode(None), false),
                    // What is queued still plays; the next talk starts a fresh decoder.
                    Frame::End => {
                        src.decoder = None;
                        src.playing = false;
                        break;
                    }
                };
                // Voice chat off (`sourceManager.clear`): nothing plays.
                if settings.voice_disabled {
                    continue;
                }
                // `shouldWrite`: the source still shows talking, but isn't heard over the other
                // kind of the same player's.
                let silenced = match (&info.kind, settings.source_types_overlap) {
                    (SourceKind::Player(p), Overlap::MuteProximity) => talking_directly.contains(&p.id),
                    (SourceKind::Direct { sender: Some(g), .. }, Overlap::MuteDirect) => talking_players.contains(&g.id),
                    _ => false,
                };
                if silenced {
                    continue;
                }
                // `calculateOcclusion`, moved toward by 0.05 a frame once heard (`lastOcclusion`).
                let occlusion = (settings.sound_occlusion && occludable(&info)).then(|| {
                    let measured = world.occlusion.get(id).copied().unwrap_or(0.0);
                    if src.last_occlusion >= 0.0 {
                        src.last_occlusion = if measured > src.last_occlusion {
                            (src.last_occlusion + 0.05).max(0.0)
                        } else {
                            (src.last_occlusion - 0.05).max(measured)
                        };
                        src.last_occlusion
                    } else {
                        src.last_occlusion = measured;
                        measured
                    }
                });
                // A stereo source plays as it is (OpenAL doesn't place stereo buffers), unless
                // `stereo_sources_to_mono` mixes it down (`convertToMonoShorts`) to place it.
                let stereo_out = decoder.channels() == 2 && !settings.stereo_sources_to_mono;
                let (l, r) = gains(&info, &world, &ears, &settings, &lines, f64::from(src.distance), occlusion, stereo_out);
                let mut out: Vec<f32> = if stereo_out {
                    pcm.chunks(2).flat_map(|c| [c[0] * l, c.get(1).copied().unwrap_or(c[0]) * r]).collect()
                } else {
                    let mono: Vec<f32> = if decoder.channels() == 2 {
                        pcm.chunks(2).map(|c| ((c[0] * 32768.0) as i32 + (c.get(1).copied().unwrap_or(c[0]) * 32768.0) as i32) as f32 / 2.0 / 32768.0).collect()
                    } else {
                        pcm
                    };
                    mono.iter().flat_map(|&v| [v * l, v * r]).collect()
                };
                // `write`: the first frame of a talk fades in, the last before its end out.
                let n = out.len() / 2;
                if !src.playing {
                    for (i, p) in out.chunks_mut(2).enumerate() {
                        let k = (i * 2) as f32 / (n * 2) as f32;
                        p.iter_mut().for_each(|v| *v *= k.min(1.0));
                    }
                } else if last {
                    for (i, p) in out.chunks_mut(2).enumerate() {
                        let k = ((n * 2 - i * 2) as f32 / (n * 2) as f32).max(0.0);
                        p.iter_mut().for_each(|v| *v *= k);
                    }
                }
                src.playing = true;
                frames.push((id.0, out));
            }
        }
        // Forget sources silent for a while (`closeTimeoutMs` 500).
        s.sources.retain(|_, src| src.last_audio.elapsed() < Duration::from_secs(5) || src.info.is_none());
        s.played += frames.len() as u64;
        let peak = frames.iter().flat_map(|(_, f)| f.iter()).fold(0f32, |m, v| m.max(v.abs()));
        // The loudest sample of the last second's frames, silent ones (out of range, muted) too.
        if !frames.is_empty() {
            if s.output_peak.1.is_none_or(|t| t.elapsed() > Duration::from_secs(1)) {
                s.output_peak = (peak, Some(now));
            } else {
                s.output_peak.0 = s.output_peak.0.max(peak);
            }
        }
        drop(s);
        if let Some((_, o)) = &out
            && let Ok(mut m) = o.mix.lock()
        {
            for (id, f) in frames {
                m.push(id, &f);
            }
        }
    }
    let mut s = lock(shared);
    s.mix = None;
    s.speakers = None;
}

/// Where the ears are and which way is right: the camera's with `camera_sound_listener` (and the
/// server's `pv.allow_freecam`), else the player's eyes and look (`AlListener.update`).
#[derive(Clone, Copy, Debug)]
struct Ears {
    position: [f64; 3],
    /// The listener's right, from its yaw (Minecraft's: 0 looks to +Z, 90 to -X).
    right: [f64; 3],
}

impl Ears {
    fn of(world: &World, settings: &Settings, allow_freecam: bool) -> Ears {
        let (position, yaw) = match world.camera {
            Some((p, yaw, _)) if settings.camera_sound_listener && allow_freecam => (p, yaw),
            _ => (world.listener, world.listener_yaw),
        };
        let yaw = f64::from(yaw).to_radians();
        Ears { position, right: [-yaw.cos(), 0.0, -yaw.sin()] }
    }
}

/// `shouldCalculateOcclusion`: every source in the world (a direct one only with a position).
fn occludable(info: &SourceInfo) -> bool {
    match &info.kind {
        SourceKind::Direct { relative, .. } => relative.is_some(),
        _ => true,
    }
}

/// Where a source is (`getPosition`, none for a direct source without a position) and where it
/// looks (`getLookAngle`, zero when unknown).
pub fn source_position(kind: &SourceKind, world: &World) -> (Option<[f64; 3]>, [f64; 3]) {
    match kind {
        SourceKind::Player(p) => (
            Some(world.players.get(&p.id).copied().unwrap_or_default()),
            world.player_looks.get(&p.id).copied().unwrap_or_default(),
        ),
        SourceKind::Entity(e) => (Some(world.entities.get(e).copied().unwrap_or_default()), world.entity_looks.get(e).copied().unwrap_or_default()),
        SourceKind::Static { pos, look } => (Some([pos.x, pos.y, pos.z]), [look.x, look.y, look.z]),
        SourceKind::Direct { relative: Some(r), camera_relative, .. } => {
            // A camera-relative position is the vector itself (`getPosition`); otherwise it is
            // from the player's eyes.
            if *camera_relative {
                (Some([r.x, r.y, r.z]), [0.0; 3])
            } else {
                let l = world.listener;
                (Some([l[0] + r.x, l[1] + r.y, l[2] + r.z]), [0.0; 3])
            }
        }
        SourceKind::Direct { .. } => (None, [0.0; 3]),
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// `Vec3.normalize`: zero for a vector shorter than 1e-4.
fn normalize(a: [f64; 3]) -> [f64; 3] {
    let l = length(a);
    if l < 1.0e-4 { [0.0; 3] } else { [a[0] / l, a[1] / l, a[2] / l] }
}

/// `updateSource`: the source's left and right gains. The volume is the voice volume × the
/// player's (or source's) × the line's, × (1 - occlusion), cubed under 1 with the exponential
/// volume slider, × the directional gain, × the distance gain (`1 - d / max`, cubed with the
/// exponential distance gain), at most 4 (`AL_MAX_GAIN`). A mono source is panned from where it
/// is around the ears (equal power: OpenAL's positioning, without HRTF); a stereo one
/// (`stereo`), a camera-relative one, the player's own, or any with Stereo Positioning off,
/// plays on both sides alike.
#[allow(clippy::too_many_arguments)]
fn gains(info: &SourceInfo, world: &World, ears: &Ears, settings: &Settings, lines: &[SourceLine], max_distance: f64, occlusion: Option<f64>, stereo: bool) -> (f32, f32) {
    let (player_volume, muted) = match &info.kind {
        SourceKind::Player(p) => (settings.player_volumes.get(&p.id).copied().unwrap_or(1.0), settings.muted_players.contains(&p.id)),
        SourceKind::Direct { sender: Some(g), .. } => (settings.player_volumes.get(&g.id).copied().unwrap_or(1.0), settings.muted_players.contains(&g.id)),
        _ => (1.0, false),
    };
    if muted {
        return (0.0, 0.0);
    }
    let line = lines.iter().find(|l| l.id() == info.line_id);
    let line_volume = line.map_or(1.0, |l| settings.line_volumes.get(&l.name).copied().unwrap_or(l.default_volume));
    let mut volume = settings.volume * player_volume * line_volume;
    if let Some(o) = occlusion {
        volume *= 1.0 - o;
    }
    if settings.exponential_volume_slider && volume < 1.0 {
        volume = volume.powi(3);
    }
    let (pos, look) = source_position(&info.kind, world);
    let Some(pos) = pos else {
        // A direct source without a position plays in the head.
        let v = volume.clamp(0.0, 4.0) as f32;
        return (v, v);
    };
    let camera_relative = matches!(info.kind, SourceKind::Direct { camera_relative: true, .. });
    // `shouldCalculateDirectionalGain`: a static source only with an angle of its own, a direct
    // one never (its look isn't sent), the others with Directional Sources on or an angle.
    let directional = match &info.kind {
        SourceKind::Static { .. } => info.angle > 0,
        SourceKind::Direct { .. } => false,
        _ => settings.directional_sources || info.angle > 0,
    };
    if directional {
        let to_listener = normalize(sub(world.listener, pos));
        let angle = dot(to_listener, look).clamp(-1.0, 1.0).acos().to_degrees();
        let inner = f64::from(if info.angle > 0 { info.angle / 2 } else { settings.directional_sources_angle / 2 });
        if angle > inner {
            let mut g = 1.0 - (angle - inner) / (180.0 - inner);
            if settings.exponential_distance_gain {
                g = g.powi(3);
            }
            volume *= g;
        }
    }
    // `calculateDistanceGain(min(d, max), max)`, 1 for a camera-relative direct source (and for
    // no distance, where Plasmo Voice's gain isn't a number and OpenAL keeps the last one).
    let d = length(sub(pos, world.listener));
    if max_distance > 0.0 && !camera_relative {
        let mut g = 1.0 - d.min(max_distance) / max_distance;
        if settings.exponential_distance_gain {
            g = g.powi(3);
        }
        volume *= g;
    }
    let v = volume.clamp(0.0, 4.0) as f32;
    let own = matches!(&info.kind, SourceKind::Player(p) if world.me == Some(p.id));
    let to = sub(pos, ears.position);
    if stereo || camera_relative || own || !settings.panning || length(to) < 0.01 {
        return (v, v);
    }
    let pan = (dot(normalize(to), ears.right)).clamp(-1.0, 1.0) as f32;
    // Equal-power panning.
    let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
    (v * angle.cos() * std::f32::consts::SQRT_2, v * angle.sin() * std::f32::consts::SQRT_2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_plays_in_order_and_conceals() {
        let mut j = Jitter { last: -1, ..Jitter::default() };
        let t0 = Instant::now();
        j.offer(1, vec![1], t0);
        j.offer(3, vec![3], t0);
        assert!(j.poll(40, t0).is_none(), "waits for the delay");
        let t = t0 + Duration::from_millis(41);
        assert!(matches!(j.poll(40, t), Some(Frame::Audio(d, false)) if d == vec![1]));
        let t = t0 + Duration::from_millis(61);
        assert!(matches!(j.poll(40, t), Some(Frame::Lost)));
        let t = t0 + Duration::from_millis(81);
        assert!(matches!(j.poll(40, t), Some(Frame::Audio(d, false)) if d == vec![3]));
    }

    #[test]
    fn ptt_tail() {
        let mut st = ActState::default();
        assert_eq!(st.ptt(true), Result::Activated);
        assert_eq!(st.ptt(false), Result::Activated, "350 ms tail");
        st.last = Some(Instant::now() - Duration::from_millis(400));
        assert_eq!(st.ptt(false), Result::End);
        assert_eq!(st.ptt(false), Result::NotActivated);
    }

    #[test]
    fn voice_holds_half_a_second() {
        let mut st = ActState::default();
        let loud = vec![20000i16; 960];
        let quiet = vec![0i16; 960];
        assert_eq!(st.voice(&loud, -30.0, false, None), Result::Activated);
        assert_eq!(st.voice(&quiet, -30.0, false, None), Result::Activated);
        st.last = Some(Instant::now() - Duration::from_millis(600));
        assert_eq!(st.voice(&quiet, -30.0, false, None), Result::End);
    }

    #[test]
    fn distance_gain_and_pan() {
        let info = SourceInfo {
            addon_id: "pv".into(),
            id: Uuid(1),
            name: None,
            state: 0,
            decoder: None,
            stereo: false,
            line_id: SourceLine::id_of("proximity"),
            icon_visible: true,
            angle: 0,
            kind: SourceKind::Player(PlayerInfo { id: Uuid(7), nick: "a".into(), muted: false, voice_disabled: false, microphone_muted: false }),
        };
        let mut world = World::default();
        world.players.insert(Uuid(7), [-8.0, 0.0, 0.0]);
        let settings = Settings::default();
        let ears = |w: &World, s: &Settings| Ears::of(w, s, true);
        let g = |w: &World, s: &Settings| gains(&info, w, &ears(w, s), s, &[], 16.0, None, false);
        // Looking to +Z (yaw 0), a player at -X is on the right; half the distance: (1-0.5)^3.
        let (l, r) = g(&world, &settings);
        assert!(r > l);
        assert!(((l * l + r * r).sqrt() - 0.125 * std::f32::consts::SQRT_2).abs() < 1e-4);
        world.players.insert(Uuid(7), [0.0, 0.0, 20.0]);
        assert_eq!(g(&world, &settings), (0.0, 0.0));
        // The settings' volume at 200% doubles the mix (the cube only applies under 100%), at
        // 50% it is cubed, and a player's own volume multiplies it.
        world.players.insert(Uuid(7), [-8.0, 0.0, 0.0]);
        let loud = Settings { volume: 2.0, ..Settings::default() };
        let (l2, r2) = g(&world, &loud);
        assert!((l2 - 2.0 * l).abs() < 1e-5 && (r2 - 2.0 * r).abs() < 1e-5);
        let quiet = Settings { volume: 0.5, ..Settings::default() };
        let (_, rq) = g(&world, &quiet);
        assert!((rq - 0.125 * r).abs() < 1e-5);
        let mut player = Settings::default();
        player.player_volumes.insert(Uuid(7), 2.0);
        assert!((g(&world, &player).1 - 2.0 * r).abs() < 1e-5);
        player.muted_players.push(Uuid(7));
        assert_eq!(g(&world, &player), (0.0, 0.0));
        // Stereo Positioning off: both sides alike.
        let flat = Settings { panning: false, ..Settings::default() };
        let (fl, fr) = g(&world, &flat);
        assert!((fl - fr).abs() < 1e-6 && (fl - 0.125).abs() < 1e-4);
        // The camera's ears (third person behind, turned around): the same player is now on
        // the left; with the setting off, the player's own ears again.
        world.camera = Some(([0.0, 0.0, 4.0], 180.0, 0.0));
        let (cl, cr) = g(&world, &settings);
        assert!(cl > cr);
        let own_ears = Settings { camera_sound_listener: false, ..Settings::default() };
        let (ol, or) = g(&world, &own_ears);
        assert!(or > ol);
        world.camera = None;
        // Occlusion: half occluded is half the volume before the cube.
        let (_, occluded) = gains(&info, &world, &ears(&world, &settings), &settings, &[], 16.0, Some(0.5), false);
        assert!((occluded - 0.125 * r).abs() < 1e-5);
    }

    #[test]
    fn directional_sources() {
        let mut info = SourceInfo {
            addon_id: "pv".into(),
            id: Uuid(1),
            name: None,
            state: 0,
            decoder: None,
            stereo: false,
            line_id: SourceLine::id_of("proximity"),
            icon_visible: true,
            angle: 0,
            kind: SourceKind::Player(PlayerInfo { id: Uuid(7), nick: "a".into(), muted: false, voice_disabled: false, microphone_muted: false }),
        };
        let mut world = World::default();
        // The talker 8 blocks to +Z, looking away from the listener (to +Z).
        world.players.insert(Uuid(7), [0.0, 0.0, 8.0]);
        world.player_looks.insert(Uuid(7), [0.0, 0.0, 1.0]);
        let off = Settings::default();
        let on = Settings { directional_sources: true, ..Settings::default() };
        let v = |info: &SourceInfo, s: &Settings, w: &World| gains(info, w, &Ears::of(w, s, true), s, &[], 16.0, None, false).0;
        let base = v(&info, &off, &world);
        // Behind: 180 degrees, past the 145 degree cone, the angle gain is 0.
        assert!(v(&info, &on, &world) < 1e-6);
        // Facing the listener: inside the cone, no change.
        world.player_looks.insert(Uuid(7), [0.0, 0.0, -1.0]);
        assert!((v(&info, &on, &world) - base).abs() < 1e-6);
        // Sideways, 90 degrees: (1 - (90 - 72) / (180 - 72))^3 with the exponential gain.
        world.player_looks.insert(Uuid(7), [1.0, 0.0, 0.0]);
        let expected = base * (1.0 - 18.0f32 / 108.0).powi(3);
        assert!((v(&info, &on, &world) - expected).abs() < 1e-5);
        // A source's own angle applies with the setting off, too.
        info.angle = 90;
        let expected = base * (1.0 - 45.0f32 / 135.0).powi(3);
        assert!((v(&info, &off, &world) - expected).abs() < 1e-5);
    }
}
