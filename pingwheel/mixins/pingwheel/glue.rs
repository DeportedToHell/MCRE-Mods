// Ping Wheel in the game: `crate::game::pingwheel_glue`.
//
// The client half (the window's `Game`): the config (`config/pingwheel.json`), the Ping Location
// and Open Settings keys, `/pingwheel`, the pings the server relays (`PingManager`, `PingView`),
// the ping action (`PingController`), the sound, the HUD and the settings screen. The server half
// (single player's server thread, a LAN host, a dedicated server): `ServerCore` with
// `config/pingwheel.server.json` and `/pingwheel:server`. Both speak the Java mod's plugin
// channels on its byte layout (pingwheel_core::protocol), so either half works with the other's
// Java version.
//
// Ping Wheel by Luken (https://github.com/LukenSkyne/Minecraft-Ping-Wheel), MIT.

use super::Game;
use super::native_hooks::NativeMod;
use super::pingwheel_hud::{self as hud, Author, Marker, Sprites};
use super::pingwheel_lang::{self as lang, t};
use super::pingwheel_screen as screen;
use horizon_client::hud::{HudBatch, Rect};
use horizon_client::items::ItemStack;
use horizon_client::mod_api::{ClientboundPlayPacket, NativeScreenView, NativeServerPlayer, NativeWorldId, ServerTickPhase, ServerboundPlayPacket, WorldPhase};
use horizon_client::text::Text;
use horizon_protocol::packets::configuration::CustomPayload;
use pingwheel_core::config::{ChannelMode, ClientConfig, MAX_CORRECTION_PERIOD, MAX_PING_DISTANCE, MAX_PING_DURATION, ServerConfig, TPS};
use pingwheel_core::protocol::{self, PING_LOCATION_C2S, PING_LOCATION_S2C, PingC2S, PingS2C, UPDATE_CHANNEL_C2S, java_hash};
use pingwheel_core::server::{Out, Player, ServerCore};
use pingwheel_core::{MAX_CHANNEL_LENGTH, math};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

pub(super) const ID: &str = "pingwheel";
pub(super) const NAME: &str = "Ping Wheel";
pub(super) const VERSION: &str = "1.0.0";
/// `gradle.properties`' description.
const DESCRIPTION: &str = "Mark locations similar to the ping mechanism found in modern co-op multiplayer games.";
const DESCRIPTION_RU: &str = "Отмечайте места метками, как пингами в современных кооперативных играх.";
const UNBOUND: &str = "key.keyboard.unknown";
/// The log target: Horizon's logger shows the informational lines of `horizon*` targets.
const LOG: &str = "horizon_pingwheel";
/// `Global.LOGGER`'s prefix.
const PREFIX: &str = "[Ping-Wheel] ";

/// `InputUtils`: the keys, their names and defaults (Mouse 5, "Forward"; Open Settings unbound).
const KEYS: [(&str, &str, &str); 2] = [("ping_location", "key.pingwheel.ping_location", "key.mouse.5"), ("open_settings", "key.pingwheel.open_settings", UNBOUND)];

const OGG: &[u8] = horizon_client::native_data!("pingwheel", "ping.ogg");
const ARROW: &[u8] = horizon_client::native_data!("pingwheel", "arrow32.rgba");
const ICON: &[u8] = horizon_client::native_data!("pingwheel", "icon.png");

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---- config files -------------------------------------------------------------------------------

/// `LegacyMigrationHandler.migrateConfig`: `ping-wheel<ext>` renamed to `pingwheel<ext>`.
fn config_path(ext: &str) -> Option<PathBuf> {
    let dir = Game::native_config_dir().ok()?;
    let path = dir.join(format!("{ID}{ext}"));
    let legacy = dir.join(format!("ping-wheel{ext}"));
    if legacy.exists() {
        match std::fs::rename(&legacy, &path) {
            Ok(()) => log::info!(target: LOG, "{PREFIX}Renamed legacy config: {} -> {}", legacy.display(), path.display()),
            Err(e) => log::error!(target: LOG, "{PREFIX}Failed to rename legacy config: {e}"),
        }
    }
    Some(path)
}

fn write_file(path: &Path, text: &str) -> bool {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(path, text) {
        Ok(()) => true,
        Err(e) => {
            log::error!(target: LOG, "{PREFIX}Saving Config failed: {e}");
            false
        }
    }
}

// ---- the client's config ------------------------------------------------------------------------

struct Stored {
    config: ClientConfig,
    /// What the file holds (`configHash`): saving writes only a change.
    saved: Option<ClientConfig>,
    path: Option<PathBuf>,
}

static CONFIG: Mutex<Option<Stored>> = Mutex::new(None);
/// The server joined, by its address (`ServerData.ip`); None in single player and as a LAN host.
static SERVER: Mutex<Option<String>> = Mutex::new(None);

/// `ConfigHandler.load`: read, or the defaults written when the file is missing or broken.
fn stored() -> MutexGuard<'static, Option<Stored>> {
    let mut g = lock(&CONFIG);
    if g.is_none() {
        let path = config_path(".json");
        let text = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok());
        let mut s = Stored { config: ClientConfig::default(), saved: None, path };
        match text.map(|t| ClientConfig::parse(&t)) {
            Some(Some(mut c)) => {
                c.validate();
                log::info!(target: LOG, "{PREFIX}Loaded {c}");
                s.saved = Some(c.clone());
                s.config = c;
            }
            Some(None) => log::error!(target: LOG, "{PREFIX}Config is broken -> reset to defaults"),
            None => {}
        }
        *g = Some(s);
    }
    g
}

/// The config, to read or change.
pub(super) fn config<R>(f: impl FnOnce(&mut ClientConfig) -> R) -> R {
    let mut g = stored();
    f(&mut g.as_mut().expect("loaded").config)
}

fn server_key() -> Option<String> {
    lock(&SERVER).clone()
}

/// `ClientConfig.getChannel`.
pub(super) fn channel() -> String {
    let server = server_key();
    config(|c| c.channel_for(server.as_deref()))
}

