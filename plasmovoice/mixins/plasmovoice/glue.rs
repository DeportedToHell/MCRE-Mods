// Plasmo Voice in the game: `crate::game::plasmovoice_glue`.
//
// The client half (the window): the `plasmo:voice/v2` payloads to and from the server, the
// config (`config/plasmovoice/client.toml`), the hotkeys, the settings screen, the HUD, and each
// frame where the listener and the other players are. The server half (single player's server
// thread, a LAN host): the voice server for the guests, and for the host's own player, who has no
// network connection to it, through `BRIDGE` in memory.
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use super::Game;
use super::native_hooks::NativeMod;
use super::plasmovoice_config::{Config, V};
use super::plasmovoice_gui as gui;
use super::plasmovoice_hud as hud;
use super::plasmovoice_lang::{self as lang, t, tf};
use horizon_client::hud::HudBatch;
use horizon_client::mod_api::{
    ClientboundPlayPacket, NativeCommand, NativeScreenView, NativeServerPlayer, ServerTickPhase, ServerboundPlayPacket, Suggestion, WorldPhase,
};
use horizon_client::text::Text;
use horizon_protocol::packets::configuration::CustomPayload;
use horizon_protocol::rsa::{RsaPrivateKey, RsaPublicKey};
use plasmovoice_engine::client::{self, ActivationSettings, ActivationType, Client, Settings, World};
use plasmovoice_engine::server::{self, Position, Server};
use plasmovoice_engine::server_commands::{self, Sender};
use plasmovoice_engine::tcp::{self, Activation};
use plasmovoice_engine::wire::Uuid;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Instant;

pub(super) const ID: &str = "plasmovoice";
const NAME: &str = "Plasmo Voice";
const VERSION: &str = gui::VERSION;
/// `fabric.mod.json`'s description.
const DESCRIPTION: &str = "A proximity voice chat mod with audio positioning and lots of features";
/// The port's Russian for it (the original has none).
const DESCRIPTION_RU: &str = "Голосовой чат по расстоянию: звук с направлением и много возможностей";

const UNBOUND: &str = "key.keyboard.unknown";

/// The mod's keys in Options > Mods: name, label key, default, and the Plasmo Voice hotkey each
/// stands for (the menu key is vanilla's `key.plasmovoice.settings` mapping; the rest mirror
/// `ConfigHotkeys`, whose own binding, with combinations, is on the settings' Hotkeys tab).
const KEYS: [(&str, &str, &str, Option<&str>); 6] = [
    ("settings", "key.plasmovoice.settings", "key.keyboard.v", None),
    ("push_to_talk", "gui.plasmovoice.activation.ptt_button", "key.keyboard.left.alt", Some("key.plasmovoice.proximity.ptt")),
    ("mute_microphone", "key.plasmovoice.general.mute_microphone", "key.keyboard.m", Some("key.plasmovoice.general.mute_microphone")),
    ("disable_voice", "key.plasmovoice.general.disable_voice", UNBOUND, Some("key.plasmovoice.general.disable_voice")),
    ("action", "key.plasmovoice.general.action", "key.mouse.right", Some("key.plasmovoice.general.action")),
    ("occlusion_toggle", "key.plasmovoice.occlusion.toggle", UNBOUND, Some("key.plasmovoice.occlusion.toggle")),
];

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---- RSA ------------------------------------------------------------------------------------

/// The client's key pair (`ModServerConnection`: RSA 2048), made once on a thread at start.
static KEYS_PAIR: OnceLock<Arc<Rsa>> = OnceLock::new();
static KEYS_STARTED: std::sync::Once = std::sync::Once::new();

struct Rsa(RsaPrivateKey);

impl client::Rsa for Rsa {
    fn public_der(&self) -> Vec<u8> {
        self.0.public_der()
    }
    fn decrypt(&self, data: &[u8]) -> Option<Vec<u8>> {
        self.0.decrypt(data).ok()
    }
}

fn rsa() -> Option<Arc<Rsa>> {
    KEYS_STARTED.call_once(|| {
        let _ = std::thread::Builder::new().name("plasmovoice-rsa".into()).spawn(|| match RsaPrivateKey::generate(2048) {
            Ok(k) => {
                let _ = KEYS_PAIR.set(Arc::new(Rsa(k)));
            }
            Err(e) => log::warn!("[{NAME}] can't make the RSA key: {e}"),
        });
    });
    KEYS_PAIR.get().cloned()
}

struct Encrypt;

impl server::RsaEncrypt for Encrypt {
    fn encrypt(&self, public_der: &[u8], data: &[u8]) -> Option<Vec<u8>> {
        RsaPublicKey::from_der(public_der).ok()?.encrypt(data).ok()
    }
}

// ---- the host's own player, in memory -------------------------------------------------------

/// Between the window and its own server (single player, a LAN host).
#[derive(Default)]
struct Bridge {
    /// The window's player in its own world: UUID, name, dimension, position.
    host: Option<(u128, String, String, [f64; 3])>,
    to_server: Vec<Vec<u8>>,
    to_client: Vec<Vec<u8>>,
}

static BRIDGE: Mutex<Bridge> = Mutex::new(Bridge { host: None, to_server: Vec::new(), to_client: Vec::new() });

// ---- the config -----------------------------------------------------------------------------

struct Stored {
    config: Config,
    path: Option<std::path::PathBuf>,
    saved: u64,
    saved_at: Instant,
}

static CONFIG: Mutex<Option<Stored>> = Mutex::new(None);

/// `BaseVoiceClient.loadConfig`: `config/plasmovoice/client.toml` read (defaults for what it
/// lacks) and written back at once.
fn stored() -> MutexGuard<'static, Option<Stored>> {
    let mut g = lock(&CONFIG);
    if g.is_none() {
        let path = Game::native_config_dir().ok().map(|d| d.join("plasmovoice").join("client.toml"));
        let config = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).map(|s| Config::parse(&s)).unwrap_or_default();
        *g = Some(Stored { config, path, saved: 0, saved_at: Instant::now() });
    }
    g
}

/// The config, to read or change.
pub(super) fn config<R>(f: impl FnOnce(&mut Config) -> R) -> R {
    let mut g = stored();
    f(&mut g.as_mut().expect("loaded").config)
}

/// `VoiceClientConfig.save`: written when it changed.
fn save(force: bool) {
    let mut g = stored();
    let Some(s) = g.as_mut() else { return };
    if s.saved == s.config.revision || (!force && s.saved_at.elapsed().as_millis() < 1000) {
        return;
    }
    s.saved = s.config.revision;
    s.saved_at = Instant::now();
    let Some(path) = s.path.clone() else { return };
    let text = s.config.write();
    drop(g);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str())
        && let Err(e) = std::fs::write(&path, text)
    {
        log::warn!("[{NAME}] can't save {}: {e}", path.display());
    }
}

pub(super) fn save_now() {
    save(true);
}

