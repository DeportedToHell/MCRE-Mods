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

/// The settings the engine reads (`VoiceClientConfig`), with Plasmo Voice's defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub voice_disabled: bool,
    pub microphone_disabled: bool,
    pub activation_threshold: f64,
    pub input_device: String,
    pub output_device: String,
    pub microphone_volume: f64,
    pub volume: f64,
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
            activation_threshold: -30.0,
            input_device: String::new(),
            output_device: String::new(),
            microphone_volume: 1.0,
            volume: 1.0,
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
    /// The listener's ears (the camera) and where it looks, yaw in degrees (Minecraft's).
    pub listener: [f64; 3],
    pub listener_yaw: f32,
    /// Players' eye positions by UUID, and entities' by id.
    pub players: HashMap<Uuid, [f64; 3]>,
    pub entities: HashMap<i32, [f64; 3]>,
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
    Audio(Vec<u8>),
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
            return Some(Frame::Audio(data));
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
                let delay = s.settings.jitter_packet_delay;
                let _ = delay;
                let entry = s.sources.entry(info.id).or_insert_with(new_source);
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
                src.last_audio = Instant::now();
                src.jitter.offer(sequence, data, Instant::now());
                if request {
                    s.tcp_out.push(Packet::SourceInfoRequest { source }.encode());
                }
            }
            _ => {}
        }
    }
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
/// .reloadInputDevice`): none while `disable_input_device` is on.
struct Microphone {
    capture: Option<Capture>,
    /// The device asked for when it was opened ("" for the system's default).
    wanted: Option<String>,
    failed_at: Option<Instant>,
}

impl Microphone {
    fn next(&mut self, shared: &Mutex<Shared>, rate: i32) -> Option<Vec<i16>> {
        let (wanted, disabled) = {
            let s = lock(shared);
            (s.settings.input_device.clone(), s.settings.disable_input_device)
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
        if self.wanted.as_deref() != Some(wanted.as_str()) {
            self.capture = None;
            self.failed_at = None;
            self.wanted = Some(wanted.clone());
        }
        if self.capture.is_none() {
            // A device that failed is tried again a second later.
            if self.failed_at.is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
                std::thread::sleep(Duration::from_millis(100));
                return None;
            }
            let frame_size = (rate / 1000 * 20) as usize;
            match Capture::open(if wanted.is_empty() { None } else { Some(&wanted) }, rate as u32, frame_size) {
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
            Ok(frame) => Some(frame),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                self.capture = None;
                self.failed_at = Some(Instant::now());
                None
            }
        }
    }
}

/// The microphone test's source (`ClientLoopbackSource`, relative): the processed frame on both
/// ears at the voice volume, cubed when the volume slider is exponential.
const LOOPBACK: u128 = u128::MAX;