/// `ClientConfig.setChannel`.
pub(super) fn set_channel(c: &mut ClientConfig, channel: &str) {
    c.set_channel_for(server_key().as_deref(), channel);
}

/// `ConfigHandler.save`: when it changed, `onUpdate` (the channel to the server) and the file.
pub(super) fn save(game: &mut Game<'_>) {
    let (changed, text, path, line) = {
        let mut g = stored();
        let s = g.as_mut().expect("loaded");
        if s.saved.as_ref() == Some(&s.config) {
            return;
        }
        (s.config.clone(), s.config.write(), s.path.clone(), s.config.to_string())
    };
    send_channel(game);
    if let Some(path) = path
        && !write_file(&path, &text)
    {
        return;
    }
    if let Some(s) = stored().as_mut() {
        s.saved = Some(changed);
    }
    log::info!(target: LOG, "{PREFIX}Saved {line}");
}

// ---- the client ---------------------------------------------------------------------------------

/// `PingView`.
struct Ping {
    pos: [f64; 3],
    entity: Option<u128>,
    author: Option<u128>,
    sequence: i32,
    dimension: i32,
    spawn: Option<i64>,
    age: i64,
    item: Option<ItemStack>,
    /// From the last `update`: on the GUI (x, y, depth) and from the camera.
    screen: [f32; 3],
    distance: f64,
}

impl Ping {
    fn from(p: PingS2C) -> Self {
        Self { pos: p.pos, entity: p.entity, author: Some(p.author), sequence: p.sequence, dimension: p.dimension, spawn: None, age: 0, item: None, screen: [0.0, 0.0, -1.0], distance: 0.0 }
    }
    /// `isExpired`.
    fn expired(&self, c: &ClientConfig) -> bool {
        self.age > i64::from(c.ping_duration * TPS) && c.ping_duration < MAX_PING_DURATION
    }
    /// `distanceToCenter`: behind the camera, as far as can be.
    fn to_centre(&self, w: f32, h: f32) -> f32 {
        if self.screen[2] <= 0.0 {
            return f32::MAX;
        }
        ((w * 0.5 - self.screen[0]).powi(2) + (h * 0.5 - self.screen[1]).powi(2)).sqrt()
    }
    /// `isRemovable`.
    fn removable(&self, c: &ClientConfig, w: f32, h: f32) -> bool {
        (c.correction_period >= MAX_CORRECTION_PERIOD || self.age as f32 > c.correction_period * TPS as f32) && self.to_centre(w, h) < c.remove_radius as f32
    }
}

#[derive(Default)]
struct Client {
    started: bool,
    /// `PingManager.PING_REPO`.
    pings: Vec<Ping>,
    /// `PingController`.
    queued: bool,
    sequence: i32,
    last_ping: i32,
    /// `onJoinServer` ran for this connection.
    joined: bool,
    open_settings: bool,
    /// `GameContext.dimension`: the level id's `hashCode`.
    dimension: i32,
    /// The player list (`PlayerInfo`): name and skin by UUID.
    profiles: HashMap<u128, (String, String)>,
    sprites: Sprites,
    upload: bool,
}

thread_local! {
    static CLIENT: RefCell<Client> = RefCell::new(Client::default());
}

fn client<R>(f: impl FnOnce(&mut Client) -> R) -> R {
    CLIENT.with(|c| f(&mut c.borrow_mut()))
}

/// The GUI's size, as the HUD is drawn.
fn gui_size(game: &Game<'_>) -> (f32, f32) {
    let size = game.renderer.as_ref().map_or([1280, 720], |r| [r.config.width, r.config.height]);
    let b = HudBatch::new(size);
    (b.width, b.height)
}

fn send(game: &mut Game<'_>, channel: &str, data: Vec<u8>) -> bool {
    game.native_send_packet(ServerboundPlayPacket::CustomPayload(CustomPayload { channel: channel.into(), data })).is_ok()
}

/// `UpdateChannelC2SPacket` with this server's channel.
fn send_channel(game: &mut Game<'_>) -> bool {
    match protocol::encode_update_channel(&channel()) {
        Ok(data) => send(game, UPDATE_CHANNEL_C2S, data),
        Err(_) => false,
    }
}

/// `onJoinServer`: the channel, after `minecraft:register` of the channel the server sends on
/// (Fabric's and Bukkit's servers send only on channels the client registered).
fn join_server(game: &mut Game<'_>) -> bool {
    if !send(game, "minecraft:register", protocol::register_payload(&[PING_LOCATION_S2C])) {
        return false;
    }
    send_channel(game)
}

/// `onLeaveServer`.
fn leave() {
    client(|c| {
        c.pings.clear();
        c.joined = false;
        c.queued = false;
    });
    *lock(&SERVER) = None;
}

/// Hook `frame`, the window's.
pub(super) fn frame(game: &mut Game<'_>) {
    if game.window.is_none() || !game.native_on(ID) {
        return;
    }
    if !client(|c| std::mem::replace(&mut c.started, true)) {
        start(game);
    }
    lang::inject();
    if client(|c| std::mem::take(&mut c.upload)) {
        upload_sprites(game);
    }
    screen::frame(game);
    let state = game.native_client_state();
    let in_world = game.native_player().is_some() && state.as_ref().is_some_and(|s| s.logged_in);
    if !in_world {
        if client(|c| c.joined || !c.pings.is_empty()) {
            leave();
        }
        return;
    }
    *lock(&SERVER) = match state.and_then(|s| s.world) {
        Some(NativeWorldId::Server(address)) => Some(address),
        _ => None,
    };
    if !client(|c| c.joined) && join_server(game) {
        client(|c| c.joined = true);
    }
    let dimension = java_hash(game.dim.id());
    let profiles: HashMap<u128, (String, String)> = game.native_client_profiles().into_iter().map(|p| (p.uuid, (p.name, p.skin))).collect();
    let open = client(|c| {
        c.dimension = dimension;
        c.profiles = profiles;
        std::mem::take(&mut c.open_settings)
    });
    if open && !screen::is_open() {
        screen::open_in_world(game);
    }
    update_pings(game);
    poll_ping(game);
}