/// The engine's settings from the config, for this server's activations and lines.
fn engine_settings(c: &Config, server: &str, activations: &[Activation], lines: &[tcp::SourceLine], test: bool) -> Settings {
    let mut s = Settings {
        voice_disabled: c.b("voice.disabled"),
        microphone_disabled: c.b("voice.microphone_disabled"),
        activation_threshold: c.f("voice.activation_threshold"),
        input_device: c.s("voice.input_device"),
        output_device: c.s("voice.output_device"),
        microphone_volume: c.f("voice.microphone_volume"),
        noise_suppression: c.b("voice.noise_suppression"),
        stereo_capture: c.b("voice.stereo_capture"),
        volume: c.f("voice.volume"),
        sound_occlusion: c.b("voice.sound_occlusion"),
        directional_sources: c.b("voice.directional_sources"),
        directional_sources_angle: c.i("advanced.directional_sources_angle") as i32,
        stereo_sources_to_mono: c.b("advanced.stereo_sources_to_mono"),
        source_types_overlap: client::Overlap::parse(&c.s("advanced.source_types_overlap")),
        camera_sound_listener: c.b("advanced.camera_sound_listener"),
        compressor_limiter: c.b("voice.compressor_limiter"),
        compressor_threshold: c.i("advanced.compressor_threshold") as i32,
        limiter_threshold: c.i("advanced.limiter_threshold") as i32,
        exponential_volume_slider: c.b("advanced.exponential_volume_slider"),
        exponential_distance_gain: c.b("advanced.exponential_distance_gain"),
        panning: c.b("advanced.panning"),
        jitter_packet_delay: c.i("advanced.jitter_packet_delay") as i32,
        disable_input_device: c.b("voice.disable_input_device"),
        microphone_test: test,
        language: lang::code(),
        ..Settings::default()
    };
    for l in lines {
        s.line_volumes.insert(l.name.clone(), c.f(&format!("voice.volumes.{}.volume", l.name)));
        if c.b(&format!("voice.volumes.{}.muted", l.name)) {
            s.muted_lines.push(l.name.clone());
        }
    }
    for path in c.paths_with_prefix("voice.volumes.source_") {
        let Some(rest) = path.strip_prefix("voice.volumes.source_") else { continue };
        let Some((uuid, field)) = rest.rsplit_once('.') else { continue };
        let Ok(id) = u128::from_str_radix(&uuid.replace('-', ""), 16) else { continue };
        match field {
            "volume" => {
                s.player_volumes.insert(Uuid(id), c.f(&path));
            }
            "muted" if c.b(&path) => s.muted_players.push(Uuid(id)),
            _ => {}
        }
    }
    for a in activations {
        let kind = ActivationType::parse(&c.s(&format!("activations.{}.type", a.name))).unwrap_or(ActivationType::PushToTalk);
        let distance = (!server.is_empty()).then(|| c.i(&gui::distance_path(server, a)) as i32);
        s.activations.insert(a.name.clone(), ActivationSettings { kind, toggle: c.b(&format!("activations.{}.toggle", a.name)), distance });
    }
    s
}

/// `VoiceClientActivationManager.register` and `VoiceClientSourceLineManager.register`: each
/// activation's config, distance and hotkeys, and each line's volume and overlay state.
fn register(c: &mut Config, server: &str, activations: &[Activation], lines: &[tcp::SourceLine]) {
    for a in activations {
        for f in ["type", "toggle"] {
            c.create(&format!("activations.{}.{f}", a.name));
        }
        if !server.is_empty() {
            let path = gui::distance_path(server, a);
            c.set_default(&path, V::I(i64::from(a.default_distance)));
            let d = gui::allowed_distance(a, c.i(&path));
            if d != c.i(&path) {
                c.set(&path, V::I(d));
            }
        }
        let ptt_default: &[&str] = &[];
        c.register_hotkey(&format!("key.plasmovoice.{}.ptt", a.name), ptt_default, "hidden", true);
        for suffix in ["toggle", "distance_increase", "distance_decrease"] {
            c.register_hotkey(&format!("key.plasmovoice.{}.{suffix}", a.name), &[], "hidden", false);
        }
    }
    for l in lines {
        // Horizon: When Talking for every line, so the overlay shows who talks out of the box
        // (Plasmo Voice defaults lines without players, proximity among them, to Off).
        c.set_default(&format!("overlay.source_states.{}", l.name), V::S("WHEN_TALKING".into()));
        c.set_default(&format!("voice.volumes.{}.volume", l.name), V::F(l.default_volume.clamp(0.0, 1.0)));
    }
}

// ---- the client half ------------------------------------------------------------------------

struct ClientSide {
    client: Option<Client>,
    /// `minecraft:register` went to this server.
    registered: bool,
    /// The activations and lines last registered, and when each line was last heard.
    known: (Vec<String>, Vec<String>, String),
    line_used: HashMap<String, Instant>,
}

static CLIENT: Mutex<Option<ClientSide>> = Mutex::new(None);

fn client_side() -> MutexGuard<'static, Option<ClientSide>> {
    let mut c = lock(&CLIENT);
    if c.is_none() {
        *c = Some(ClientSide { client: None, registered: false, known: Default::default(), line_used: HashMap::new() });
    }
    c
}

/// A text the server sent (`LanguagePacket`), for `lang::t`.
pub(super) fn server_text(key: &str) -> Option<String> {
    let guard = CLIENT.try_lock().ok()?;
    guard.as_ref()?.client.as_ref()?.translate(key)
}

/// The server's host and port as the player joined it (`host:port`, `[v6]:port`, or a bare host).
fn split_address(address: &str) -> (String, u16) {
    if let Some(rest) = address.strip_prefix('[')
        && let Some((host, port)) = rest.split_once(']')
    {
        return (host.to_owned(), port.trim_start_matches(':').parse().unwrap_or(25565));
    }
    match address.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') => (h.to_owned(), p.parse().unwrap_or(25565)),
        _ => (address.to_owned(), 25565),
    }
}

/// The window plays its own world (single player or its LAN host).
fn hosting(game: &Game<'_>) -> bool {
    matches!(
        game.native_client_state().and_then(|s| s.world),
        Some(horizon_client::mod_api::NativeWorldId::Singleplayer(_))
    )
}

/// A server payload on `plasmo:voice/v2`, or the host's from `BRIDGE`.
fn client_receive(game: &Game<'_>, side: &mut ClientSide, data: &[u8]) {
    if side.client.is_none() {
        // The key comes a second or two after start; the server asks again (1, 3, 5, 10, 15 s).
        let Some(keys) = rsa() else { return };
        let (host, port) = if hosting(game) {
            ("127.0.0.1".to_owned(), 0)
        } else {
            split_address(&game.native_client_state().and_then(|s| s.server_address).unwrap_or_default())
        };
        log::info!("[{NAME}] voice chat on this server, connecting");
        let settings = config(|c| engine_settings(c, "", &[], &[], false));
        side.client = Some(Client::new(keys, "26.3", &host, port, settings));
    }
    if let Some(c) = side.client.as_mut() {
        c.handle_tcp(data);
    }
}

/// Hook `client_packet`: a play packet from the server.
pub(super) fn packet(game: &mut Game<'_>, packet: &ClientboundPlayPacket) {
    let ClientboundPlayPacket::CustomPayload(CustomPayload { channel, data }) = packet else { return };
    if channel != tcp::CHANNEL || !game.native_on(ID) || hosting(game) {
        return;
    }
    let mut guard = client_side();
    let Some(side) = guard.as_mut() else { return };
    client_receive(game, side, data);
}

/// Hook `client_disconnecting`: the voice connection ends with the server's.
pub(super) fn disconnecting() {
    let mut guard = client_side();
    if let Some(side) = guard.as_mut() {
        side.client = None;
        side.registered = false;
    }
    lock(&BRIDGE).host = None;
}

// ---- hotkeys --------------------------------------------------------------------------------

/// `ConfigHotkeys`' pressed keys and hotkeys, `PlayerVolumeAction`, and what waits for `frame`.
#[derive(Default)]
struct Keys {
    pressed: HashSet<String>,
    down: HashSet<String>,
    open_menu: bool,
    action_bar: Vec<String>,
    /// The player looked at while the action key is held, and the last scroll.
    focused: Option<u128>,
    last_scroll: Option<Instant>,
    /// Horizon's binding and Plasmo Voice's, as last seen, per key of `KEYS`.
    synced: HashMap<&'static str, (String, String)>,
    /// The settings screen's clipboard keys.
    edits: Vec<Edit>,
}

