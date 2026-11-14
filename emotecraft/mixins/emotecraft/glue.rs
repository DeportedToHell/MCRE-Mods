// Emotecraft in the game: `crate::game::emotecraft_glue`. The client half (the window): the
// emotes (`EmoteHolder.list`), the config, the keys, each avatar's emote (`EmotePlayer`, ticked
// with the game and posed every frame), the camera, the stop rules, the songs, and the
// `emotecraft:emote` messages to and from the server (`ClientNetwork`, `BaseClientNetwork`,
// `ClientPacketManager`), or, for single player and a LAN host, to the window's own server half
// through `BRIDGE` in memory. The server half is `emotecraft_server`, the screens
// `emotecraft_ui`.
//
// This Source Code Form is a port of Emotecraft (https://github.com/KosmX/emotes), GPL-3.0.

use super::Game;
use super::emotecraft_lang::{self as lang, t};
use super::emotecraft_ui as ui;
use super::native_hooks::NativeMod;
use super::perspective::CameraType;
use horizon_client::mobs::anim::emotecraft_pose as pose;
use emotecraft_engine::anim::Animation;
use emotecraft_engine::config::Config;
use emotecraft_engine::controller::Controller;
use emotecraft_engine::emotes::{self, BuiltIn, Loaded};
use emotecraft_engine::packet::{self, Bound, NetData, Task, Versions};
use horizon_client::mod_api::{ClientboundPlayPacket, ServerboundPlayPacket};
use horizon_client::text::Text;
use horizon_protocol::packets::configuration::CustomPayload;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

pub(super) const ID: &str = "emotecraft";
pub(super) const NAME: &str = "Emotecraft";
pub(super) const VERSION: &str = "1.0.2";
const UNBOUND: &str = "key.keyboard.unknown";

/// The mod's keys in Options > Mods (`EmotecraftClientMod`'s key mappings).
pub(super) const KEYS: [(&str, &str, &str); 2] = [("fastchoose", "key.emotecraft.fastchoose", "key.keyboard.b"), ("stop", "key.emotecraft.stop", UNBOUND)];

pub(super) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// An engine frame as the model hooks read it.
pub(super) struct FrameAdapter(pub emotecraft_engine::controller::Frame);

impl pose::Frame for FrameAdapter {
    fn transform(&self, name: &str, bone: &mut pose::Bone) {
        let mut b = emotecraft_engine::controller::Bone { pos: bone.pos, rot: bone.rot, scale: bone.scale, bend: bone.bend };
        self.0.transform(name, &mut b);
        *bone = pose::Bone { pos: b.pos, rot: b.rot, scale: b.scale, bend: b.bend };
    }
}

pub(super) type Frames = HashMap<u32, Arc<dyn pose::Frame>>;

// ---- the built-in emotes --------------------------------------------------------------------

const BUILT_IN: [(&str, &[u8], &[u8]); 14] = [
    ("waving", horizon_client::native_data!("emotecraft", "emotes/waving.json"), horizon_client::native_data!("emotecraft", "emotes/waving.png")),
    ("clap", horizon_client::native_data!("emotecraft", "emotes/clap.json"), horizon_client::native_data!("emotecraft", "emotes/clap.png")),
    ("crying", horizon_client::native_data!("emotecraft", "emotes/crying.json"), horizon_client::native_data!("emotecraft", "emotes/crying.png")),
    ("point", horizon_client::native_data!("emotecraft", "emotes/point.json"), horizon_client::native_data!("emotecraft", "emotes/point.png")),
    ("here", horizon_client::native_data!("emotecraft", "emotes/here.json"), horizon_client::native_data!("emotecraft", "emotes/here.png")),
    ("palm", horizon_client::native_data!("emotecraft", "emotes/palm.json"), horizon_client::native_data!("emotecraft", "emotes/palm.png")),
    ("backflip", horizon_client::native_data!("emotecraft", "emotes/backflip.json"), horizon_client::native_data!("emotecraft", "emotes/backflip.png")),
    ("roblox_potion_dance", horizon_client::native_data!("emotecraft", "emotes/roblox_potion_dance.json"), horizon_client::native_data!("emotecraft", "emotes/roblox_potion_dance.png")),
    ("kazotsky_kick", horizon_client::native_data!("emotecraft", "emotes/kazotsky_kick.json"), horizon_client::native_data!("emotecraft", "emotes/kazotsky_kick.png")),
    ("twerk", horizon_client::native_data!("emotecraft", "emotes/twerk.json"), horizon_client::native_data!("emotecraft", "emotes/twerk.png")),
    ("club_penguin_dance", horizon_client::native_data!("emotecraft", "emotes/club_penguin_dance.json"), horizon_client::native_data!("emotecraft", "emotes/club_penguin_dance.png")),
    ("take_the_l", horizon_client::native_data!("emotecraft", "emotes/take_the_l.json"), horizon_client::native_data!("emotecraft", "emotes/take_the_l.png")),
    ("caramell_dance", horizon_client::native_data!("emotecraft", "emotes/caramell_dance.json"), horizon_client::native_data!("emotecraft", "emotes/caramell_dance.png")),
    ("lets_groove", horizon_client::native_data!("emotecraft", "emotes/lets_groove.json"), horizon_client::native_data!("emotecraft", "emotes/lets_groove.png")),
];