/// The first frame: the config read (and written when new), the server's config file as Ping
/// Wheel's init makes it, Mod Menu's files, the textures.
fn start(game: &mut Game<'_>) {
    drop(stored());
    let fresh = lock(&CONFIG).as_ref().is_some_and(|s| s.saved.is_none());
    if fresh {
        save(game);
    }
    drop(server_side());
    write_data(game);
    upload_sprites(game);
}

/// `PingManager.updatePings`: ages, entities followed, the expired gone; with the ping key pressed
/// near a ping (within `removeRadius` of the crosshair, past the correction period) that ping goes
/// instead of a new one being sent.
fn update_pings(game: &mut Game<'_>) {
    if client(|c| c.pings.is_empty()) {
        return;
    }
    let time = game.world.game_time;
    let (w, h) = gui_size(game);
    let Some(cam) = hud::camera(game, w / h.max(1.0)) else { return };
    let partial = f64::from(cam.partial_tick);
    let cfg = config(|c| c.clone());
    let follow: Vec<Option<u128>> = client(|c| c.pings.iter().map(|p| p.entity).collect());
    let followed: Vec<Option<([f64; 3], Option<ItemStack>)>> = follow.iter().map(|e| e.and_then(|e| game.pingwheel_entity(e, partial))).collect();
    client(|c| {
        let queued = c.queued;
        let mut target: Option<usize> = None;
        let mut index = 0;
        c.pings.retain_mut(|p| {
            let spawn = *p.spawn.get_or_insert(time);
            p.age = time - spawn;
            if let Some(Some((pos, stack))) = followed.get(index) {
                if stack.is_some() && cfg.item_icon_visible {
                    p.item.clone_from(stack);
                }
                p.pos = *pos;
            }
            index += 1;
            p.screen = hud::project(&cam, p.pos, w, h);
            p.distance = dist(cam.position, p.pos);
            if p.expired(&cfg) {
                return false;
            }
            true
        });
        if queued {
            for (k, p) in c.pings.iter().enumerate() {
                if p.removable(&cfg, w, h) && target.is_none_or(|t| p.to_centre(w, h) < c.pings[t].to_centre(w, h)) {
                    target = Some(k);
                }
            }
        }
        if let Some(t) = target {
            c.pings.remove(t);
            c.queued = false;
        }
        c.pings.sort_by(|a, b| b.distance.total_cmp(&a.distance));
    });
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// `PingController.pollPingAction` and `performPingAction`: the ping key's ping, sent to the
/// server where the crosshair's ray hits; a new sequence once the last ping is older than the
/// correction period (a ping within it replaces the last one).
fn poll_ping(game: &mut Game<'_>) {
    if !client(|c| c.queued) {
        return;
    }
    let time = game.world.game_time as i32;
    let cfg = config(|c| c.clone());
    let (sequence, dimension) = client(|c| {
        if cfg.correction_period < MAX_CORRECTION_PERIOD && (time - c.last_ping) as f32 > cfg.correction_period * TPS as f32 {
            c.sequence += 1;
        }
        c.last_ping = time;
        c.queued = false;
        (c.sequence, c.dimension)
    });
    let distance = f64::from(cfg.raycast_distance.min(cfg.ping_distance));
    let Some(hit) = game.pingwheel_trace(distance) else { return };
    let packet = PingC2S { channel: channel(), pos: hit.location, entity: hit.entity, sequence, dimension };
    if let Ok(data) = packet.encode() {
        send(game, PING_LOCATION_C2S, data);
    }
}

/// Hook `key`: the ping key (`InputUtils.consumePingHotkey`) and the settings key, while playing.
pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed || game.window.is_none() || !game.native_on(ID) || !game.native_playing() {
        return;
    }
    let ping = game.native_mod_key(ID, KEYS[0].0, KEYS[0].2);
    if key == ping && ping != UNBOUND {
        // Bound like Pick Block: a ping only where Pick Block has nothing (the crosshair on no
        // block, or on an entity out of creative).
        let queue = if ping == game.settings.keys.get("key.pickItem") {
            let block = game.aim().and_then(|(eye, dir)| game.clip_ray(eye, dir, game.reach())).is_some();
            let entity = game.entity_targeted();
            entity && !game.creative || (!block && !entity)
        } else {
            true
        };
        if queue {
            client(|c| c.queued = true);
        }
    }
    let settings = game.native_mod_key(ID, KEYS[1].0, KEYS[1].2);
    if key == settings && settings != UNBOUND {
        client(|c| c.open_settings = true);
    }
}

/// Hook `client_packet`: `ping-wheel-s2c:ping-location` (`PingManager.acceptPingPacket`).
pub(super) fn packet(game: &mut Game<'_>, packet: &ClientboundPlayPacket) {
    let ClientboundPlayPacket::CustomPayload(CustomPayload { channel: name, data }) = packet else { return };
    if name != PING_LOCATION_S2C || game.window.is_none() || !game.native_on(ID) {
        return;
    }
    let Some(p) = PingS2C::decode(data) else {
        log::warn!(target: LOG, "{PREFIX}received invalid ping location from server");
        return;
    };
    let Some(me) = game.native_player() else { return };
    let cfg = config(|c| c.clone());
    if p.channel != channel() {
        return;
    }
    let feet = [me.x, me.y, me.z];
    if cfg.ping_distance < MAX_PING_DISTANCE && dist(feet, p.pos) > f64::from(cfg.ping_distance) {
        return;
    }
    let ping = Ping::from(p);
    let (pos, dimension) = (ping.pos, ping.dimension);
    let current = client(|c| {
        // `addOrReplacePing`: the same author's same sequence is a correction.
        match c.pings.iter().position(|q| q.author == ping.author && q.sequence == ping.sequence) {
            Some(i) => c.pings[i] = ping,
            None => c.pings.push(ping),
        }
        c.dimension
    });
    // `playSoundInDimension`.
    if dimension == current && dimension == java_hash(game.dim.id()) {
        play_sound(game, feet, pos, cfg.ping_volume as f32 / 100.0);
    }
}