static KEYS_STATE: Mutex<Option<Keys>> = Mutex::new(None);

fn keys<R>(f: impl FnOnce(&mut Keys) -> R) -> R {
    let mut g = lock(&KEYS_STATE);
    f(g.get_or_insert_with(Keys::default))
}

/// The key is held now (`Hotkeys.getPressedKeys`).
pub(super) fn held(key: &str) -> bool {
    keys(|k| k.pressed.contains(key))
}

/// `resetPressedStates`, when a hotkey starts being bound.
pub(super) fn reset_hotkeys() {
    keys(|k| k.down.clear());
}

/// Hook `key`: a key went down or up (`ConfigHotkeys.onKeyPressed`, `ModVoiceClient.onKeyPressed`).
pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if game.window.is_none() {
        return;
    }
    let playing = game.native_client_state().is_some_and(|s| s.screen == horizon_client::mod_api::NativeScreenKind::Playing);
    // A key let go that the screen's release hook didn't take: a hotkey being bound ends.
    if !pressed && gui::is_open() {
        config(|c| gui::with(|s| s.key_released(c, key)));
    }
    // The menu key (`MENU_KEY.consumeClick` with a player and no screen).
    if pressed && playing && game.native_player().is_some() && key == game.native_mod_key(ID, "settings", "key.keyboard.v") {
        keys(|k| k.open_menu = true);
    }
    let hotkeys = config(|c| c.hotkeys.clone());
    let mut events: Vec<(String, bool)> = Vec::new();
    keys(|k| {
        if pressed {
            k.pressed.insert(key.to_owned());
        } else {
            k.pressed.remove(key);
        }
        // `VoiceHotkey.updateState`.
        for h in hotkeys.iter().filter(|h| h.any_context || playing) {
            if h.keys.is_empty() {
                continue;
            }
            let all = h.keys.iter().all(|x| k.pressed.contains(x));
            if pressed && !k.down.contains(&h.name) && all {
                k.down.insert(h.name.clone());
                events.push((h.name.clone(), true));
            } else if k.down.contains(&h.name) && !all {
                k.down.remove(&h.name);
                events.push((h.name.clone(), false));
            }
        }
    });
    for (name, down) in events {
        on_hotkey(game, &name, down);
    }
}

/// The hotkeys' actions (`HotkeyActions`, `VoiceClientActivation`'s listeners, `PlayerVolumeAction`).
fn on_hotkey(game: &mut Game<'_>, name: &str, down: bool) {
    if name == "key.plasmovoice.general.action" {
        let focused = if down { player_by_sight(game) } else { None };
        keys(|k| {
            k.focused = focused;
            if !down {
                k.last_scroll = None;
            }
        });
        return;
    }
    if !down {
        return;
    }
    let message = config(|c| match name {
        "key.plasmovoice.general.mute_microphone" => {
            c.invert("voice.microphone_disabled");
            None
        }
        "key.plasmovoice.general.disable_voice" => {
            c.invert("voice.disabled");
            None
        }
        "key.plasmovoice.occlusion.toggle" => {
            c.invert("voice.sound_occlusion");
            Some(tf("message.plasmovoice.occlusion_changed", &[&on_off(c.b("voice.sound_occlusion"))]))
        }
        _ => {
            let rest = name.strip_prefix("key.plasmovoice.")?;
            let (activation, what) = rest.rsplit_once('.')?;
            let snap = activation_info(activation)?;
            let (a, server) = snap;
            match what {
                "toggle" => {
                    let kind = c.s(&format!("activations.{}.type", a.name));
                    if kind == "PUSH_TO_TALK" {
                        return None;
                    }
                    let path = format!("activations.{}.toggle", a.name);
                    c.invert(&path);
                    Some(tf("message.plasmovoice.activation.toggle", &[&t(&a.translation), &on_off(!c.b(&path))]))
                }
                "distance_increase" | "distance_decrease" if !a.distances.is_empty() && !server.is_empty() => {
                    let path = gui::distance_path(&server, &a);
                    let now = c.i(&path) as i32;
                    let n = a.distances.len() as i32;
                    let i = a.distances.iter().position(|d| *d == now).map_or(-1, |i| i as i32);
                    let next = if what == "distance_increase" { (i + 1).rem_euclid(n) } else if i - 1 < 0 { n - 1 } else { i - 1 };
                    let d = a.distances[next as usize];
                    c.set(&path, V::I(i64::from(d)));
                    Some(tf("message.plasmovoice.distance_changed", &[&t(&a.translation), &d.to_string()]))
                }
                _ => None,
            }
        }
    });
    if let Some(m) = message {
        keys(|k| k.action_bar.push(m));
    }
}

fn on_off(b: bool) -> String {
    t(if b { "message.plasmovoice.on" } else { "message.plasmovoice.off" })
}

/// The server's activation by name, and the server's id.
fn activation_info(name: &str) -> Option<(Activation, String)> {
    let guard = CLIENT.try_lock().ok()?;
    let c = guard.as_ref()?.client.as_ref()?;
    let server = c.status().server_id.map(|u| u.to_string()).unwrap_or_default();
    c.activations().into_iter().find(|a| a.name == name).map(|a| (a, server))
}

/// `PlayerVolumeAction.getPlayerBySight`: a block at a time along the look, until a solid block,
/// the first other visible player whose feet are in the 1 × 2 × 1 box under the point; only one
/// with voice chat.
fn player_by_sight(game: &Game<'_>) -> Option<u128> {
    let state = game.native_client_state()?;
    let eye = state.eye_position?;
    let (yaw, pitch) = (f64::from(state.yaw).to_radians(), f64::from(state.pitch).to_radians());
    let look = [-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()];
    let players: Vec<u128> = status()?.players.iter().map(|p| p.id.0).collect();
    let me = game.own_uuid();
    let entities = game.native_client_entities();
    let mut p = eye;
    for _ in 0..(game.settings.render_distance.max(2) as usize * 16) {
        p = [p[0] + look[0], p[1] + look[1], p[2] + look[2]];
        let block = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
        if game.native_client_block_obstruction(block).is_some_and(|(_, occludes)| occludes) {
            break;
        }
        let (min, max) = ([p[0] - 0.5, p[1] - 2.0, p[2] - 0.5], [p[0] + 0.5, p[1], p[2] + 0.5]);
        for e in &entities {
            let Some(uuid) = e.player_uuid else { continue };
            let q = e.position;
            if uuid != me && !e.invisible && (0..3).all(|i| q[i] >= min[i] && q[i] <= max[i]) {
                return players.contains(&uuid).then_some(uuid);
            }
        }
    }
    None
}

/// Hook `wheel`: with the action key held on a player, the wheel sets their volume by 5%
/// (`PlayerVolumeAction.onScroll`).
pub(super) fn wheel(game: &mut Game<'_>, steps: f32, claimed: &mut bool) {
    let playing = game.native_client_state().is_some_and(|s| s.screen == horizon_client::mod_api::NativeScreenKind::Playing);
    let Some(player) = keys(|k| k.focused) else { return };
    if !playing || steps == 0.0 {
        return;
    }
    keys(|k| k.last_scroll = Some(Instant::now()));
    config(|c| {
        let path = format!("voice.volumes.source_{}.volume", Uuid(player));
        let value = c.f(&path) + if steps > 0.0 { 0.05 } else { -0.05 };
        let stepped = ((value * 2.0 * 100.0 / 5.0 + 0.5).floor() * 5.0) / (2.0 * 100.0);
        c.set(&path, V::F(stepped));
    });
    *claimed = true;
}