pub(super) fn built_ins() -> Vec<BuiltIn> {
    BUILT_IN
        .iter()
        .map(|(name, json, icon)| BuiltIn {
            name,
            json: std::str::from_utf8(json).unwrap_or_default(),
            icon,
            song: super::emotecraft_songs::SONGS.iter().find(|(n, _)| n == name).map(|(_, b)| *b),
        })
        .collect()
}

pub(super) const FOLDER_ICON: &[u8] = horizon_client::native_data!("emotecraft", "folder.png");
pub(super) const FOLDER_OPEN_ICON: &[u8] = horizon_client::native_data!("emotecraft", "folder_open.png");
pub(super) const ICON: &[u8] = horizon_client::native_data!("emotecraft", "icon.png");

// ---- the emotes -----------------------------------------------------------------------------

/// `EmoteHolder`: an emote of the list and where it came from.
#[derive(Clone)]
pub(super) struct Holder {
    pub anim: Arc<Animation>,
    /// Sent by the server (`fromInstance`); dropped when leaving it.
    pub from_server: bool,
}

/// `EmoteHolder.list`, in the order the emotes came.
static LIST: Mutex<Vec<Holder>> = Mutex::new(Vec::new());
/// The emotes' folder loading on a thread (`EmotecraftClientMod.loadEmotes`).
static LOADED: Mutex<Option<Loaded>> = Mutex::new(None);
static LOADING: AtomicBool = AtomicBool::new(false);

pub(super) fn list() -> Vec<Holder> {
    lock(&LIST).clone()
}

pub(super) fn find(uuid: u128) -> Option<Arc<Animation>> {
    lock(&LIST).iter().find(|h| h.anim.uuid == uuid).map(|h| h.anim.clone())
}

/// `EmoteHolder.addEmoteToList`: an equal one already there stays.
fn add_emote(anim: Arc<Animation>, from_server: bool) {
    let mut l = lock(&LIST);
    if l.iter().any(|h| h.anim.uuid == anim.uuid && *h.anim == *anim) || l.iter().any(|h| *h.anim == *anim) {
        return;
    }
    l.push(Holder { anim, from_server });
}

/// When a reload the screens asked for started (`EmoteListener.load` toasts its time).
static RELOAD_STARTED: Mutex<Option<Instant>> = Mutex::new(None);

/// Loads the emotes again: at start, and when the screens ask (`EmoteListener.load`, with its
/// toasts).
pub(super) fn reload_emotes(toast: bool) {
    if LOADING.swap(true, Ordering::AcqRel) {
        return;
    }
    if toast {
        *lock(&RELOAD_STARTED) = Some(Instant::now());
        ui::toast(Text::literal(NAME), Text::literal(t("emotecraft.reloading")));
    }
    let (builtin, dir) = client(|c| (c.config.b("loadbuiltin"), c.emotes_dir.clone()));
    let spawned = std::thread::Builder::new().name("Emotecraft emote loading".into()).spawn(move || {
        let loaded = emotes::load(&built_ins(), builtin, &dir, false);
        *lock(&LOADED) = Some(loaded);
        LOADING.store(false, Ordering::Release);
    });
    if spawned.is_err() {
        LOADING.store(false, Ordering::Release);
    }
}

pub(super) fn loading() -> bool {
    LOADING.load(Ordering::Acquire)
}

// ---- the host's own player, in memory -------------------------------------------------------

/// Between the window and its own server (single player, a LAN host).
#[derive(Default)]
pub(super) struct Bridge {
    /// The window's player in its own world.
    pub host: Option<(u128, String)>,
    pub to_server: Vec<Vec<u8>>,
    pub to_client: Vec<Vec<u8>>,
}

pub(super) static BRIDGE: Mutex<Bridge> = Mutex::new(Bridge { host: None, to_server: Vec::new(), to_client: Vec::new() });

// ---- the client's state ---------------------------------------------------------------------

/// An avatar's emote layer (`EmotePlayer` on an `Avatar`) and its age for the queue's delay.
pub(super) struct Avatar {
    pub ctrl: Controller,
    pub age: u32,
}

/// `ClientNetwork` (the server's) state.
#[derive(Default)]
struct Net {
    /// The server registered `emotecraft:emote` (or answered).
    channel_open: bool,
    /// The server's CONFIG came (`isConfiguredNormally`).
    configured: bool,
    versions: Versions,
    register_sent: bool,
    config_sent: bool,
    joined: Option<Instant>,
    hosting: bool,
}

pub(super) struct Client {
    pub config: Config,
    config_path: Option<PathBuf>,
    pub emotes_dir: PathBuf,
    saved_at: Instant,
    /// Avatars by uuid (the own player's too).
    pub avatars: HashMap<u128, Avatar>,
    /// `BaseClientNetwork.QUEUE`: emotes for players not loaded yet.
    queue: HashMap<u128, (Arc<Animation>, f32, u64)>,
    /// The own emote can't be stopped by the player (`emotecraft$isForced`).
    pub forced: bool,
    /// `EmotePlayer.perspective`: the camera was moved for the emote.
    perspective: bool,
    net: Net,
    pub tick: u64,
    songs: emotecraft_engine::audio::Songs,
    /// What the own player last did, for the stop rules.
    own: u128,
    in_world: bool,
}

static CLIENT: Mutex<Option<Client>> = Mutex::new(None);