/// `DirectionalSoundInstance`: the ping sound from the ping's direction, at most 14 blocks away
/// (64 blocks and more map to 14), on the master source.
fn play_sound(game: &mut Game<'_>, player: [f64; 3], pos: [f64; 3], volume: f32) {
    if !horizon_client::audio::pingwheel_sound::prepare(&mut game.audio, OGG) {
        return;
    }
    let between = [pos[0] - player[0], pos[1] - player[1], pos[2] - player[2]];
    let len = (between[0].powi(2) + between[1].powi(2) + between[2].powi(2)).sqrt();
    let mapped = len.min(64.0) / 64.0 * 14.0;
    let at = if len < 1e-4 { player } else { std::array::from_fn(|i| player[i] + between[i] / len * mapped) };
    game.audio.play_unshared(horizon_client::audio::pingwheel_sound::EVENT, Some(at), volume, 1.0);
}

/// Hook `client_disconnecting`.
pub(super) fn disconnecting() {
    leave();
}

/// Hook `hud`: the markers (`OverlayRenderer`), under F1 not at all.
pub(super) fn draw_hud(game: &mut Game<'_>, batch: &mut HudBatch) {
    if game.window.is_none() || !game.native_on(ID) || game.native_player().is_none() {
        return;
    }
    let (w, h) = (batch.width, batch.height);
    let Some(cam) = hud::camera(game, w / h.max(1.0)) else { return };
    let cfg = config(|c| c.clone());
    // `keyPlayerList.isDown()`, as the player list itself asks it.
    let list_held = game.keys.held(horizon_client::actions::Action::Playerlist) && game.screen == super::Screen::Playing;
    let markers: Vec<Marker> = client(|c| {
        c.pings
            .iter()
            .filter(|p| p.dimension == c.dimension)
            .map(|p| {
                let distance = dist(cam.position, p.pos);
                let author = p.author.and_then(|a| c.profiles.get(&a)).map(|(name, skin)| author(game, name, skin));
                Marker { screen: hud::project(&cam, p.pos, w, h), distance, scale: math::ping_scale(distance, cfg.ping_size), item: p.item.clone(), author }
            })
            .collect()
    });
    let sprites = client(|c| c.sprites);
    hud::draw(game, batch, &cfg, &markers, sprites, list_held);
}

/// `PlayerInfo` of the author: the team's formatting and colour.
fn author(game: &Game<'_>, name: &str, skin: &str) -> Author {
    let team = game.score.board.players_team(name);
    let color = team.and_then(|t| t.color.as_deref()).and_then(horizon_client::scoreboard::team_color_rgb).map_or(hud::WHITE, |rgb| 0xFF00_0000 | rgb);
    let formatted = team.map_or_else(|| Text::literal(name), |t| t.formatted_name(Text::literal(name)));
    Author { name: formatted, plain: Text::literal(name), skin: skin.to_owned(), team_color: color }
}

// ---- textures -----------------------------------------------------------------------------------

/// The rows of the mod-image slot Ping Wheel has (team/knowledge/shared-resources.md).
const ROW: u32 = 976;
const SIDE: u32 = 32;

/// `pixels` (`w` x `h`) boxed down or stretched to `SIDE` x `SIDE`.
fn fit(w: u32, h: u32, pixels: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; (SIDE * SIDE * 4) as usize];
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (x0, x1) = (x * w / SIDE, ((x + 1) * w / SIDE).max(x * w / SIDE + 1).min(w));
            let (y0, y1) = (y * h / SIDE, ((y + 1) * h / SIDE).max(y * h / SIDE + 1).min(h));
            let mut sum = [0u32; 4];
            let mut n = 0;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = ((sy * w + sx) * 4) as usize;
                    let a = u32::from(pixels[i + 3]);
                    for k in 0..3 {
                        sum[k] += u32::from(pixels[i + k]) * a;
                    }
                    sum[3] += a;
                    n += 1;
                }
            }
            let o = ((y * SIDE + x) * 4) as usize;
            if sum[3] > 0 {
                for k in 0..3 {
                    out[o + k] = (sum[k] / sum[3]) as u8;
                }
            }
            out[o + 3] = (sum[3] / n.max(1)) as u8;
        }
    }
    out
}

/// `arrow.png` (a resource pack's, else the original's) and a resource pack's `ping.png`
/// (`ResourceReloadListener.hasCustomTexture`) into the slot; again after each reload.
fn upload_sprites(game: &mut Game<'_>) {
    let Some(rect) = game.gui.atlas.sprites.get(horizon_client::hud::MOD_IMAGE_SPRITE).copied() else {
        client(|c| c.sprites = Sprites::default());
        return;
    };
    let resources = game.native_client_resources();
    let arrow = resources.texture("pingwheel:arrow").filter(|t| t.width > 0 && t.height > 0).map(|t| fit(t.width, t.height, &t.rgba));
    let arrow = arrow.or_else(|| (ARROW.len() == (SIDE * SIDE * 4) as usize).then(|| ARROW.to_vec()));
    let ping = resources.texture("pingwheel:ping").filter(|t| t.width > 0 && t.height > 0).map(|t| fit(t.width, t.height, &t.rgba));
    let (w, h) = (game.gui.atlas.width as f32, game.gui.atlas.height as f32);
    let (x0, y0) = ((rect.u0 * w).round() as u32, (rect.v0 * h).round() as u32);
    let size = horizon_client::hud::MOD_IMAGE_SIZE as f32;
    let sub = |col: u32| {
        let mut r = rect;
        let (du, dv) = (rect.u1 - rect.u0, rect.v1 - rect.v0);
        r.u0 = rect.u0 + col as f32 / size * du;
        r.u1 = rect.u0 + (col + SIDE) as f32 / size * du;
        r.v0 = rect.v0 + ROW as f32 / size * dv;
        r.v1 = rect.v0 + (ROW + SIDE) as f32 / size * dv;
        r
    };
    let mut sprites = Sprites::default();
    for (col, pixels, slot) in [(0, arrow, 0), (SIDE, ping, 1)] {
        let Some(rgba) = pixels else { continue };
        game.map_patches.push(horizon_client::font::Patch { x: x0 + col, y: y0 + ROW, width: SIDE, height: SIDE, rgba });
        let r: Rect = sub(col);
        if slot == 0 {
            sprites.arrow = Some(r);
        } else {
            sprites.ping = Some(r);
        }
    }
    client(|c| c.sprites = sprites);
}