/// `PlayerVolumeAction.isShown`: the player's volume shows over them for a second after a scroll.
pub(super) fn volume_shown(player: u128) -> bool {
    keys(|k| k.focused == Some(player) && k.last_scroll.is_some_and(|t| t.elapsed().as_millis() < 1000))
}

/// The mod's keys in Options > Mods and Plasmo Voice's hotkeys kept the same: a key changed on
/// one side is set on the other (a combination shows its first key there).
fn sync_keys(game: &mut Game<'_>) {
    for (name, _, default, hotkey) in KEYS {
        let Some(hotkey) = hotkey else { continue };
        let horizon = game.native_mod_key(ID, name, default);
        let plasmo = config(|c| c.hotkey(hotkey).map(|h| h.keys.join("+")).unwrap_or_default());
        let last = keys(|k| k.synced.get(name).cloned());
        let first = |s: &str| s.split('+').next().filter(|k| !k.is_empty()).unwrap_or(UNBOUND).to_owned();
        match last {
            // First frame: the config's binding wins (it is what Plasmo Voice saved).
            None => {
                if first(&plasmo) != horizon {
                    game.native_set_mod_key(ID, name, Some(&first(&plasmo)));
                }
                let now = game.native_mod_key(ID, name, default);
                keys(|k| k.synced.insert(name, (now, plasmo)));
            }
            Some((h, p)) if h != horizon => {
                let keys_now = if horizon == UNBOUND { Vec::new() } else { vec![horizon.clone()] };
                config(|c| c.set_hotkey(hotkey, keys_now));
                let plasmo = config(|c| c.hotkey(hotkey).map(|h| h.keys.join("+")).unwrap_or_default());
                keys(|k| k.synced.insert(name, (horizon, plasmo)));
                let _ = p;
            }
            Some((_, p)) if p != plasmo => {
                game.native_set_mod_key(ID, name, Some(&first(&plasmo)));
                let now = game.native_mod_key(ID, name, default);
                keys(|k| k.synced.insert(name, (now, plasmo)));
            }
            _ => {}
        }
    }
}

// ---- each frame -----------------------------------------------------------------------------