fn new_client() -> Client {
    let config_path = Game::native_config_dir().ok().map(|d| d.join("emotecraft.json"));
    let mut config = match config_path.as_ref().map(std::fs::read_to_string) {
        Some(Ok(text)) => Config::parse(&text, true).unwrap_or_else(|e| {
            log::warn!("[{NAME}] Failed to read config! {e}");
            Config::new(true)
        }),
        _ => {
            let mut c = Config::new(true);
            c.dirty = true;
            c
        }
    };
    if config.version != emotecraft_engine::config::CONFIG_VERSION {
        config.dirty = true;
    }
    let dir = std::env::current_dir().unwrap_or_default();
    Client {
        emotes_dir: dir.join(config.s("emotesDirectory")),
        config,
        config_path,
        saved_at: Instant::now(),
        avatars: HashMap::new(),
        queue: HashMap::new(),
        forced: false,
        perspective: false,
        net: Net::default(),
        tick: 0,
        songs: Default::default(),
        own: 0,
        in_world: false,
    }
}

/// The client's state, to read or change.
pub(super) fn client<R>(f: impl FnOnce(&mut Client) -> R) -> R {
    let mut g = lock(&CLIENT);
    f(g.get_or_insert_with(new_client))
}

pub(super) fn config<R>(f: impl FnOnce(&mut Config) -> R) -> R {
    client(|c| f(&mut c.config))
}

/// `Serializer.saveConfig`.
pub(super) fn save_config() {
    let (path, text) = client(|c| {
        c.config.dirty = false;
        c.saved_at = Instant::now();
        (c.config_path.clone(), c.config.write())
    });
    let Some(path) = path else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str())
        && let Err(e) = std::fs::write(&path, text)
    {
        log::warn!("[{NAME}] Failed to save config! {e}");
    }
}

// ---- sending --------------------------------------------------------------------------------

/// The window plays its own world (single player or its LAN host).
pub(super) fn hosting(game: &Game<'_>) -> bool {
    matches!(game.native_client_state().and_then(|s| s.world), Some(horizon_client::mod_api::NativeWorldId::Singleplayer(_)))
}

/// A message to the server: through `BRIDGE` to the window's own, else as a custom payload of
/// any size the channel allows. `native_send_packet` refuses past vanilla's 32767 bytes, which
/// Emotecraft's servers raise to 1 MiB on its channel, so the payload goes out through the
/// game's own `send`.
fn send_raw(game: &mut Game<'_>, channel: &str, data: Vec<u8>) {
    if hosting(game) {
        if channel == packet::CHANNEL {
            lock(&BRIDGE).to_server.push(data);
        }
        return;
    }
    if game.remote.is_none() {
        return;
    }
    game.send(ServerboundPlayPacket::CustomPayload(CustomPayload { channel: channel.into(), data }));
}

/// `ClientPacketManager.isInstanceOutdatedForStreaming`.
fn outdated(v: &Versions) -> bool {
    let d = packet::default_versions();
    [packet::NEW_ANIMATION_FORMAT, packet::NBS_CONFIG].iter().any(|id| v.get(id).is_none_or(|x| x < &d[id]))
}

/// What the wheel's header says (`FastMenuScreen.init`).
pub(super) enum ServerState {
    Fine,
    Different,
    None,
}

pub(super) fn server_state() -> ServerState {
    client(|c| {
        if c.net.hosting || c.net.channel_open || c.net.configured {
            if c.net.configured && outdated(&c.net.versions) { ServerState::Different } else { ServerState::Fine }
        } else {
            ServerState::None
        }
    })
}

/// `ClientPacketManager.send` / `ClientNetwork.sendMessage`: to the server in its versions
/// (the sender's id left out: the server tracks who plays), with optional parts dropped past
/// the payload limit, and the "song is too big" toast.
fn send_data(game: &mut Game<'_>, mut data: NetData) {
    let (active, versions) = client(|c| (c.net.hosting || c.net.channel_open || c.net.configured, c.net.versions.clone()));
    if !active {
        return;
    }
    if !versions.is_empty() {
        data.versions = versions;
    }
    data.player = None;
    data.size_limit = packet::MAX_PACKET_SIZE - 16;
    data.strict_size_limit = false;
    match data.write(Bound::Server) {
        Ok(bytes) => {
            if data.skipped.contains(&packet::NBS_CONFIG) && data.emote.as_ref().is_some_and(|e| e.sound.is_some() || e.nbs.is_some()) {
                ui::toast(Text::literal(NAME), Text::literal(t("emotecraft.song_too_big_to_send")));
            }
            send_raw(game, packet::CHANNEL, bytes);
        }
        Err(e) => log::error!("[{NAME}] Error while sending packet! {e}"),
    }
}

// ---- playing --------------------------------------------------------------------------------

/// Things the screens and keys ask for that need the game.
pub(super) enum Action {
    Play(Arc<Animation>),
}

static ACTIONS: Mutex<Vec<Action>> = Mutex::new(Vec::new());

pub(super) fn act(a: Action) {
    lock(&ACTIONS).push(a);
}

fn camera_from(name: &str) -> CameraType {
    match name {
        "FIRST_PERSON" => CameraType::FirstPerson,
        "THIRD_PERSON_BACK" => CameraType::ThirdPersonBack,
        _ => CameraType::ThirdPersonFront,
    }
}

/// `EmoteHolder.canRunEmote`: the player hasn't moved more than the stop speed this tick.
fn can_run(game: &Game<'_>) -> bool {
    let Some(p) = game.player.as_ref() else { return false };
    let (y_ratio, threshold) = config(|c| (f64::from(c.f("yratio")), f64::from(c.f("stopthreshold"))));
    let (pos, old) = (p.position, game.previous_position);
    let y = old.y + y_ratio * (pos.y - old.y);
    let d = ((pos.x - old.x).powi(2) + (pos.y - y).powi(2) + (pos.z - old.z).powi(2)).sqrt();
    d <= threshold
}