/// `VoiceAudioCapture.run`: frames from the microphone through the activations, the filters
/// (`StereoToMonoFilter`, `GainFilter`; the compressor and limiter are commented out in Plasmo
/// Voice), Opus and AES, to the server.
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
    let Some(mut encoder) = Encoder::new(rate, false, &mode, bitrate, mtu.max(64) as usize) else {
        lock(shared).error = Some("opus encoder".into());
        return;
    };
    let mut mic = Microphone { capture: None, wanted: None, failed_at: None };
    let mut gain = dsp::Gain::default();
    let mut states: HashMap<Uuid, ActState> = HashMap::new();
    while !stop.load(Ordering::Relaxed) {
        let Some(raw) = mic.next(shared, rate) else { continue };
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
        // The processed frame (`EncodedCapture.getMono`): what the meter and the test hear.
        let mut processed = raw.clone();
        gain.process(&mut processed, settings.microphone_volume as f32);
        s.level = dsp::highest_level(&processed);
        s.frames += 1;
        if settings.microphone_test
            && let Some(mix) = s.mix.clone()
        {
            let mut volume = settings.volume as f32;
            if settings.exponential_volume_slider {
                volume = volume.powi(3);
            }
            let stereo: Vec<f32> = processed.iter().flat_map(|&v| {
                let f = f32::from(v) / 32768.0 * volume;
                [f, f]
            }).collect();
            if let Ok(mut m) = mix.lock() {
                m.push(LOOPBACK, &stereo);
            }
        }
        drop(s);
        let Some(parent) = activations.iter().find(|a| a.name == crate::tcp::PROXIMITY).cloned() else { continue };
        let send = |activation: &Activation, state: &mut ActState, encoded: Option<&[u8]>, end: bool, s: &mut Shared| {
            let distance = distance_of(s, activation) as i16;
            if let Some(data) = encoded {
                state.sequence += 1;
                let p = udp::Packet::PlayerAudio { sequence: state.sequence, data: data.to_vec(), activation: activation.id(), distance, stereo: false };
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
                    send(a, st, None, true, &mut s);
                }
            }
            s.talking.clear();
            encoder = Encoder::new(rate, false, &mode, bitrate, mtu.max(64) as usize).unwrap_or(encoder);
            continue;
        }
        // The frame, encoded once for every activation that sends it.
        let mut encoded: Option<Option<Vec<u8>>> = None;
        let mut encode = |frame: &[i16]| -> Option<Vec<u8>> {
            let opus = encoder.encode(frame)?;
            Some(match &key {
                Some(k) => k.encrypt(&opus),
                None => opus,
            })
        };
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
                let data = encoded.get_or_insert_with(|| encode(&processed)).clone();
                send(a, st, data.as_deref(), r == Result::End, &mut s);
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
                let data = encoded.get_or_insert_with(|| encode(&processed)).clone();
                send(&parent, st, data.as_deref(), parent_result == Result::End, &mut s);
                if parent_result == Result::Activated {
                    s.talking.push(parent.id());
                }
            }
        } else if st.active {
            st.active = false;
            send(&parent, st, None, true, &mut s);
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
        let mut frames: Vec<(u128, Vec<f32>)> = Vec::new();
        for (id, src) in s.sources.iter_mut() {
            let Some(info) = src.info.clone() else { continue };
            while let Some(frame) = src.jitter.poll(delay, now) {
                let decoder = src.decoder.get_or_insert_with(|| Decoder::new(sample_rate, false).expect("opus decoder"));
                let pcm = match frame {
                    Frame::Audio(data) => {
                        let plain = match &key {
                            Some(k) => k.decrypt(&data),
                            None => Some(data),
                        };
                        decoder.decode(plain.as_deref())
                    }
                    Frame::Lost => decoder.decode(None),
                    // What is queued still plays; the next talk starts a fresh decoder.
                    Frame::End => {
                        src.decoder = None;
                        break;
                    }
                };
                // Voice chat off (`sourceManager.clear`) or the source's line muted: nothing plays.
                let line_muted = lines.iter().find(|l| l.id() == info.line_id).is_some_and(|l| settings.muted_lines.contains(&l.name));
                if settings.voice_disabled || line_muted {
                    continue;
                }
                let (l, r) = gains(&info, &world, &settings, &lines, f64::from(src.distance));
                let mono = if decoder.channels() == 2 { pcm.chunks(2).map(|c| (c[0] + c[1]) / 2.0).collect() } else { pcm };
                let stereo: Vec<f32> = mono.iter().flat_map(|&v| [v * l, v * r]).collect();
                frames.push((id.0, stereo));
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

/// `updateSource`: the volume (master × source × line × distance, cubed as Plasmo does) and the
/// left/right gains from where the source is around the listener.
fn gains(info: &SourceInfo, world: &World, settings: &Settings, lines: &[SourceLine], max_distance: f64) -> (f32, f32) {
    let pos = match &info.kind {
        SourceKind::Player(p) => world.players.get(&p.id).copied(),
        SourceKind::Entity(e) => world.entities.get(e).copied(),
        SourceKind::Static { pos, .. } => Some([pos.x, pos.y, pos.z]),
        SourceKind::Direct { .. } => None,
    };
    let (player_volume, muted) = match &info.kind {
        SourceKind::Player(p) => (settings.player_volumes.get(&p.id).copied().unwrap_or(1.0), settings.muted_players.contains(&p.id)),
        _ => (1.0, false),
    };
    if muted {
        return (0.0, 0.0);
    }
    let line = lines.iter().find(|l| l.id() == info.line_id);
    let line_volume = line.map_or(1.0, |l| settings.line_volumes.get(&l.name).copied().unwrap_or(l.default_volume));
    let mut volume = settings.volume * player_volume * line_volume;
    if settings.exponential_volume_slider && volume < 1.0 {
        volume = volume.powi(3);
    }
    let Some(pos) = pos else {
        // Direct sources play in the head, at full volume.
        let v = volume.max(0.0) as f32;
        return (v, v);
    };
    let d = ((pos[0] - world.listener[0]).powi(2) + (pos[1] - world.listener[1]).powi(2) + (pos[2] - world.listener[2]).powi(2)).sqrt();
    if max_distance > 0.0 {
        let mut g = 1.0 - d.min(max_distance) / max_distance;
        if settings.exponential_distance_gain {
            g = g.powi(3);
        }
        volume *= g;
    }
    let v = volume.max(0.0) as f32;
    if !settings.panning || d < 0.01 {
        return (v, v);
    }
    // Minecraft's yaw: 0 looks to +Z, 90 to -X. The listener's right is (-cos, 0, -sin) of yaw.
    let yaw = f64::from(world.listener_yaw).to_radians();
    let (dx, dz) = (pos[0] - world.listener[0], pos[2] - world.listener[2]);
    let right = (-yaw.cos(), -yaw.sin());
    let horizontal = (dx * dx + dz * dz).sqrt().max(1e-6);
    let pan = ((dx * right.0 + dz * right.1) / horizontal).clamp(-1.0, 1.0) as f32;
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
        assert!(matches!(j.poll(40, t), Some(Frame::Audio(d)) if d == vec![1]));
        let t = t0 + Duration::from_millis(61);
        assert!(matches!(j.poll(40, t), Some(Frame::Lost)));
        let t = t0 + Duration::from_millis(81);
        assert!(matches!(j.poll(40, t), Some(Frame::Audio(d)) if d == vec![3]));
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
        // Looking to +Z (yaw 0), a player at -X is on the right; half the distance: (1-0.5)^3.
        let (l, r) = gains(&info, &world, &settings, &[], 16.0);
        assert!(r > l);
        assert!(((l * l + r * r).sqrt() - 0.125 * std::f32::consts::SQRT_2).abs() < 1e-4);
        world.players.insert(Uuid(7), [0.0, 0.0, 20.0]);
        assert_eq!(gains(&info, &world, &settings, &[], 16.0), (0.0, 0.0));
        // The settings' volume at 200% doubles the mix (the cube only applies under 100%), at
        // 50% it is cubed, and a player's own volume multiplies it.
        world.players.insert(Uuid(7), [-8.0, 0.0, 0.0]);
        let loud = Settings { volume: 2.0, ..Settings::default() };
        let (l2, r2) = gains(&info, &world, &loud, &[], 16.0);
        assert!((l2 - 2.0 * l).abs() < 1e-5 && (r2 - 2.0 * r).abs() < 1e-5);
        let quiet = Settings { volume: 0.5, ..Settings::default() };
        let (_, rq) = gains(&info, &world, &quiet, &[], 16.0);
        assert!((rq - 0.125 * r).abs() < 1e-5);
        let mut player = Settings::default();
        player.player_volumes.insert(Uuid(7), 2.0);
        assert!((gains(&info, &world, &player, &[], 16.0).1 - 2.0 * r).abs() < 1e-5);
        player.muted_players.push(Uuid(7));
        assert_eq!(gains(&info, &world, &player, &[], 16.0), (0.0, 0.0));
    }
}