/// Hook `frame`, the window's: payloads out, the world in, the config, the screens.
pub(super) fn frame(game: &mut Game<'_>) {
    if game.window.is_none() {
        return;
    }
    let _ = rsa();
    write_data(game);
    super::plasmovoice_menu::frame(game);
    sync_keys(game);
    for m in keys(|k| std::mem::take(&mut k.action_bar)) {
        game.native_client_action_bar(Text::literal(m), false);
    }
    // The settings screen: gone when Escape or another screen closed it, its requests, opening.
    if gui::is_open() && game.native_mod_screen() != Some(ID) {
        gui::closed();
        save(true);
    }
    for r in gui::with(|s| std::mem::take(&mut s.requests)).unwrap_or_default() {
        match r {
            gui::Request::Click => game.audio.play_button_click(),
            gui::Request::Close => {
                game.native_mod_screen_close(ID);
                gui::closed();
                save(true);
            }
        }
    }
    if keys(|k| std::mem::take(&mut k.open_menu)) && game.native_on(ID) && game.native_mod_screen_open(ID) {
        gui::open();
    }
    for e in keys(|k| std::mem::take(&mut k.edits)) {
        match e {
            Edit::Paste => {
                let text = horizon_client::clipboard::Clipboard::get(&mut game.clipboard);
                config(|c| gui::with(|s| s.paste(c, &text)));
            }
            Edit::Copy(text) => {
                if !text.is_empty() {
                    horizon_client::clipboard::Clipboard::set(&mut game.clipboard, &text);
                }
            }
        }
    }

    let state = game.native_client_state();
    let in_world = game.native_player().is_some() && state.as_ref().is_some_and(|s| s.logged_in);
    let on = game.native_on(ID);
    let host = hosting(game);
    let mut guard = client_side();
    let Some(side) = guard.as_mut() else { return };
    if !in_world || !on {
        side.client = None;
        side.registered = false;
        lock(&BRIDGE).host = None;
        drop(guard);
        hud::set(hud::Snap::default());
        gui::set_info(gui::Info::default());
        save(false);
        return;
    }
    if host {
        let me = game.native_player();
        let (uuid, name) = (game.own_uuid(), game.own_profile_name());
        let incoming = {
            let mut b = lock(&BRIDGE);
            b.host = me.map(|p| (uuid, name, p.dimension.to_owned(), [p.x, p.y, p.z]));
            std::mem::take(&mut b.to_client)
        };
        for data in incoming {
            client_receive(game, side, &data);
        }
    } else if !side.registered {
        // Paper and the proxies only pass on channels the client registered (`minecraft:register`).
        let channels = format!("{}\0{}", tcp::CHANNEL, tcp::FLAG_CHANNEL).into_bytes();
        let p = ServerboundPlayPacket::CustomPayload(CustomPayload { channel: "minecraft:register".into(), data: channels });
        side.registered = game.native_send_packet(p).is_ok();
    }
    let me = game.own_uuid();
    let profiles = game.native_client_profiles();
    let skins: HashMap<u128, String> = profiles.iter().map(|p| (p.uuid, p.skin.clone())).collect();
    let Some(client) = side.client.as_mut() else {
        drop(guard);
        hud::set(hud::Snap { me, skins: skins.clone(), ..hud::Snap::default() });
        gui::set_info(gui::Info { me, skins, ..gui::Info::default() });
        save(false);
        return;
    };
    // Out: to the server, or to the host's own server in memory.
    let out = client.take_tcp();
    if host {
        lock(&BRIDGE).to_server.extend(out);
    } else {
        for data in out {
            let p = ServerboundPlayPacket::CustomPayload(CustomPayload { channel: tcp::CHANNEL.into(), data });
            if let Err(e) = game.native_send_packet(p) {
                log::debug!("[{NAME}] can't send: {e}");
            }
        }
    }
    // In: where the listener and the players are, and the push-to-talk keys.
    let partial = state.as_ref().map_or(1.0, |s| s.world_partial_tick);
    let mut world = World { in_world: true, me: Some(Uuid(me)), ..World::default() };
    if let Some(s) = &state {
        world.listener = s.eye_position.unwrap_or_default();
        world.listener_yaw = s.yaw;
        world.listener_pitch = s.pitch;
        world.typing = s.screen == horizon_client::mod_api::NativeScreenKind::Chat;
    }
    // The camera (`gameRenderer.mainCamera`): the ears with Camera Sound Listener.
    world.camera = match game.native_camera() {
        Some(c) => Some((c.position, c.yaw, c.pitch)),
        None => Some((game.view.position, game.view.y_rot, game.view.x_rot)),
    };
    for e in game.native_client_entities() {
        let at = e.position_at(partial);
        let eyes = [at[0], at[1] + f64::from(e.height) * 0.9, at[2]];
        // `getLookAngle` (`calculateViewVector` of the head's pitch and yaw).
        let (yaw, pitch) = (f64::from(e.head_yaw).to_radians(), f64::from(e.head_pitch).to_radians());
        let look = [-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()];
        if let Some(id) = e.player_uuid {
            world.players.insert(Uuid(id), eyes);
            world.player_looks.insert(Uuid(id), look);
        }
        world.entities.insert(e.id as i32, eyes);
        world.entity_looks.insert(e.id as i32, look);
    }

    let mut activations = client.activations();
    // `getActivations`' order: heaviest first, then by name.
    activations.sort_by(|a, b| b.weight.cmp(&a.weight).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    let mut lines = client.lines();
    lines.sort_by(|a, b| b.weight.cmp(&a.weight).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    let status = client.status();
    let server = status.server_id.map(|u| u.to_string()).unwrap_or_default();
    // A server that changed its distances (`/vreload`) registers them again, as a new Config does.
    let known = (activations.iter().map(|a| format!("{} {:?} {}", a.name, a.distances, a.default_distance)).collect(), lines.iter().map(|l| l.name.clone()).collect(), server.clone());
    if side.known != known {
        config(|c| register(c, &server, &activations, &lines));
        side.known = known;
    }
    let down = keys(|k| k.down.clone());
    world.held = activations.iter().filter(|a| down.contains(&format!("key.plasmovoice.{}.ptt", a.name))).map(|a| a.name.clone()).collect();
    let test = gui::with(|s| s.mic_test && s.page == gui::Page::Settings).unwrap_or(false);
    let settings = config(|c| engine_settings(c, &server, &activations, &lines, test));
    // Sound occlusion: each source's blocks between it and the player's eyes.
    if settings.sound_occlusion {
        for src in &status.sources {
            let (Some(pos), _) = client::source_position(&src.kind, &world) else { continue };
            if matches!(src.kind, tcp::SourceKind::Direct { camera_relative: true, .. }) {
                continue;
            }
            world.occlusion.insert(src.id, occluded_percent(game, pos, world.listener));
        }
    }
    client.update(world, &settings);
    // The distance spheres (`DistanceVisualizeStateExtractor.render`).
    let visualize = client.take_visualize();
    if !visualize.is_empty() {
        let render_distance = game.settings.render_distance.max(2) as i32 * 16;
        config(|c| {
            for mut v in visualize {
                if v.on_join {
                    if !c.b("advanced.visualize_voice_distance_on_join") {
                        continue;
                    }
                    // The player's distance for this server, which the settings know.
                    if let Some(a) = activations.iter().find(|a| a.name == tcp::PROXIMITY)
                        && !server.is_empty()
                    {
                        v.radius = c.i(&gui::distance_path(&server, a)) as i32;
                    }
                }
                if !c.b("advanced.visualize_voice_distance") || v.radius < 2 || v.radius > render_distance {
                    continue;
                }
                spheres(|list| {
                    // One sphere around the player at a time; ones at a place add up.
                    if v.position.is_none() {
                        list.retain(|s| s.position.is_some());
                    }
                    list.push(Sphere { radius: v.radius as f32, color: (v.color as u32) & 0x00FF_FFFF, position: v.position, created: Instant::now(), alpha: 150.0, last: Instant::now() });
                });
            }
        });
    }
    let status = client.status();
    log_status(&status, &settings);
    log_output(&status, &settings);
    for src in status.sources.iter().filter(|s| s.activated) {
        side.line_used.insert(src.line.clone(), Instant::now());
    }
    let direct_senders = status
        .sources
        .iter()
        .filter_map(|s| match &s.kind {
            tcp::SourceKind::Direct { sender: Some(p), .. } => Some((p.id.0, p.name.clone())),
            _ => None,
        })
        .collect();
    let info = gui::Info {
        connected: status.connected && status.server_id.is_some(),
        has_client: true,
        closed: status.error.is_some() && !status.connected,
        server_id: server,
        activations: activations.clone(),
        lines: lines.clone(),
        players: status.players.clone(),
        direct_senders,
        me,
        skins: skins.clone(),
        input_device: status.input_device.clone(),
        input_error: status.input_error.clone(),
        speakers: status.speakers.clone(),
        level: status.level,
        frames: status.frames,
        line_used: side.line_used.clone(),
    };
    drop(guard);
    gui::set_info(info);
    hud::set(hud::Snap { status, activations, lines, me, skins, server: true, ..hud::Snap::default() });
    let dragging = gui::with(|s| s.busy()).unwrap_or(false);
    if !dragging {
        save(false);
    }
}

/// The voice connection's state in the log, when it changes.
fn log_status(s: &client::Status, settings: &Settings) {
    static LAST: Mutex<String> = Mutex::new(String::new());
    let speaking: Vec<String> = s.sources.iter().filter(|x| x.activated).map(|x| x.name.clone().unwrap_or_else(|| x.line.clone())).collect();
    let line = format!(
        "connected {} ({:?}), timed out {}, talking {}, hearing {:?}, players {}, microphone {:?}{}; volume {:.2}, microphone volume {:.2}, threshold {:.0}, muted {}, voice off {}, test {}",
        s.connected,
        s.address,
        s.timed_out,
        !s.talking.is_empty(),
        speaking,
        s.players.len(),
        s.input_device,
        s.error.as_ref().map(|e| format!(", error: {e}")).unwrap_or_default(),
        settings.volume,
        settings.microphone_volume,
        settings.activation_threshold,
        settings.microphone_disabled,
        settings.voice_disabled,
        settings.microphone_test,
    );
    let mut last = lock(&LAST);
    if *last != line {
        log::info!("[{NAME}] {line}; played {} frames on {:?}", s.played, s.speakers);
        *last = line;
    }
}

/// While someone is heard, once a second: how loud the mix is, at what volume.
fn log_output(s: &client::Status, settings: &Settings) {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    if !s.sources.iter().any(|x| x.activated) {
        return;
    }
    let mut last = lock(&LAST);
    if last.is_some_and(|t| t.elapsed().as_secs() < 1) {
        return;
    }
    *last = Some(Instant::now());
    // Silence (a source out of range, muted) reads as -inf.
    log::info!("[{NAME}] hearing: output peak {:.1} dBFS at volume {:.2}, played {} frames", 20.0 * s.output_peak.log10(), settings.volume, s.played);
}

/// What the HUD shows: whether the player is talking now, and who is heard.
pub(super) fn status() -> Option<client::Status> {
    let guard = CLIENT.try_lock().ok()?;
    guard.as_ref()?.client.as_ref().map(Client::status)
}

// ---- in the world: sound occlusion and the distance spheres ---------------------------------

/// `SoundOcclusion.getOccludedPercent`: block by block from the sound to the listener (at most
/// 200 steps), each block crossed whose collision shape the line to the listener meets adds 0.5
/// (a solid block) or 0.25, a quarter of that after the first, up to 0.98.
fn occluded_percent(game: &Game<'_>, sound: [f64; 3], listener: [f64; 3]) -> f64 {
    const MULTIPLIER: f64 = 0.5;
    const MAX: f64 = 0.98;
    let floor = |p: [f64; 3]| [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
    let mut occluded = 0.0;
    let mut sound = [sound[0] + 0.01, sound[1] + 0.01, sound[2] + 0.01];
    if sound.iter().chain(&listener).any(|v| v.is_nan()) {
        return occluded;
    }
    let listener_pos = floor(listener);
    let mut sound_pos = floor(sound);
    for i in 1..=200 {
        let (prev_sound, prev_pos) = (sound, sound_pos);
        if sound.iter().any(|v| v.is_nan()) || sound_pos == listener_pos {
            return occluded;
        }
        let next: [i32; 3] = std::array::from_fn(|k| sound_pos[k] + i32::from(listener_pos[k] > sound_pos[k]));
        let diff: [f64; 3] = std::array::from_fn(|k| listener[k] - sound[k]);
        let change: [f64; 3] =
            std::array::from_fn(|k| if listener_pos[k] != sound_pos[k] { (f64::from(next[k]) - sound[k]) / diff[k] } else { f64::INFINITY });
        let mut offset = [0; 3];
        let axis = if change[0] < change[1] && change[0] < change[2] {
            0
        } else if change[1] < change[2] {
            1
        } else {
            2
        };
        let t = change[axis];
        sound = std::array::from_fn(|k| if k == axis { f64::from(next[k]) } else { sound[k] + diff[k] * t });
        if listener_pos[axis] < sound_pos[axis] {
            offset[axis] = -1;
        }
        sound_pos = floor(sound);
        for k in 0..3 {
            sound_pos[k] += offset[k];
        }
        if i <= 1 {
            continue;
        }
        // Air and shapes without collision let the sound through; else the line from here to
        // the listener has to meet the block's collision boxes (`VoxelShape.clip`).
        let boxes = game.native_block_collision(prev_pos);
        let base = prev_pos.map(f64::from);
        if !boxes.iter().any(|b| segment_hits_box(prev_sound, listener, [base[0] + b[0], base[1] + b[1], base[2] + b[2]], [base[0] + b[3], base[1] + b[4], base[2] + b[5]])) {
            continue;
        }
        // `isSolidRender`: a full block that hides what is behind it.
        let solid = game.native_client_block_obstruction(prev_pos).is_some_and(|(full, occludes)| full && occludes);
        let add = if solid { MULTIPLIER } else { MULTIPLIER / 2.0 };
        occluded += if occluded > 0.0 { add / 4.0 } else { add };
        if occluded > MAX {
            return MAX;
        }
    }
    occluded
}

/// The segment from `a` to `b` meets the box (`AABB.clip`, or starts in it).
fn segment_hits_box(a: [f64; 3], b: [f64; 3], min: [f64; 3], max: [f64; 3]) -> bool {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for k in 0..3 {
        let d = b[k] - a[k];
        if d.abs() < 1e-12 {
            if a[k] < min[k] || a[k] > max[k] {
                return false;
            }
            continue;
        }
        let (mut lo, mut hi) = ((min[k] - a[k]) / d, (max[k] - a[k]) / d);
        if lo > hi {
            std::mem::swap(&mut lo, &mut hi);
        }
        t0 = t0.max(lo);
        t1 = t1.min(hi);
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// A voice distance sphere being shown (`DistanceVisualizeStateExtractor.Entry`).
struct Sphere {
    radius: f32,
    color: u32,
    position: Option<[f64; 3]>,
    created: Instant,
    /// 150 at first, fading after two seconds.
    alpha: f32,
    last: Instant,
}

static SPHERES: Mutex<Vec<Sphere>> = Mutex::new(Vec::new());

fn spheres<R>(f: impl FnOnce(&mut Vec<Sphere>) -> R) -> R {
    f(&mut lock(&SPHERES))
}

/// Hook `native_world_primitives`: the spheres (`DistanceVisualizeRenderer`, Plasmo Voice's
/// translucent sphere of 18 × 72 quads, as the lines between its corners), around the player's
/// feet or at their place, fading 2 s after they appear.
pub(super) fn world_primitives(game: &Game<'_>, primitives: &mut horizon_client::world_primitives::WorldPrimitives) {
    if game.window.is_none() {
        return;
    }
    let on = config(|c| c.b("advanced.visualize_voice_distance"));
    let Some(player) = game.native_player() else {
        spheres(Vec::clear);
        return;
    };
    let camera = game.native_camera().map_or(game.view.position, |c| c.position);
    let render_distance = f64::from(game.settings.render_distance.max(2) as i32 * 16);
    spheres(|list| {
        list.retain(|s| {
            s.alpha > 0.0 && on && s.position.is_none_or(|p| ((p[0] - camera[0]).powi(2) + (p[1] - camera[1]).powi(2) + (p[2] - camera[2]).powi(2)).sqrt() <= render_distance)
        });
        for s in list.iter_mut() {
            // `alpha - 10 * delta`, delta in ticks: 200 a second.
            let dt = s.last.elapsed().as_secs_f32();
            s.last = Instant::now();
            if s.created.elapsed().as_millis() > 2000 {
                s.alpha = (s.alpha - 10.0 * dt * 20.0).max(0.0);
            }
            let center = s.position.unwrap_or([player.x, player.y, player.z]);
            let color = ((s.alpha as u32 & 0xFF) << 24) | s.color;
            const STACKS: usize = 18;
            const SLICES: usize = 72;
            let point = |stack: usize, slice: usize| {
                let a = -std::f64::consts::FRAC_PI_2 + stack as f64 * std::f64::consts::PI / STACKS as f64;
                let b = slice as f64 * std::f64::consts::TAU / SLICES as f64;
                let (r, y) = (f64::from(s.radius) * a.cos(), f64::from(s.radius) * a.sin());
                [center[0] + r * b.cos(), center[1] + y, center[2] - r * b.sin()]
            };
            for i in 0..STACKS {
                for j in 0..SLICES {
                    primitives.line(point(i, j), point(i + 1, j), color, 1.5);
                    if i > 0 {
                        primitives.line(point(i, j), point(i, j + 1), color, 1.5);
                    }
                }
            }
        }
    });
}

// ---- the screens' hooks ---------------------------------------------------------------------

/// What a key in a field asks of the clipboard, done in `frame`.
pub(super) enum Edit {
    Paste,
    Copy(String),
}

fn mine(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    screen.id == "mod" && game.native_mod_screen() == Some(ID) && gui::is_open()
}

/// Hook `screen_render_post`: the settings screen, or the Mod Menu screen over the options.
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    super::plasmovoice_menu::render(game, batch, screen);
    if mine(game, screen) {
        config(|c| gui::render(game, batch, c));
    }
}

fn size(screen: &NativeScreenView<'_>) -> (f32, f32) {
    (screen.rect[2].floor(), screen.rect[3].floor())
}

pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, modifiers: u32) -> bool {
    if super::plasmovoice_menu::pressed(game, screen, button, modifiers) {
        return true;
    }
    if !mine(game, screen) {
        return false;
    }
    let (w, h) = size(screen);
    let p = (screen.mouse[0], screen.mouse[1]);
    config(|c| gui::with(|s| s.press(&game.gui.atlas, c, w, h, p, button)));
    true
}

pub(super) fn released(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    if super::plasmovoice_menu::released(game, screen, button) {
        return true;
    }
    if !mine(game, screen) {
        return false;
    }
    let (w, h) = size(screen);
    config(|c| gui::with(|s| s.release(&game.gui.atlas, c, w, h, button)));
    true
}

pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    if super::plasmovoice_menu::dragged(game, screen) {
        return true;
    }
    if !mine(game, screen) {
        return false;
    }
    let (w, h) = size(screen);
    let p = (screen.mouse[0], screen.mouse[1]);
    config(|c| gui::with(|s| s.dragged(&game.gui.atlas, c, w, h, p)));
    true
}

pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    if super::plasmovoice_menu::scrolled(game, screen, scroll) {
        return true;
    }
    if !mine(game, screen) {
        return false;
    }
    gui::with(|s| s.scrolled(scroll[1] as f32));
    true
}

pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if super::plasmovoice_menu::key_pressed(game, screen, key, modifiers) {
        return true;
    }
    if !mine(game, screen) {
        return false;
    }
    let menu = game.native_mod_key(ID, "settings", "key.keyboard.v");
    let (taken, edit) = config(|c| gui::with(|s| s.key_pressed(&game.gui.atlas, c, key, modifiers, &menu))).unwrap_or((false, None));
    if let Some(e) = edit {
        keys(|k| k.edits.push(e));
    }
    taken
}