/// `ClientEmotePlay.clientStartLocalEmote`: to the server, then on the own player, the camera
/// moved to the configured view from first person.
fn start_local(game: &mut Game<'_>, anim: Arc<Animation>, tick: f32) -> bool {
    if client(|c| c.forced && c.avatars.get(&c.own).is_some_and(|a| a.ctrl.is_active())) {
        return false;
    }
    let own = game.own_uuid();
    send_data(game, NetData::stream(anim.clone(), Some(own), tick));
    // Horizon's own emote steps aside.
    game.stop_emote();
    play_on(game, own, anim, tick, false);
    true
}

/// `emotecraft$playEmote` on an avatar.
fn play_on(game: &mut Game<'_>, uuid: u128, anim: Arc<Animation>, tick: f32, forced: bool) {
    let own = game.own_uuid();
    let to = camera_from(&config(|c| c.s("cameraType")));
    let mut init_perspective = false;
    client(|c| {
        let a = c.avatars.entry(uuid).or_insert_with(|| Avatar { ctrl: Controller::new(), age: 2 });
        a.ctrl.stop();
        a.ctrl.play(anim, None, tick);
        if uuid == own {
            c.forced = forced;
            // `initEmotePerspective`.
            if !to.first_person() && game.camera_type.first_person() {
                c.perspective = true;
                init_perspective = true;
            }
        }
    });
    if init_perspective {
        game.camera_type = to;
        game.check_entity_post_effect(false);
    }
}

/// `EmotePlayer.internalStop` for the own player: back to first person if the emote moved the
/// camera and the player didn't change it since.
fn internal_stop(game: &mut Game<'_>) {
    let to = camera_from(&config(|c| c.s("cameraType")));
    let back = client(|c| {
        let b = c.perspective && !to.first_person() && game.camera_type == to;
        if b {
            c.perspective = false;
        }
        b
    });
    if back {
        game.camera_type = CameraType::FirstPerson;
        game.check_entity_post_effect(true);
    }
}

/// `ClientEmotePlay.clientStopLocalEmote`.
fn stop_local(game: &mut Game<'_>) -> bool {
    let own = game.own_uuid();
    let current = client(|c| {
        if c.forced {
            return None;
        }
        c.avatars.get(&own).filter(|a| a.ctrl.is_active()).and_then(|a| a.ctrl.current().cloned())
    });
    let Some(a) = current else { return false };
    send_data(game, NetData::stop(a.uuid, Some(own)));
    client(|c| {
        if let Some(av) = c.avatars.get_mut(&own) {
            av.ctrl.stop();
        }
    });
    internal_stop(game);
    true
}

/// The own emote now (`getCurrentAnimationInstance` while running).
pub(super) fn own_emote() -> Option<Arc<Animation>> {
    client(|c| c.avatars.get(&c.own).filter(|a| a.ctrl.is_active()).and_then(|a| a.ctrl.current().cloned()))
}

/// `EmoteHolder.playEmote`: if the player may play now.
pub(super) fn play_emote(game: &mut Game<'_>, anim: Arc<Animation>) -> bool {
    if game.player.is_none() || !can_run(game) {
        return false;
    }
    start_local(game, anim, 0.0)
}

// ---- receiving ------------------------------------------------------------------------------

/// `BaseClientNetwork.receiveMessage` / `executeMessage`.
fn receive(game: &mut Game<'_>, bytes: &[u8]) {
    let data = match NetData::read(bytes, Bound::Client, true) {
        Ok(d) => d,
        Err(e) => {
            log::warn!("[{NAME}] Invalid message from the server: {e}");
            return;
        }
    };
    if data.player.is_none() && data.purpose.player_bound() {
        log::error!("[{NAME}] Didn't received any player information!");
        return;
    }
    match data.purpose {
        Task::Stream => {
            let Some(e) = data.emote else { return };
            let validate = config(|c| c.b("alwaysValidateEmote"));
            let _ = validate;
            let Some(player) = data.player else { return };
            // `isEmoteAllowed`: a hidden (muted) player's emotes don't show.
            if config(|c| c.b("playersafety")) && game.social.hidden.contains(&player) && player != game.own_uuid() {
                return;
            }
            if entity_of(game, player).is_some() || player == game.own_uuid() {
                if player == game.own_uuid() {
                    game.stop_emote();
                }
                play_on(game, player, e, data.tick, data.forced);
            } else {
                let tick = client(|c| c.tick);
                client(|c| {
                    c.queue.insert(player, (e, data.tick, tick));
                });
            }
        }
        Task::Stop => {
            let (Some(player), Some(stop)) = (data.player, data.stop_emote) else { return };
            let own = game.own_uuid();
            let was = client(|c| {
                let playing = c.avatars.get(&player).and_then(|a| a.ctrl.current().map(|x| x.uuid)) == Some(stop);
                if playing && let Some(a) = c.avatars.get_mut(&player) {
                    a.ctrl.stop();
                }
                if !c.avatars.contains_key(&player) {
                    c.queue.remove(&player);
                }
                playing
            });
            if was && player == own {
                internal_stop(game);
                if !data.forced {
                    ui::toast(Text::literal(NAME), Text::literal(t("emotecraft.blockedEmote")));
                }
            }
        }
        Task::Config => {
            let outdated_now = client(|c| {
                c.net.versions = data.versions.clone();
                c.net.configured = true;
                c.net.channel_open = true;
                outdated(&c.net.versions)
            });
            log::info!("[{NAME}] The configuration of the server is complete!");
            if outdated_now {
                ui::toast(Text::literal(NAME), Text::literal(t("emotecraft.different_server")).with_color(0xFF5555));
            }
        }
        Task::File => {
            if let Some(e) = data.emote {
                add_emote(e, true);
            }
        }
        Task::Remove => {
            let ids = data.remove_emotes;
            lock(&LIST).retain(|h| !(h.from_server && ids.contains(&h.anim.uuid)));
        }
        Task::Unknown => {}
    }
}