/// Hook `client_resources_reloaded`: the textures into the new atlas.
pub(super) fn reloaded() {
    client(|c| c.upload = true);
}

// ---- chat and /pingwheel ------------------------------------------------------------------------

const YELLOW: u32 = 0xFF_FF55;
const GRAY: u32 = 0xAA_AAAA;
const DARK_GRAY: u32 = 0x55_5555;
const RED: u32 = 0xFF_5555;

/// A line in this client's chat.
fn chat(game: &mut Game<'_>, line: Text) {
    let focused = game.screen == super::Screen::Chat;
    let ticks = game.visual_ticks();
    let atlas = &game.gui.atlas;
    game.ui.chat.add(atlas, line, None, horizon_client::chat::ChatSource::Client, ticks, focused);
}

/// `LanguageUtils.withModPrefix`.
fn prefixed(response: Text) -> Text {
    Text::empty().append(Text::colored(PREFIX, DARK_GRAY)).append(response)
}

/// `LanguageUtils.wrapped`: "(text)" in grey.
fn wrapped(key: &str, tr: &dyn Fn(&str, Vec<Text>) -> Text) -> Text {
    Text::empty().append(Text::literal("(")).append(tr(key, Vec::new())).append(Text::literal(")")).with_color(GRAY)
}

/// `LanguageUtils.join`: the parts on lines of their own.
fn join(parts: Vec<Text>) -> Text {
    let mut out = Text::empty();
    for (i, p) in parts.into_iter().enumerate() {
        if i > 0 {
            out = out.append(Text::literal("\n"));
        }
        out = out.append(p);
    }
    out
}

/// Brigadier's quotable string (`StringArgumentType.string()`): a quoted string with `\`
/// escapes, or a word of `0-9 A-Z a-z _ - . +`. The value and the bytes read, or the error and
/// where it is.
fn read_string(s: &str) -> Result<(String, usize), (&'static str, usize)> {
    let mut chars = s.char_indices().peekable();
    match chars.peek() {
        Some(&(_, q)) if q == '"' || q == '\'' => {
            chars.next();
            let mut out = String::new();
            let mut escaped = false;
            for (i, c) in chars {
                if escaped {
                    if c == q || c == '\\' {
                        out.push(c);
                        escaped = false;
                    } else {
                        return Err(("parsing.quote.escape", i));
                    }
                } else if c == '\\' {
                    escaped = true;
                } else if c == q {
                    return Ok((out, i + 1));
                } else {
                    out.push(c);
                }
            }
            Err(("parsing.quote.expected.end", s.len()))
        }
        _ => {
            let end = s.find(|c: char| !(c.is_ascii_alphanumeric() || "_-.+".contains(c))).unwrap_or(s.len());
            Ok((s[..end].to_owned(), end))
        }
    }
}

/// `CommandSyntaxException` as Fabric shows it: "message at position N: …context<--[HERE]".
fn syntax_error(input: &str, cursor: usize, message: Text) -> Text {
    let cursor = cursor.min(input.len());
    let from = input[..cursor].char_indices().rev().nth(9).map_or(0, |(i, _)| i);
    let context = format!("{}{}", if from > 0 { "..." } else { "" }, &input[from..cursor]);
    let context = Text::literal(context).append(Text::translate("command.context.here", &[]));
    Text::translate("command.context.parse_error", &[message, Text::literal(cursor.to_string()), context])
}

/// What a command line holds after its literal: nothing, or the rest after one space.
enum Rest<'a> {
    End,
    Arg(&'a str, usize),
    Bad(usize),
}

fn rest(input: &str, at: usize) -> Rest<'_> {
    let tail = &input[at..];
    if tail.is_empty() {
        Rest::End
    } else if let Some(arg) = tail.strip_prefix(' ') {
        if arg.is_empty() { Rest::Bad(at + 1) } else { Rest::Arg(arg, at + 1) }
    } else {
        Rest::Bad(at)
    }
}