pub(super) fn key_released(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str) -> bool {
    if super::plasmovoice_menu::key_released(game, screen) {
        return true;
    }
    if !mine(game, screen) {
        return false;
    }
    config(|c| gui::with(|s| s.key_released(c, key))).unwrap_or(false)
}

pub(super) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, ch: char) -> bool {
    if super::plasmovoice_menu::char_typed(game, screen, ch) {
        return true;
    }
    if !mine(game, screen) {
        return false;
    }
    config(|c| {
        gui::with(|s| {
            let max = s.number_max();
            s.typed(c, &ch.to_string(), max)
        })
    })
    .unwrap_or(false)
}

/// Hook `hud`: the activation icon and the overlay (`HudRenderEvent`).
pub(super) fn draw_hud(game: &mut Game<'_>, batch: &mut HudBatch) {
    if game.window.is_none() || !game.native_on(ID) {
        return;
    }
    config(|c| {
        hud::overlay(game, batch, c);
        hud::activation_icon(game, batch, c);
    });
}

/// Hook `gui_layer_pre`: the icons over heads, under the HUD (before the hotbar).
pub(super) fn layer_pre(game: &mut Game<'_>, layer: &str, batch: &mut HudBatch) {
    if layer != "minecraft:hotbar" || game.window.is_none() || !game.native_on(ID) {
        return;
    }
    config(|c| hud::world_icons(game, batch, c, false));
}