/// Hook `client_packet`: Emotecraft's messages, and the server registering its channel.
pub(super) fn packet(game: &mut Game<'_>, packet: &ClientboundPlayPacket) {
    let ClientboundPlayPacket::CustomPayload(CustomPayload { channel, data }) = packet else { return };
    if !game.native_on(ID) || hosting(game) {
        return;
    }
    match channel.as_str() {
        packet::CHANNEL => {
            client(|c| c.net.channel_open = true);
            receive(game, data);
        }
        packet::STREAM_CHANNEL => log::error!("[{NAME}] Streaming message received!"),
        "minecraft:register" => {
            if data.split(|b| *b == 0).any(|c| c == packet::CHANNEL.as_bytes()) {
                client(|c| c.net.channel_open = true);
            }
        }
        _ => {}
    }
}

/// Hook `client_disconnecting`: the server's emotes go (`EmoteHolder.clearEmotes(instance)`).
pub(super) fn disconnecting() {
    lock(&LIST).retain(|h| !h.from_server);
    client(|c| {
        c.net = Net::default();
        c.queue.clear();
        c.avatars.clear();
        c.forced = false;
        c.perspective = false;
    });
    lock(&BRIDGE).host = None;
}

/// The entity id of a player in the window's level.
fn entity_of(game: &Game<'_>, uuid: u128) -> Option<u32> {
    game.mobs.iter().find(|m| !m.removed && game.remote_player_uuid(m.id) == Some(uuid)).map(|m| m.id)
}

// ---- once a tick ----------------------------------------------------------------------------

/// Where a queued emote is now: the tick it came with plus the ticks since it came. The
/// queue's cleanup counts the same way (`checkQueue` adds the client's whole uptime instead,
/// which drops an emote that has just come once the client ran longer than it lasts).
fn queued_tick(begin: f32, received: u64, now: u64) -> f32 {
    begin + now.saturating_sub(received) as f32
}

/// Hook `tick` (the window's): every emote a tick on, the own player's stop rules
/// (`EmoteAvatarMixin.tick`, `PlayerMixin.updatePlayerPose`), the queue (`checkQueue`), the
/// screens' animations.
pub(super) fn tick(game: &mut Game<'_>) {
    if game.window.is_none() || !game.native_on(ID) {
        return;
    }
    let own = game.own_uuid();
    let present: Vec<u128> = game.mobs.iter().filter(|m| !m.removed).filter_map(|m| game.remote_player_uuid(m.id)).collect();
    // Avatars that just came: their queued emote starts on their second tick.
    let mut start: Vec<(u128, Arc<Animation>, f32)> = Vec::new();
    client(|c| {
        c.tick += 1;
        c.own = own;
        let now = c.tick;
        for uuid in &present {
            let a = c.avatars.entry(*uuid).or_insert_with(|| Avatar { ctrl: Controller::new(), age: 0 });
            if a.age <= 1 {
                if a.age == 1
                    && let Some((e, tick, received)) = c.queue.remove(uuid)
                {
                    let t = queued_tick(tick, received, now);
                    if e.is_playing_at(t) {
                        start.push((*uuid, e, t));
                    }
                }
                a.age += 1;
            }
        }
        // Gone players' emotes go with them.
        c.avatars.retain(|u, _| *u == own || present.contains(u));
        for a in c.avatars.values_mut() {
            a.ctrl.tick();
        }
        if now % 21 == 20 {
            c.queue.retain(|_, (e, tick, received)| {
                let t = queued_tick(*tick, *received, now);
                !((!e.is_playing_at(t) && t > 0.0) || now - *received > 24000)
            });
        }
    });
    for (u, e, t) in start {
        play_on(game, u, e, t, false);
    }
    // The own player.
    let playing = client(|c| c.avatars.get(&own).is_some_and(|a| a.ctrl.is_active()));
    if playing && game.player.is_some() {
        let to = camera_from(&config(|c| c.s("cameraType")));
        client(|c| {
            if c.perspective && game.camera_type != to {
                c.perspective = false;
            }
        });
        let forced = client(|c| c.forced);
        if !can_run(game) {
            stop_local(game);
        } else if !forced && config(|c| c.b("checkPose")) && invalid_pose(game) {
            stop_local(game);
        }
    }
    ui::tick(game);
}

/// `PlayerMixin.updatePlayerPose`: crouching, dying, swimming, gliding or sleeping.
fn invalid_pose(game: &Game<'_>) -> bool {
    use horizon_client::pose::Pose;
    let Some(p) = game.player.as_ref() else { return false };
    matches!(p.pose, Pose::Crouching | Pose::Swimming | Pose::FallFlying | Pose::Sleeping) || p.health.dead_or_dying()
}