/// Hook `command`: `/pingwheel [help|config|channel [<channel_name>]]` (`ClientCommandBuilder`).
pub(super) fn command(game: &mut Game<'_>, input: &str, ran: &mut bool) {
    if game.window.is_none() || !game.native_on(ID) || input.split(' ').next() != Some("pingwheel") {
        return;
    }
    *ran = true;
    let tr = |key: &str, args: Vec<Text>| Text::translate(key, &args);
    let feedback = |game: &mut Game<'_>, ok: bool, response: Text| {
        let line = prefixed(response);
        chat(game, if ok { line } else { Text::empty().append(line).with_color(RED) });
    };
    let help = |game: &mut Game<'_>| {
        let text = join(vec![
            Text::empty(),
            Text::literal("/pingwheel config"),
            wrapped("command.pingwheel.config.description", &tr),
            Text::literal("/pingwheel channel"),
            wrapped("command.pingwheel.channel.get.description", &tr),
            Text::literal("/pingwheel channel <channel_name>"),
            wrapped("command.pingwheel.channel.set.description", &tr),
        ]);
        feedback(game, true, text);
    };
    let error = |game: &mut Game<'_>, at: usize, key: &str| chat(game, syntax_error(input, at, Text::translate(key, &[])).with_color(RED));
    let sub_at = "pingwheel".len();
    let (sub, at) = match rest(input, sub_at) {
        Rest::End => return help(game),
        Rest::Bad(at) => return error(game, at, "command.unknown.argument"),
        Rest::Arg(a, at) => (a, at),
    };
    let word = sub.split(' ').next().unwrap_or_default();
    let after = at + word.len();
    match word {
        "help" | "config" if !matches!(rest(input, after), Rest::End) => error(game, after, "command.unknown.argument"),
        "help" => help(game),
        "config" => client(|c| c.open_settings = true),
        "channel" => match rest(input, after) {
            Rest::End => {
                let current = channel();
                if current.is_empty() {
                    feedback(game, true, tr("command.pingwheel.channel.get.response.default", Vec::new()));
                } else {
                    feedback(game, true, tr("command.pingwheel.channel.get.response", vec![Text::colored(current, YELLOW)]));
                }
            }
            Rest::Bad(at) => error(game, at, "command.unknown.argument"),
            Rest::Arg(arg, at) => match read_string(arg) {
                Err((key, i)) => error(game, at + i, key),
                Ok((_, used)) if used < arg.len() => {
                    let key = if arg[used..].starts_with(' ') { "command.unknown.argument" } else { "command.expected.separator" };
                    error(game, at + used, key)
                }
                Ok((name, _)) => {
                    if name.chars().count() > MAX_CHANNEL_LENGTH {
                        feedback(game, false, tr("command.pingwheel.channel.set.reject", vec![Text::literal(MAX_CHANNEL_LENGTH.to_string())]));
                        return;
                    }
                    config(|c| set_channel(c, &name));
                    save(game);
                    if name.is_empty() {
                        feedback(game, true, tr("command.pingwheel.channel.set.response.default", Vec::new()));
                    } else {
                        feedback(game, true, tr("command.pingwheel.channel.set.response", vec![Text::colored(name, YELLOW)]));
                    }
                }
            },
        },
        _ => error(game, at, "command.unknown.argument"),
    }
}

/// Hook `commands`: `/pingwheel` is the client's own.
pub(super) fn commands(commands: &mut Vec<&'static str>) {
    commands.push("pingwheel");
}

// ---- Options > Mods, Mod Menu, the screens ------------------------------------------------------

/// Hook `mods`: the mod's row in Options > Mods, with its keys.
pub(super) fn list(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: ID.into(),
        title: format!("{NAME} {VERSION}"),
        description: if lang::russian() { DESCRIPTION_RU } else { DESCRIPTION }.into(),
        keys: KEYS.iter().map(|(name, label, default)| [(*name).to_owned(), t(label), (*default).to_owned()]).collect(),
    });
}

/// Hook `settings`: the mod's page, Configure... first.
pub(super) fn page(mod_id: &str, labels: &mut Vec<String>) {
    if mod_id == ID {
        labels.push(if lang::russian() { "Настройки..." } else { "Configure..." }.into());
    }
}

/// Hook `settings_click`: the page's Configure..., or Mod Menu's Configure (`ModMenuIntegration`:
/// the settings screen with Mod Menu as its parent).
pub(super) fn page_click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id == ID && (button == 0 || button == screen::MODMENU_CONFIGURE) {
        screen::open_over_options(game, button != 0);
    }
}

pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, view: &NativeScreenView<'_>) {
    screen::render(game, batch, view);
}

/// The original's icon and what Mod Menu shows (`fabric.mod.json`).
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
  "authors": ["Luken"],
  "contributors": ["RXJpaw", "DeportedToHell"],
  "license": "MIT",
  "links": {{
    "homepage": "https://www.curseforge.com/minecraft/mc-mods/ping-wheel",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/pingwheel",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "config_screen": true
}}
"#
    )
}

/// `icon.png` and `modmenu.json` in the mod's data folder, each written only when it differs.
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

// ---- the server half ----------------------------------------------------------------------------

struct ServerSide {
    core: ServerCore,
    config: ServerConfig,
    saved: Option<ServerConfig>,
    path: Option<PathBuf>,
    clock: Instant,
}

static SERVER_SIDE: Mutex<Option<ServerSide>> = Mutex::new(None);

/// `ServerConfig.HANDLER.load` (Ping Wheel loads it at start, on clients too).
fn server_side() -> MutexGuard<'static, Option<ServerSide>> {
    let mut g = lock(&SERVER_SIDE);
    if g.is_none() {
        let path = config_path(".server.json");
        let text = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok());
        let mut s = ServerSide { core: ServerCore::new(), config: ServerConfig::default(), saved: None, path, clock: Instant::now() };
        match text.map(|t| ServerConfig::parse(&t)) {
            Some(Some(mut c)) => {
                c.validate();
                log::info!(target: LOG, "{PREFIX}Loaded {c}");
                s.saved = Some(c.clone());
                s.config = c;
            }
            Some(None) => log::error!(target: LOG, "{PREFIX}Config is broken -> reset to defaults"),
            None => {}
        }
        save_server(&mut s);
        *g = Some(s);
    }
    g
}

/// `ConfigHandler.save` of the server's config.
fn save_server(s: &mut ServerSide) {
    if s.saved.as_ref() == Some(&s.config) {
        return;
    }
    if let Some(path) = &s.path
        && !write_file(path, &s.config.write())
    {
        return;
    }
    s.saved = Some(s.config.clone());
    log::info!(target: LOG, "{PREFIX}Saved {}", s.config);
}

/// A player as the relay needs them: name and scoreboard team.
fn relay_player(game: &Game<'_>, p: &NativeServerPlayer) -> Player {
    Player { uuid: p.uuid, name: p.name.clone(), team: game.score.board.players_team(&p.name).map(|t| t.name.clone()) }
}

/// The players' operator levels, as the last server tick read them.
static LEVELS: Mutex<Option<HashMap<u128, u8>>> = Mutex::new(None);