/// Hook `gui_render_post`: with the HUD hidden (F1), the icons over heads when they show always.
pub(super) fn gui_post(game: &Game<'_>, batch: &mut HudBatch, gui_hidden: bool) {
    let playing = game.native_client_state().is_some_and(|s| s.screen == horizon_client::mod_api::NativeScreenKind::Playing);
    if !gui_hidden || !playing || game.window.is_none() || !game.native_on(ID) {
        return;
    }
    config(|c| hud::world_icons(game, batch, c, true));
}

// ---- Options > Mods and Mod Menu ------------------------------------------------------------

/// Hook `mods`: the mod's row in Options > Mods, with its keys.
pub(super) fn list(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: ID.into(),
        title: format!("{NAME} {VERSION}"),
        description: DESCRIPTION.into(),
        keys: KEYS.iter().map(|(name, label, default, _)| [(*name).to_owned(), t(label), (*default).to_owned()]).collect(),
    });
}

/// Hook `settings`: the mod's page in Options > Mods, Configure... first.
pub(super) fn page(mod_id: &str, labels: &mut Vec<String>) {
    if mod_id == ID {
        labels.push(if lang::russian() { "Настройки..." } else { "Configure..." }.into());
    }
}

/// Hook `settings_click`: Configure... on the page, or Mod Menu's Configure: Plasmo Voice's Mod
/// Menu screen (its Cloth Config screen).
pub(super) fn page_click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != ID {
        return;
    }
    if button == 0 || button == super::plasmovoice_menu::MODMENU_CONFIGURE {
        super::plasmovoice_menu::open(game, button != 0);
    }
}

/// The icon (the original's) and what Mod Menu shows of the mod.
const ICON: &[u8] = horizon_client::native_data!("plasmovoice", "icon.png");

fn modmenu_json() -> String {
    let description = serde_json::to_string(DESCRIPTION).unwrap_or_default();
    let description_ru = serde_json::to_string(DESCRIPTION_RU).unwrap_or_default();
    format!(
        r#"{{
  "schema": 1,
  "name": "{NAME}",
  "version": "{VERSION}",
  "description": {description},
  "description_ru": {description_ru},
  "authors": ["Plasmo"],
  "contributors": ["DeportedToHell"],
  "license": "LGPL-3.0",
  "links": {{
    "homepage": "https://github.com/plasmoapp/plasmo-voice",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/plasmovoice",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "badges": ["client"],
  "config_screen": true
}}
"#
    )
}

static WROTE_DATA: AtomicBool = AtomicBool::new(false);

/// `icon.png` and `modmenu.json` in the mod's data folder, once (each written only when it
/// differs), and the config written back as Plasmo Voice does after loading it.
fn write_data(game: &Game<'_>) {
    if WROTE_DATA.swap(true, Ordering::Relaxed) {
        return;
    }
    save(true);
    let dir = game.native_data_dir(ID);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let meta = modmenu_json();
    for (name, bytes) in [("icon.png", ICON), ("modmenu.json", meta.as_bytes())] {
        let path = dir.join(name);
        if std::fs::read(&path).ok().as_deref() != Some(bytes) {
            let _ = std::fs::write(&path, bytes);
        }
    }
}

/// Hook `render_name_tag`: whose name tags show (the icon goes over them).
pub(super) fn name_tag(entity: u32, visible: bool) {
    hud::name_tag(entity, visible);
}

// ---- the server half ------------------------------------------------------------------------
//
// Single player's server thread, a LAN host, and a dedicated server (`horizon-engine`, no
// window): Plasmo Voice's server, with its files in `config/plasmovoice/server/` as the Fabric
// mod keeps them (`config.toml`, `voice_mutes.json`, `languages/`), its commands, and its UDP
// server on `[host]`'s address (port 0: the Minecraft server's port on a dedicated server, any free
// port in a LAN world).

struct ServerSide {
    server: Server,
    joined: HashSet<u128>,
    /// The host's own player, joined through `BRIDGE`.
    host: Option<u128>,
    /// Ticks since the players' permissions and languages were last read.
    reads: u32,
}

static SERVER: Mutex<Option<ServerSide>> = Mutex::new(None);

/// Operators, for the commands and permissions Plasmo Voice gives `PermissionDefault.OP`:
/// vanilla's level for game-master commands. A permissions mod answers the nodes instead.
const OP_LEVEL: u8 = 2;

/// The dedicated server's port: the one it listens on, else `server-port` (`server.properties`,
/// vanilla's 25565 when unset).
fn server_port(game: &Game<'_>, dir: &std::path::Path) -> u16 {
    if let Some(port) = game.lan_port().filter(|p| *p != 0) {
        return port;
    }
    std::fs::read_to_string(dir.join("server.properties"))
        .ok()
        .and_then(|text| {
            text.lines()
                .filter_map(|l| l.split_once('='))
                .find(|(k, _)| k.trim() == "server-port")
                .and_then(|(_, v)| v.trim().parse().ok())
        })
        .unwrap_or(25565)
}

/// The names in the server's `usercache.json`, for `/vmutelist` and `/vunmute` of players who
/// aren't on (`getGameProfile`).
fn user_cache(dir: &std::path::Path) -> Vec<(Uuid, String)> {
    let Ok(text) = std::fs::read_to_string(dir.join("usercache.json")) else { return Vec::new() };
    let Ok(serde_json::Value::Array(entries)) = serde_json::from_str::<serde_json::Value>(&text) else { return Vec::new() };
    entries
        .iter()
        .filter_map(|e| {
            let id = plasmovoice_engine::server_config::parse_uuid(e.get("uuid")?.as_str()?)?;
            Some((id, e.get("name")?.as_str()?.to_owned()))
        })
        .collect()
}

/// Hook `world_lifecycle`: the voice server runs while the world does.
pub(super) fn world(game: &mut Game<'_>, phase: WorldPhase) {
    if game.window.is_some() {
        return;
    }
    let mut guard = lock(&SERVER);
    match phase {
        WorldPhase::WorldOpenReady => {
            if guard.is_none() && game.native_on(ID) {
                let folder = Game::native_config_dir().ok().map(|d| d.join(ID).join("server"));
                // Single player's server thread runs with the dedicated options too, so
                // `native_server_dir` answers there as well: the role tells them apart.
                let dedicated = (game.options.execution_role == super::execution_role::ExecutionRole::DedicatedServer)
                    .then(|| game.native_server_dir())
                    .flatten();
                let minecraft_port = dedicated.as_deref().map_or(0, |d| server_port(game, d));
                let options = server::Options { folder, minecraft_port };
                let mut server = Server::start(options, Arc::new(Encrypt));
                if let Some(dir) = &dedicated {
                    server.remember(user_cache(dir));
                }
                *guard = Some(ServerSide { server, joined: HashSet::new(), host: None, reads: 0 });
            }
        }
        WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed => {
            *guard = None;
            let mut b = lock(&BRIDGE);
            b.to_client.clear();
            b.to_server.clear();
        }
        _ => {}
    }
}