// ---- keys -----------------------------------------------------------------------------------

/// Hook `key`: the wheel key, the stop key and the emotes' own keys while playing
/// (`EmotecraftClientMod.onClientTick`, `EmoteHolder.handleKeyPress`).
pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if game.window.is_none() || !game.native_on(ID) {
        return;
    }
    ui::key_anywhere(game, key, pressed);
    if !pressed || game.screen != super::Screen::Playing || game.player.is_none() {
        return;
    }
    if key == game.native_mod_key(ID, "fastchoose", "key.keyboard.b") {
        // `alwaysOpenEmoteScreen || player == cameraEntity`. Opened next frame, so this key
        // press does not reach the wheel it opens (which closes on its own key).
        OPEN_WHEEL.store(true, Ordering::Relaxed);
        return;
    }
    if key == game.native_mod_key(ID, "stop", UNBOUND) {
        stop_local(game);
        return;
    }
    if !can_run(game) {
        return;
    }
    let bound = config(|c| c.keys.get(key).cloned());
    if let Some(a) = bound {
        start_local(game, a, 0.0);
    }
}

// ---- each frame -----------------------------------------------------------------------------

static STARTED: AtomicBool = AtomicBool::new(false);
/// The wheel key was pressed: the wheel opens next frame.
static OPEN_WHEEL: AtomicBool = AtomicBool::new(false);

/// Hook `frame`, the window's: the emotes loaded, the handshake, the messages, each avatar's
/// pose this frame, the songs, the config saved.
pub(super) fn frame(game: &mut Game<'_>) {
    if game.window.is_none() {
        return;
    }
    if !STARTED.swap(true, Ordering::Relaxed) {
        let dir = game.native_instance_dir();
        client(|c| c.emotes_dir = dir.join(c.config.s("emotesDirectory")));
        write_data(game);
        reload_emotes(false);
    }
    if let Some(loaded) = lock(&LOADED).take() {
        let mut l = lock(&LIST);
        l.retain(|h| h.from_server);
        let server: Vec<Holder> = std::mem::take(&mut *l);
        drop(l);
        for a in loaded.all() {
            add_emote(a, false);
        }
        for h in server {
            add_emote(h.anim, true);
        }
        let migrated = config(|c| c.migrate(find));
        if migrated {
            save_config();
        }
        ui::emotes_reloaded();
        if let Some(at) = lock(&RELOAD_STARTED).take() {
            ui::toast(Text::literal(NAME), Text::literal(lang::tf("emotecraft.reloading.done", &[&format!("{:.3}", at.elapsed().as_secs_f64())])));
        }
    }
    if !game.native_on(ID) {
        pose::set_frames(HashMap::new());
        return;
    }
    for a in std::mem::take(&mut *lock(&ACTIONS)) {
        match a {
            Action::Play(anim) => {
                play_emote(game, anim);
            }
        }
    }
    if OPEN_WHEEL.swap(false, Ordering::Relaxed) && game.screen == super::Screen::Playing && game.player.is_some() {
        ui::open_wheel(game);
    }
    ui::frame(game);
    network(game);
    pose_frames(game);
    songs(game);
    let save = client(|c| c.config.dirty && c.saved_at.elapsed().as_secs() >= 2);
    if save {
        save_config();
    }
}

/// The handshake (`configureOnPlay`: `minecraft:register`, then the client's CONFIG) and, for
/// the host, the messages through `BRIDGE`.
fn network(game: &mut Game<'_>) {
    let in_world = game.player.is_some() && game.native_client_state().is_some_and(|s| s.logged_in);
    let host = hosting(game);
    let was = client(|c| std::mem::replace(&mut c.in_world, in_world));
    if !in_world {
        if was {
            disconnecting();
        }
        lock(&BRIDGE).host = None;
        return;
    }
    client(|c| {
        c.net.hosting = host;
        c.net.joined.get_or_insert_with(Instant::now);
    });
    if host {
        let (uuid, name) = (game.own_uuid(), game.own_profile_name());
        let incoming = {
            let mut b = lock(&BRIDGE);
            b.host = Some((uuid, name));
            std::mem::take(&mut b.to_client)
        };
        let send_config = client(|c| !std::mem::replace(&mut c.net.config_sent, true));
        if send_config {
            let mut d = NetData::config(false);
            if let Ok(bytes) = d.write(Bound::Server) {
                lock(&BRIDGE).to_server.push(bytes);
            }
        }
        for data in incoming {
            receive(game, &data);
        }
        return;
    }
    let (register, config_now) = client(|c| {
        let reg = !std::mem::replace(&mut c.net.register_sent, true);
        // Configure once the server has the channel, or after a moment in case its
        // registration came in the configuration phase, which mods don't see.
        let late = c.net.joined.is_some_and(|j| j.elapsed().as_secs() >= 3);
        let cfg = !c.net.config_sent && (c.net.channel_open || late);
        if cfg {
            c.net.config_sent = true;
        }
        (reg, cfg)
    });
    if register {
        send_raw(game, "minecraft:register", format!("{}\0{}", packet::CHANNEL, packet::STREAM_CHANNEL).into_bytes());
    }
    if config_now {
        log::warn!("[{NAME}] The server failed to configure the client, attempting to configure...");
        let mut d = NetData::config(false);
        if let Ok(bytes) = d.write(Bound::Server) {
            send_raw(game, packet::CHANNEL, bytes);
        }
    }
}