/// Hook `server_tick`, at its end: the players' levels for `/pingwheel:server`.
pub(super) fn server_tick(game: &mut Game<'_>, phase: ServerTickPhase) {
    if game.window.is_some() || !matches!(phase, ServerTickPhase::ServerTickEnd { .. }) {
        return;
    }
    let levels: HashMap<u128, u8> = game.native_server_players().iter().map(|p| (p.uuid, p.permission)).collect();
    *lock(&LEVELS) = Some(levels);
}

/// Hook `world_lifecycle`: the server half's state lives with the world.
pub(super) fn world(game: &mut Game<'_>, phase: WorldPhase) {
    if game.window.is_some() {
        return;
    }
    match phase {
        WorldPhase::WorldOpenReady => drop(server_side()),
        WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed => {
            if let Some(s) = lock(&SERVER_SIDE).as_mut() {
                s.core = ServerCore::new();
            }
        }
        _ => {}
    }
}

/// Hook `player_logged_in`: the channels the server takes, registered with the client (as a Fabric
/// or Bukkit server registers them), so a Java client's `canSend` says yes.
pub(super) fn logged_in(game: &mut Game<'_>, player: u128) {
    if game.window.is_some() || !game.native_on(ID) {
        return;
    }
    let _ = game.native_send_custom_payload_to_player(player, "minecraft:register", &protocol::register_payload(&[PING_LOCATION_C2S, UPDATE_CHANNEL_C2S]));
}

/// Hook `player_logged_out` (`onPlayerDisconnect`).
pub(super) fn logged_out(game: &mut Game<'_>, player: u128) {
    if game.window.is_some() {
        return;
    }
    if let Some(s) = server_side().as_mut() {
        s.core.disconnect(player);
    }
}

/// A guest's plugin message (`native_custom_payload_from_client`): a ping, or a channel.
pub(super) fn payload(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if game.window.is_some() || !game.native_on(ID) || (channel != PING_LOCATION_C2S && channel != UPDATE_CHANNEL_C2S) {
        return;
    }
    let players = game.native_server_players();
    let online: Vec<Player> = players.iter().map(|p| relay_player(game, p)).collect();
    let Some(sender) = online.iter().find(|p| p.uuid == player).cloned() else { return };
    let out = {
        let mut g = server_side();
        let Some(s) = g.as_mut() else { return };
        if channel == UPDATE_CHANNEL_C2S {
            s.core.on_channel_update(&sender, data)
        } else {
            let now = s.clock.elapsed().as_nanos() as i128;
            let config = s.config.clone();
            s.core.on_ping(&sender, data, &online, &config, now)
        }
    };
    for o in out {
        match o {
            // `ServerPlayNetworking.canSend`: only to clients that registered the channel.
            Out::Send { to, data } => {
                if game.native_player_channels(to).iter().any(|c| c == PING_LOCATION_S2C) {
                    let _ = game.native_send_custom_payload_to_player(to, PING_LOCATION_S2C, &data);
                }
            }
            Out::Tell { to, legacy } => {
                game.native_tell(to, Text::legacy(&legacy));
            }
            Out::Log { warn: true, line } => log::warn!(target: LOG, "{line}"),
            Out::Log { warn: false, line } => log::info!(target: LOG, "{line}"),
        }
    }
}

/// `/pingwheel:server`'s answer: success, or a failure in red.
struct Reply {
    ok: bool,
    text: Text,
}