/// Hook `player_logged_in`: the server asks the player's client for its info.
pub(super) fn logged_in(game: &mut Game<'_>, player: u128, name: &str) {
    if game.window.is_some() || !game.native_on(ID) {
        return;
    }
    let mut guard = lock(&SERVER);
    let Some(side) = guard.as_mut() else { return };
    if side.host == Some(player) || !side.joined.insert(player) {
        return;
    }
    // The channels Plasmo Voice's clients send on, registered as a Paper server registers them.
    let channels = format!("{}\0{}", tcp::CHANNEL, tcp::FLAG_CHANNEL).into_bytes();
    let _ = game.native_send_custom_payload_to_player(player, "minecraft:register", &channels);
    side.server.join(Uuid(player), name);
}

/// Hook `player_logged_out`.
pub(super) fn logged_out(game: &mut Game<'_>, player: u128) {
    if game.window.is_some() {
        return;
    }
    let mut guard = lock(&SERVER);
    let Some(side) = guard.as_mut() else { return };
    if side.joined.remove(&player) {
        side.server.quit(Uuid(player));
    }
}

/// A guest's payload (`native_custom_payload_from_client`).
pub(super) fn payload(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if game.window.is_some() || channel != tcp::CHANNEL {
        return;
    }
    let mut guard = lock(&SERVER);
    if let Some(side) = guard.as_mut() {
        side.server.handle_tcp(Uuid(player), data);
    }
}

/// Hook `server_tick`, at its end: positions, permissions and languages in; payloads, chat lines
/// and kicks out.
pub(super) fn server_tick(game: &mut Game<'_>, phase: ServerTickPhase) {
    if game.window.is_some() || !matches!(phase, ServerTickPhase::ServerTickEnd { .. }) {
        return;
    }
    let mut guard = lock(&SERVER);
    let Some(side) = guard.as_mut() else { return };
    let (host, from_host) = {
        let mut b = lock(&BRIDGE);
        (b.host.clone(), std::mem::take(&mut b.to_server))
    };
    // The host's own player joins and leaves with the window.
    match (&host, side.host) {
        (Some((uuid, name, ..)), None) => {
            side.host = Some(*uuid);
            // Told of as a player too (`player_logged_in`): their requests go through BRIDGE now.
            if !side.joined.remove(uuid) {
                side.server.join(Uuid(*uuid), name);
            }
        }
        (None, Some(uuid)) => {
            side.host = None;
            side.server.quit(Uuid(uuid));
        }
        _ => {}
    }
    if let Some(uuid) = side.host {
        for data in from_host {
            side.server.handle_tcp(Uuid(uuid), &data);
        }
    }
    let players = game.native_server_players();
    // Once a second: what each player may do (`pv.activation.proximity`, `pv.allow_freecam`,
    // `pv.bypass_mod_requirement`), and their game's language for the server's messages.
    side.reads += 1;
    if side.reads >= 20 {
        side.reads = 0;
        for p in &players {
            let perms = server::Perms {
                proximity: game.native_has_permission(p.uuid, "pv.activation.proximity", 0),
                allow_freecam: game.native_has_permission(p.uuid, "pv.allow_freecam", 0),
                bypass_mod_requirement: game.native_has_permission(p.uuid, "pv.bypass_mod_requirement", OP_LEVEL),
            };
            let language = game.native_player_client(p.uuid).map(|c| c.language).filter(|l| !l.is_empty());
            side.server.set_player(Uuid(p.uuid), language, perms);
        }
    }
    let mut positions: Vec<(Uuid, Position)> = players
        .into_iter()
        .map(|p| (Uuid(p.uuid), Position { world: p.dimension, pos: [p.position[0], p.position[1] + f64::from(p.eye_height), p.position[2]] }))
        .collect();
    if let Some((uuid, _, dimension, pos)) = host {
        positions.push((Uuid(uuid), Position { world: dimension, pos }));
    }
    side.server.update(&positions);
    let host = side.host;
    let out = side.server.take_tcp();
    let tells = side.server.take_tells();
    let kicks = side.server.take_kicks();
    drop(guard);
    for (to, data) in out {
        if Some(to.0) == host {
            lock(&BRIDGE).to_client.push(data);
        } else if let Err(e) = game.native_send_custom_payload_to_player(to.0, tcp::CHANNEL, &data) {
            log::debug!("[{NAME}] can't send to {to}: {e}");
        }
    }
    for (to, line) in tells {
        game.native_tell(to.0, Text::literal(line));
    }
    for (to, reason) in kicks {
        game.native_kick(to.0, &Text::literal(reason));
    }
}

fn ours(command: &str) -> bool {
    let label = command.trim_start_matches('/').split(' ').next().unwrap_or_default();
    Server::command_names().contains(&label)
}

/// Hook `register_commands`: `/vlist`, `/vmute`, `/vunmute`, `/vmutelist`, `/vreload`, `/vrc`,
/// each decided by its permission node (`pv.list`...), whose default is everyone or operators.
pub(super) fn register_commands(game: &mut Game<'_>, commands: &mut Vec<NativeCommand>) {
    if game.window.is_some() || !game.native_on(ID) {
        return;
    }
    for c in server_commands::COMMANDS {
        let level = match c.default {
            server_commands::Default::Everyone => 0,
            server_commands::Default::Op => OP_LEVEL,
        };
        commands.push(NativeCommand::op(c.usage, level).permission(c.permission));
    }
}

/// Runs one of the commands and answers its sender.
fn run_command(game: &mut Game<'_>, sender: &Sender, command: &str) -> bool {
    if game.window.is_some() || !ours(command) {
        return false;
    }
    let lines = {
        let mut guard = lock(&SERVER);
        let Some(side) = guard.as_mut() else { return false };
        side.server.command(sender, command)
    };
    let Some(lines) = lines else { return false };
    for line in lines {
        game.native_feedback(Text::literal(line));
    }
    true
}

/// Hook `perform_command`: a player's command (their level or permission node already allowed).
pub(super) fn command(game: &mut Game<'_>, player: &NativeServerPlayer, command: &str, ran: &mut bool) {
    let sender = Sender::Player { id: Uuid(player.uuid), name: player.name.clone() };
    if run_command(game, &sender, command) {
        *ran = true;
    }
}

/// Hook `perform_console_command`: the dedicated server's console.
pub(super) fn console_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    if run_command(game, &Sender::Console, command) {
        *ran = true;
    }
}

/// `native_tab_complete_v2`: `/vmute`'s durations and `/vunmute`'s muted players.
pub(super) fn complete(game: &mut Game<'_>, label: &str, arguments: &[String], range: [usize; 2], suggestions: &mut Vec<Suggestion>) {
    if game.window.is_some() || !ours(label) {
        return;
    }
    let guard = lock(&SERVER);
    let Some(side) = guard.as_ref() else { return };
    let Some(list) = side.server.complete(label, arguments) else { return };
    suggestions.clear();
    suggestions.extend(list.into_iter().map(|text| Suggestion { range: (range[0], range[1]), text, tooltip: None, int_value: None }));
}