/// Every avatar's pose at this frame's partial tick (`process` / `setupAnim`), handed to the
/// model's hooks by entity id; an emote that ended puts the camera back.
fn pose_frames(game: &mut Game<'_>) {
    let partial = game.native_client_state().map_or(1.0, |s| s.world_partial_tick) as f32;
    let own = game.own_uuid();
    let ids: HashMap<u128, u32> = game.mobs.iter().filter(|m| !m.removed).filter_map(|m| game.remote_player_uuid(m.id).map(|u| (u, m.id))).collect();
    let mut frames: Frames = HashMap::new();
    let mut own_ended = false;
    client(|c| {
        for (uuid, a) in c.avatars.iter_mut() {
            if !a.ctrl.is_active() {
                continue;
            }
            let running = a.ctrl.process(partial);
            if !running {
                if *uuid == own {
                    own_ended = true;
                }
                continue;
            }
            let Some(f) = a.ctrl.frame() else { continue };
            let id = if *uuid == own { Some(pose::OWN_MODEL) } else { ids.get(uuid).copied() };
            if let Some(id) = id {
                frames.insert(id, Arc::new(FrameAdapter(f)));
            }
        }
        if own_ended {
            c.forced = false;
        }
    });
    if own_ended {
        internal_stop(game);
    }
    ui::preview_frames(&mut frames);
    pose::set_frames(frames);
}

/// The songs where their avatars are (`EmotePlayer.startSound`, `EmoteSoundInstance`).
fn songs(game: &mut Game<'_>) {
    let own = game.own_uuid();
    let volume = game.settings.master_volume * game.settings.volumes.get(sound_source("players")).copied().unwrap_or(1.0);
    let listener_pos = game.native_camera().map_or(game.view.position, |c| c.position);
    let positions: HashMap<u128, [f64; 3]> = game
        .mobs
        .iter()
        .filter(|m| !m.removed && !m.invisible)
        .filter_map(|m| game.remote_player_uuid(m.id).map(|u| (u, [m.position.x, m.position.y, m.position.z])))
        .collect();
    let own_pos = game.player.as_ref().map(|p| [p.position.x, p.position.y, p.position.z]);
    let listener = emotecraft_engine::audio::Listener {
        position: listener_pos,
        yaw: game.view.y_rot,
        volume,
        normalize: config(|c| c.b("normalizeSoundVolume")),
        other_volume: config(|c| c.f("otherEmoteVolume")),
        device: game.settings.sound_device.clone(),
    };
    // The sounds pause with the game (`SoundManager.pauseAllExcept`).
    let paused = game.pause;
    client(|c| {
        let mut wanted = Vec::new();
        if volume > 0.0 && !paused {
            for (uuid, a) in &c.avatars {
                if !a.ctrl.is_active() || a.ctrl.is_paused() {
                    continue;
                }
                let Some(e) = a.ctrl.current() else { continue };
                let Some(sound) = e.sound.clone() else { continue };
                let pos = if *uuid == own { own_pos } else { positions.get(uuid).copied() };
                let Some(position) = pos else { continue };
                wanted.push(emotecraft_engine::audio::Wanted {
                    id: *uuid,
                    sound,
                    offset: (a.ctrl.ticks() / 20.0 * emotecraft_engine::opus::SAMPLE_RATE as f32) as i32,
                    position,
                    own: *uuid == own,
                });
            }
        }
        c.songs.update(&listener, wanted);
    });
}

fn sound_source(name: &str) -> usize {
    horizon_client::settings::SOUND_SOURCES.iter().position(|n| *n == name).unwrap_or(0)
}

// ---- the commands ---------------------------------------------------------------------------

/// Hook `commands`: `/emotes-client` (`ClientCommands`).
pub(super) fn commands(game: &mut Game<'_>, commands: &mut Vec<&'static str>) {
    if game.window.is_some() && game.native_on(ID) {
        commands.push("emotes-client");
    }
}

/// Splits a command's arguments, a quoted one kept whole (`StringArgumentType.string`).
pub(super) fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut any = false;
    for ch in s.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            ' ' if !quoted => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                any = false;
            }
            c => {
                cur.push(c);
                any = true;
            }
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// An emote's name as plain text, for the commands (`StringUtil.filterText(name.getString())`).
pub(super) fn plain_name(a: &Animation) -> Option<String> {
    let raw = a.str("name")?;
    Some(emotecraft_engine::text::filter(&emotecraft_engine::text::plain(raw, &|k| lang::text(k))))
}

/// Hook `command`: `/emotes-client play <emote>` and `/emotes-client stop`.
pub(super) fn command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    if game.window.is_none() || !game.native_on(ID) {
        return;
    }
    let line = command.trim_start_matches('/');
    let Some(rest) = line.strip_prefix("emotes-client") else { return };
    if !(rest.is_empty() || rest.starts_with(' ')) {
        return;
    }
    *ran = true;
    let args = split_args(rest.trim());
    let forced_msg = || Text::literal(t("emotecraft.cant.override.forced")).with_color(0xFF5555);
    match args.first().map(String::as_str) {
        Some("play") => {
            let Some(id) = args.get(1) else { return };
            let found = list().into_iter().find(|h| emotecraft_engine::buf::parse_uuid(id) == Some(h.anim.uuid) || plain_name(&h.anim).as_deref() == Some(id.as_str()));
            match found {
                Some(h) => {
                    if !start_local(game, h.anim, 0.0) {
                        game.native_client_action_bar(forced_msg(), false);
                    }
                }
                None => game.native_client_action_bar(Text::literal(format!("Not emote with name: {id}")).with_color(0xFF5555), false),
            }
        }
        Some("stop") => {
            if client(|c| c.forced) {
                game.native_client_action_bar(forced_msg(), false);
            } else {
                stop_local(game);
            }
        }
        _ => {}
    }
}