/// `ServerCommandBuilder`: `/pingwheel:server [help | default_channel [<mode_name>] |
/// player_tracking [true|false] | regen_time [<time>] | rate_limit [<limit>]]`. `tr` makes a
/// translatable text (the player's client translates it; the console's is English). None when
/// the line isn't this command.
fn server_command(input: &str, tr: &dyn Fn(&str, Vec<Text>) -> Text) -> Option<Reply> {
    let input = input.strip_prefix('/').unwrap_or(input);
    let label = "pingwheel:server";
    if input.split(' ').next() != Some(label) {
        return None;
    }
    let ok = |text: Text| Some(Reply { ok: true, text: prefixed(text) });
    let fail = |text: Text| Some(Reply { ok: false, text: prefixed(text) });
    let syntax = |at: usize, message: Text| Some(Reply { ok: false, text: syntax_error(input, at, message) });
    let yellow = |t: Text| t.with_color(YELLOW);
    let value = |mode: ChannelMode| yellow(tr(&format!("value.pingwheel.{}", mode.lower()), Vec::new()));
    let help = || {
        let lines = [
            ("/pingwheel:server default_channel", "command.pingwheel.default_channel.get.description"),
            ("/pingwheel:server default_channel <mode_name>", "command.pingwheel.default_channel.set.description"),
            ("/pingwheel:server player_tracking", "command.pingwheel.player_tracking.get.description"),
            ("/pingwheel:server player_tracking true|false", "command.pingwheel.player_tracking.set.description"),
            ("/pingwheel:server regen_time", "command.pingwheel.regen_time.get.description"),
            ("/pingwheel:server regen_time <milliseconds>", "command.pingwheel.regen_time.set.description"),
            ("/pingwheel:server rate_limit", "command.pingwheel.rate_limit.get.description"),
            ("/pingwheel:server rate_limit <limit>", "command.pingwheel.rate_limit.set.description"),
        ];
        let mut parts = vec![Text::empty()];
        for (usage, key) in lines {
            parts.push(Text::literal(usage));
            parts.push(wrapped(key, tr));
        }
        ok(join(parts))
    };
    let (sub, at) = match rest(input, label.len()) {
        Rest::End => return help(),
        Rest::Bad(at) => return syntax(at, tr("command.unknown.argument", Vec::new())),
        Rest::Arg(a, at) => (a, at),
    };
    let word = sub.split(' ').next().unwrap_or_default();
    let after = at + word.len();
    let arg = rest(input, after);
    if let Rest::Bad(at) = arg {
        return syntax(at, tr("command.unknown.argument", Vec::new()));
    }
    let mut g = server_side();
    let s = g.as_mut()?;
    let int_arg = |a: &str, at: usize| -> Result<i32, Option<Reply>> {
        let end = a.find(|c: char| !(c.is_ascii_digit() || c == '-' || c == '.')).unwrap_or(a.len());
        if end < a.len() {
            return Err(syntax(at + end, tr(if a[end..].starts_with(' ') { "command.unknown.argument" } else { "command.expected.separator" }, Vec::new())));
        }
        let Ok(n) = a.parse::<i32>() else {
            return Err(syntax(at, tr("parsing.int.invalid", vec![Text::literal(a)])));
        };
        if n < 0 {
            return Err(syntax(at, tr("argument.integer.low", vec![Text::literal("0"), Text::literal(n.to_string())])));
        }
        Ok(n)
    };
    match (word, arg) {
        ("help", Rest::End) => help(),
        ("default_channel", Rest::End) => ok(tr("command.pingwheel.default_channel.get.response", vec![value(s.config.default_channel_mode)])),
        ("default_channel", Rest::Arg(a, at)) => {
            let (name, used) = match read_string(a) {
                Ok(v) => v,
                Err((key, i)) => return syntax(at + i, tr(key, Vec::new())),
            };
            if used < a.len() {
                return syntax(at + used, tr("command.unknown.argument", Vec::new()));
            }
            let Some(mode) = ChannelMode::parse(&name) else {
                let names: Vec<String> = ChannelMode::ALL.iter().map(|m| m.lower()).collect();
                return fail(tr("command.pingwheel.default_channel.set.reject", vec![Text::literal(names.join(" | "))]));
            };
            s.config.default_channel_mode = mode;
            save_server(s);
            ok(tr("command.pingwheel.default_channel.set.response", vec![value(mode)]))
        }
        ("player_tracking", Rest::End) => ok(tr("command.pingwheel.player_tracking.get.response", vec![yellow(Text::literal(s.config.player_tracking_enabled.to_string()))])),
        ("player_tracking", Rest::Arg(a, at)) => {
            let word = a.split(' ').next().unwrap_or_default();
            let state = match word {
                "true" => true,
                "false" => false,
                _ => return syntax(at, tr("parsing.bool.invalid", vec![Text::literal(word)])),
            };
            if word.len() < a.len() {
                return syntax(at + word.len(), tr("command.unknown.argument", Vec::new()));
            }
            s.config.player_tracking_enabled = state;
            save_server(s);
            ok(tr("command.pingwheel.player_tracking.set.response", vec![yellow(Text::literal(state.to_string()))]))
        }
        ("regen_time", Rest::End) => ok(tr("command.pingwheel.regen_time.get.response", vec![yellow(Text::literal(s.config.ms_to_regenerate.to_string()))])),
        ("regen_time", Rest::Arg(a, at)) => match int_arg(a, at) {
            Err(e) => e,
            Ok(n) => {
                s.config.ms_to_regenerate = n;
                save_server(s);
                ok(tr("command.pingwheel.regen_time.set.response", vec![yellow(Text::literal(n.to_string()))]))
            }
        },
        ("rate_limit", Rest::End) => ok(tr("command.pingwheel.rate_limit.get.response", vec![yellow(Text::literal(s.config.rate_limit.to_string()))])),
        ("rate_limit", Rest::Arg(a, at)) => match int_arg(a, at) {
            Err(e) => e,
            Ok(n) => {
                s.config.rate_limit = n;
                save_server(s);
                ok(tr("command.pingwheel.rate_limit.set.response", vec![yellow(Text::literal(n.to_string()))]))
            }
        },
        (_, Rest::Arg(_, _)) | ("help", _) => syntax(after, tr("command.unknown.argument", Vec::new())),
        _ => syntax(at, tr("command.unknown.argument", Vec::new())),
    }
}

/// Hook `player_command`: `/pingwheel:server` from a game master (permission level 2). Others
/// get the game's unknown-command answer, as the command isn't theirs to see in Java.
pub(super) fn player_command(game: &mut Game<'_>, player: u128, command: &str, canceled: &mut bool, replies: &mut Vec<Text>) {
    if game.window.is_some() || !game.native_on(ID) || command.split(' ').next() != Some("pingwheel:server") {
        return;
    }
    // `requires(COMMANDS_GAMEMASTER)`: the hub is out of reach here, so the level is the one the
    // server tick last read (a permissions mod may grant the node instead).
    let level = lock(&LEVELS).as_ref().and_then(|l| l.get(&player).copied()).unwrap_or(0);
    if level < 2 && !game.native_has_permission(player, "pingwheel.command.server", 2) {
        return;
    }
    let tr = |key: &str, args: Vec<Text>| Text::translate(key, &args);
    let Some(reply) = server_command(command, &tr) else { return };
    *canceled = true;
    replies.push(if reply.ok { reply.text } else { Text::empty().append(reply.text).with_color(RED) });
}

/// The console's line (`native_console_line`): `/pingwheel:server` at level 4, answered in English.
pub(super) fn console_line(game: &mut Game<'_>, line: &str, cancel: &mut bool, replies: &mut Vec<Text>) {
    if game.window.is_some() || !game.native_on(ID) {
        return;
    }
    let tr = |key: &str, args: Vec<Text>| {
        let plain: Vec<String> = args.iter().map(|a| a.resolved().plain()).collect();
        let refs: Vec<&str> = plain.iter().map(String::as_str).collect();
        match lang::ENGLISH.iter().find(|(k, _)| *k == key) {
            Some(_) => Text::literal(lang::format(&english(key), &refs)),
            None => Text::translate(key, &args),
        }
    };
    let Some(reply) = server_command(line.trim(), &tr) else { return };
    *cancel = true;
    replies.push(Text::literal(reply.text.resolved().plain()));
}

fn english(key: &str) -> String {
    lang::ENGLISH.iter().find(|(k, _)| *k == key).map_or_else(|| key.to_owned(), |(_, v)| (*v).to_owned())
}