// ---- the elytra -----------------------------------------------------------------------------

/// `ElytraLayerMixin`: while an avatar emotes, its wings follow the body, the elytra and the cape
/// bones. True when drawn here (the game's own draw is skipped).
#[allow(clippy::too_many_arguments)]
pub(super) fn elytra(game: &Game<'_>, m: &horizon_client::mobs::Mob, chest: Option<&str>, camera: glam::DVec3, partial: f32, light: u32, out: &mut Vec<horizon_client::mesher::Vertex>) -> bool {
    let Some(frame) = pose::frame_of(m) else { return false };
    let main = if m.slim { "player_slim#main" } else { "main" };
    let Some(model) = m.kind.layer(main) else { return false };
    let data = game.res.mob_data();
    let mut poses = horizon_client::mobs::anim::Poses::new(model);
    horizon_client::mobs::anim::setup(m, &horizon_client::mobs::anim::state(m, partial), &mut poses, &data.animations);
    let base = horizon_client::mobs::render::base_transform(m, partial, camera);
    let Some(body) = horizon_client::mobs::render::part_matrix(model, base, &poses, "body", 0.0) else { return false };
    let mut bone = frame.bone("elytra");
    let cape = frame.bone("cape");
    for i in 0..3 {
        bone.pos[i] += cape.pos[i];
        bone.rot[i] += cape.rot[i];
        bone.scale[i] *= cape.scale[i];
    }
    bone.pos[1] *= -1.0;
    let z = glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.0, 0.125));
    let b = glam::Mat4::from_translation(glam::Vec3::new(bone.pos[0], bone.pos[1], bone.pos[2]) / 16.0)
        * glam::Mat4::from_rotation_z(bone.rot[2])
        * glam::Mat4::from_rotation_y(bone.rot[1])
        * glam::Mat4::from_rotation_x(bone.rot[0])
        * glam::Mat4::from_scale(glam::Vec3::new(bone.scale[0], bone.scale[1], bone.scale[2]));
    // The game moves the wings by 0.125 itself: the bone goes in after that move.
    let moved = base * z * base.inverse() * body * z * b * z.inverse() * z.inverse();
    let stack = m.extra.armor[1].as_ref().filter(|s| chest == Some(&*s.item));
    game.wings_geometry(chest, stack, m.cape.as_deref(), false, moved, m.extra.elytra.angles(partial), m.extra.crouching, light, game.wings_overlay(stack), out);
    true
}

// ---- Options > Mods and Mod Menu ------------------------------------------------------------

/// Hook `mods`: the mod's row in Options > Mods, with its keys.
pub(super) fn mods_list(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: ID.into(),
        title: format!("{NAME} {VERSION}"),
        description: lang::get_en("modmenu.descriptionTranslation.emotecraft").lines().next().unwrap_or_default().to_owned(),
        keys: KEYS.iter().map(|(name, label, default)| [(*name).to_owned(), t(label), (*default).to_owned()]).collect(),
    });
}

/// Hook `settings`: the mod's page, Configure... first.
pub(super) fn page(mod_id: &str, labels: &mut Vec<String>) {
    if mod_id == ID {
        labels.push(if lang::russian() { "Настройки..." } else { "Configure..." }.into());
    }
}

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after it opened the page.
pub(super) const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

/// Hook `settings_click`: Configure... or Mod Menu's Configure opens the Emotes config screen
/// (`EmoteMenu`, what Emotecraft's Mod Menu entry opens).
pub(super) fn page_click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id == ID && (button == 0 || button == MODMENU_CONFIGURE) {
        ui::open_over_options(game, button != 0);
    }
}

fn modmenu_json() -> String {
    let s = |v: &str| serde_json::to_string(v).unwrap_or_default();
    format!(
        r#"{{
  "schema": 1,
  "name": "{NAME}",
  "version": "{VERSION}",
  "description": {},
  "description_ru": {},
  "authors": ["KosmX", "dima_dencep"],
  "contributors": ["Kale Ko", "ZigyTheBird", "BoBkiNN", "DeportedToHell (Horizon port)"],
  "license": "GPL-3.0",
  "links": {{
    "homepage": "https://modrinth.com/mod/emotecraft",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/emotecraft",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues",
    "original": "https://github.com/KosmX/emotes",
    "discord": "https://discord.gg/6NfdRuE"
  }},
  "badges": ["client"],
  "config_screen": true
}}
"#,
        s(lang::get_en("modmenu.descriptionTranslation.emotecraft")),
        s(emotecraft_engine::lang::get("ru_ru", "modmenu.descriptionTranslation.emotecraft").unwrap_or_default()),
    )
}

/// `icon.png` and `modmenu.json` in the mod's data folder (each written only when it differs).
fn write_data(game: &Game<'_>) {
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

// ---- Horizon's own emote wheel --------------------------------------------------------------

/// HEAD of Horizon's `open_emote_wheel`: while Emotecraft is on, its wheel key (B) is
/// Emotecraft's, as in Java, and Horizon's own wheel stays shut.
pub(super) fn horizon_wheel_blocked(game: &Game<'_>) -> bool {
    game.native_on(ID)
}

