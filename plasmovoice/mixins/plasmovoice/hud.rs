// What Plasmo Voice draws over the game: the activation icon (`HudIconRenderer`), the list of
// who is talking (`OverlayRenderer`), and the icons over players and sources (`EntityIconRenderer`
// with `EntityIconStateExtractor`, `StaticSourceIconRenderer`), the last ones projected from the
// world with this frame's camera and drawn as sprites of the GUI.
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use super::Game;
use super::plasmovoice_config::Config;
use super::plasmovoice_icons as icons;
use super::plasmovoice_lang::t;
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::text::Text;
use plasmovoice_engine::client::{SourceStatus, Status};
use plasmovoice_engine::tcp::{Activation, SourceKind, SourceLine};
use plasmovoice_engine::wire::Uuid;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// What the HUD knows this frame, filled by the glue.
#[derive(Clone, Default)]
pub(super) struct Snap {
    pub status: Status,
    /// `getActivations` (heaviest first) and `getLines`.
    pub activations: Vec<Activation>,
    pub lines: Vec<SourceLine>,
    pub me: u128,
    /// Player list profiles (who is online) and their skins.
    pub skins: HashMap<u128, String>,
    /// The client knows a voice server (`serverInfo.isPresent`) and has a UDP client.
    pub server: bool,
    /// Entities whose name tag shows this frame (`EntityRenderState.nameTag`).
    pub name_tags: HashSet<u32>,
}

thread_local! {
    static SNAP: RefCell<Snap> = RefCell::new(Snap::default());
    static TAGS: RefCell<HashSet<u32>> = RefCell::new(HashSet::new());
}

pub(super) fn set(mut snap: Snap) {
    snap.name_tags = TAGS.with(|t| std::mem::take(&mut *t.borrow_mut()));
    SNAP.with(|s| *s.borrow_mut() = snap);
}

/// Hook `render_name_tag`: whose name tags the world drew.
pub(super) fn name_tag(entity: u32, visible: bool) {
    if visible {
        TAGS.with(|t| t.borrow_mut().insert(entity));
    }
}

fn argb(c: u32) -> u32 {
    let [a, r, g, b] = c.to_be_bytes();
    rgba(r, g, b, a)
}

// ---- the activation icon --------------------------------------------------------------------

/// `IconPosition`: x (None centred), y; negative from the right or the bottom.
fn icon_position(name: &str) -> (Option<i32>, i32) {
    match name {
        "TOP_LEFT" => (Some(16), 16),
        "TOP_CENTER" => (None, 16),
        "TOP_RIGHT" => (Some(-16), 16),
        "BOTTOM_LEFT" => (Some(16), -16),
        "BOTTOM_RIGHT" => (Some(-16), -16),
        _ => (None, -36),
    }
}

/// `HudIconRenderer.onRender`.
pub(super) fn activation_icon(game: &Game<'_>, batch: &mut HudBatch, cfg: &Config) {
    SNAP.with(|s| {
        let s = s.borrow();
        let st = &s.status;
        if !st.connected || game.native_player().is_none() || !cfg.b("overlay.show_activation_icon") {
            return;
        }
        let icon = if st.timed_out {
            "plasmovoice:textures/icons/microphone_disconnected.png".to_owned()
        } else if cfg.b("voice.disabled") {
            "plasmovoice:textures/icons/speaker_disabled.png".to_owned()
        } else if st.server_muted {
            "plasmovoice:textures/icons/microphone_muted.png".to_owned()
        } else if cfg.b("voice.microphone_disabled") {
            "plasmovoice:textures/icons/microphone_disabled.png".to_owned()
        } else {
            // From the lightest activation up: the last active one, stopping at the first that
            // isn't transitive.
            let mut current: Option<&Activation> = None;
            for a in s.activations.iter().rev() {
                if !st.talking.iter().any(|(n, _)| *n == a.name) {
                    continue;
                }
                current = Some(a);
                if !a.transitive {
                    break;
                }
            }
            match current {
                Some(a) => a.icon.clone(),
                None => return,
            }
        };
        let pos = cfg.s("overlay.activation_icon_position");
        let (x, y) = icon_position(&pos);
        let (w, h) = (batch.width.floor(), batch.height.floor());
        let x = match x {
            None => (w / 2.0).floor() - 8.0,
            Some(x) if x < 0 => w + x as f32 - 16.0,
            Some(x) => x as f32,
        };
        let mut y = if y < 0 { h + y as f32 - 16.0 } else { y as f32 };
        if pos == "BOTTOM_CENTER" && game.creative {
            y -= 10.0;
        }
        icons::draw(batch, &game.gui.atlas, &icon, x, y, 16.0, 16.0, 0xFFFF_FFFF);
    });
}

// ---- the overlay ----------------------------------------------------------------------------

struct Entry {
    name: String,
    player: Option<u128>,
    activated: bool,
}

/// `OverlayPosition`.
fn overlay_position(name: &str) -> (i32, i32) {
    match name {
        "TOP_RIGHT" => (-4, 4),
        "BOTTOM_LEFT" => (4, -4),
        "BOTTOM_RIGHT" => (-4, -4),
        _ => (4, 4),
    }
}

/// `getSourceSenderName`: the source's own name (cut at 40), a direct source's sender, a
/// player's nick, else the line's name.
fn sender_name(s: &Snap, src: &SourceStatus, line: &SourceLine) -> String {
    if let Some(n) = &src.name {
        return if n.chars().count() > 40 { n.chars().take(40).collect::<String>() + "..." } else { n.clone() };
    }
    match &src.kind {
        SourceKind::Direct { sender: Some(p), .. } => p.name.clone(),
        SourceKind::Player(p) => s.status.players.iter().find(|x| x.id == p.id).map_or_else(|| p.nick.clone(), |x| x.nick.clone()),
        _ => t(&line.translation),
    }
}

fn sender(src: &SourceStatus) -> (u128, Option<u128>) {
    match &src.kind {
        SourceKind::Direct { sender: Some(p), .. } => (p.id.0, Some(p.id.0)),
        SourceKind::Direct { .. } => (src.id.0, None),
        SourceKind::Player(p) => (p.id.0, Some(p.id.0)),
        _ => (src.id.0, None),
    }
}

/// `OverlayRenderer.onRender`.
pub(super) fn overlay(game: &Game<'_>, batch: &mut HudBatch, cfg: &Config) {
    SNAP.with(|s| {
        let s = s.borrow();
        if !s.server || game.native_player().is_none() || !cfg.b("overlay.overlay_enabled") {
            return;
        }
        let position = cfg.s("overlay.overlay_position");
        let mut index = 0;
        for line in &s.lines {
            let state = cfg.s(&format!("overlay.source_states.{}", line.name));
            if state == "OFF" || state == "NEVER" {
                continue;
            }
            let heard = s.status.sources.iter().filter(|src| src.line == line.name && src.activated);
            let mut entries: Vec<Entry> = Vec::new();
            if let (Some(players), "ALWAYS") = (&line.players, state.as_str()) {
                let mut players = players.clone();
                players.sort_by(|a, b| a.name.cmp(&b.name));
                for p in players {
                    entries.push(Entry { name: p.name.clone(), player: Some(p.id.0), activated: false });
                }
                // Senders of the line's direct sources heard now.
                for src in heard {
                    if let SourceKind::Direct { sender: Some(p), .. } = &src.kind
                        && let Some(e) = entries.iter_mut().find(|e| e.player == Some(p.id.0))
                    {
                        e.activated = true;
                    }
                }
            } else {
                for src in heard {
                    let (_, player) = sender(src);
                    entries.push(Entry { name: sender_name(&s, src, line), player, activated: true });
                }
            }
            for e in entries {
                entry(game, batch, cfg, &s, line, &position, index, &e);
                index += 1;
            }
        }
    });
}

/// `renderEntry`: the face, the name on the text background, the line's icon when talking.
#[allow(clippy::too_many_arguments)]
fn entry(game: &Game<'_>, batch: &mut HudBatch, cfg: &Config, s: &Snap, line: &SourceLine, position: &str, index: i32, e: &Entry) {
    let atlas = &game.gui.atlas;
    let style = cfg.s("overlay.overlay_style");
    let (px, py) = overlay_position(position);
    let right = px < 0;
    let bottom = py < 0;
    let (w, h) = (batch.width.floor(), batch.height.floor());
    let mut x = if px < 0 { w + px as f32 } else { px as f32 };
    let mut y = if py < 0 { h + py as f32 } else { py as f32 };
    if bottom {
        y -= 17.0 * (index + 1) as f32;
    } else {
        y += 17.0 * index as f32;
    }
    // `Options.getBackgroundColor(0.25f)` with "chat background only" on, as Horizon has it.
    let background = argb(((0.25f32 * 255.0) as u32) << 24);
    if style != "NAME"
        && let Some(id) = e.player
    {
        if right {
            x -= 16.0;
        }
        let skin = s.skins.get(&id).cloned().unwrap_or_else(|| horizon_client::tab_list::default_skin(id).to_owned());
        let mut draws = Vec::new();
        horizon_client::tab_list::face(&mut draws, &skin, x as i32, y as i32, 16, true, false, horizon_client::hud_layout::WHITE);
        game.native_gui_paint(batch, &draws);
        if !right {
            x += 17.0;
        }
    }
    let name = Text::literal(e.name.clone());
    if style != "SKIN" && !e.name.trim().is_empty() {
        let text_w = atlas.font.width_text(&name) + 8.0;
        if right {
            x -= text_w + 1.0;
        }
        batch.fill(atlas, x, y, text_w, 16.0, background);
        batch.text_styled(atlas, &name, x + 4.0, y + 4.0, u32::MAX, false);
        if e.activated && !right {
            x += text_w + 1.0;
        }
    }
    if e.activated {
        if right {
            x -= 17.0;
        }
        batch.fill(atlas, x, y, 16.0, 16.0, background);
        icons::draw(batch, atlas, &line.icon, x, y, 16.0, 16.0, 0xFFFF_FFFF);
    }
}

// ---- the player list (a Horizon addition) ----

/// Each player with voice chat's icon in the player list (`plasmovoice_tab`): the white
/// microphone (the HUD's activation icon) while they talk, crossed out in red otherwise. The own
/// row by this client's own talking. None with the option off or no voice connection.
pub(super) fn tab_icons(cfg: &Config, me: u128) -> Option<HashMap<u128, &'static str>> {
    if !cfg.b("horizon.tab_list_icons") {
        return None;
    }
    SNAP.with(|s| {
        let s = s.borrow();
        if !s.status.connected {
            return None;
        }
        let talking = |id: u128| {
            if id == me {
                !s.status.talking.is_empty()
            } else {
                s.status.sources.iter().any(|src| src.activated && matches!(&src.kind, SourceKind::Player(x) if x.id.0 == id))
            }
        };
        let icons = s
            .status
            .players
            .iter()
            .map(|p| (p.id.0, if talking(p.id.0) { "microphone" } else { "microphone_disabled" }))
            .collect();
        Some(icons)
    })
}

// ---- icons in the world ---------------------------------------------------------------------

/// The highest line of `sources` activated with its icon shown (`getHighestActivatedSourceLine`).
fn highest<'a>(sources: impl Iterator<Item = &'a SourceStatus>) -> Option<&'a SourceStatus> {
    sources.filter(|src| src.activated && src.icon_visible).max_by_key(|src| src.line_weight)
}

/// `EntityIconStateExtractor.getPlayerIcon`.
fn player_icon(s: &Snap, cfg: &Config, uuid: u128, entity: u32) -> Option<String> {
    let hidden = |v: &str| s.status.icon_hidden.iter().any(|h| h == v);
    let by_source = || -> Option<String> {
        if hidden("HIDE_SOURCE_ICON") {
            return None;
        }
        let player = highest(s.status.sources.iter().filter(|src| matches!(&src.kind, SourceKind::Player(p) if p.id.0 == uuid)));
        let found = player.or_else(|| highest(s.status.sources.iter().filter(|src| matches!(src.kind, SourceKind::Entity(e) if e as u32 == entity))));
        found.map(|src| src.line_icon.clone())
    };
    let Some(info) = s.status.players.iter().find(|p| p.id.0 == uuid) else {
        return by_source().or_else(|| (!hidden("HIDE_NOT_INSTALLED")).then(|| "plasmovoice:textures/icons/headset_not_installed.png".into()));
    };
    if cfg.b(&format!("voice.volumes.source_{}.muted", Uuid(uuid))) {
        return (!hidden("HIDE_CLIENT_MUTED")).then(|| "plasmovoice:textures/icons/speaker_disabled.png".into());
    }
    if info.muted {
        return (!hidden("HIDE_SERVER_MUTED")).then(|| "plasmovoice:textures/icons/speaker_muted.png".into());
    }
    if info.voice_disabled {
        return (!hidden("HIDE_VOICE_CHAT_DISABLED")).then(|| "plasmovoice:textures/icons/headset_disabled.png".into());
    }
    by_source()
}

/// A world point to GUI pixels with the frame's camera (camera-relative view, reversed-Z
/// projection), and its depth; None behind the camera.
fn project(cam: &horizon_client::mod_api::NativeCamera, p: [f64; 3], w: f32, h: f32) -> Option<(f32, f32)> {
    let rel = [(p[0] - cam.position[0]) as f32, (p[1] - cam.position[1]) as f32, (p[2] - cam.position[2]) as f32, 1.0];
    let mul = |m: &[f32; 16], v: [f32; 4]| -> [f32; 4] {
        let mut out = [0.0; 4];
        for (r, o) in out.iter_mut().enumerate() {
            *o = (0..4).map(|c| m[c * 4 + r] * v[c]).sum();
        }
        out
    };
    let clip = mul(&cam.projection, mul(&cam.view, rel));
    if clip[3] <= 1e-4 {
        return None;
    }
    let (nx, ny) = (clip[0] / clip[3], clip[1] / clip[3]);
    Some(((nx + 1.0) * 0.5 * w, (1.0 - ny) * 0.5 * h))
}

/// The camera's up and right in the world (rows of the view's rotation).
fn axes(cam: &horizon_client::mod_api::NativeCamera) -> ([f64; 3], [f64; 3]) {
    let m = &cam.view;
    (
        [f64::from(m[1]), f64::from(m[5]), f64::from(m[9])],
        [f64::from(m[0]), f64::from(m[4]), f64::from(m[8])],
    )
}

/// A 10 × 10 billboard of 0.025-block units (`EntityIconRenderer.quad`) whose top centre is at
/// `top` (or whose centre is, `centred`): drawn opaque where the camera sees it, at alpha 40
/// through blocks (the see-through pass).
#[allow(clippy::too_many_arguments)]
fn billboard(game: &Game<'_>, batch: &mut HudBatch, cam: &horizon_client::mod_api::NativeCamera, icon: &str, at: [f64; 3], centred: bool) {
    let (up, _) = axes(cam);
    let (w, h) = (batch.width, batch.height);
    let off = |p: [f64; 3], k: f64| [p[0] + up[0] * k, p[1] + up[1] * k, p[2] + up[2] * k];
    let (top, bottom) = if centred { (off(at, 0.125), off(at, -0.125)) } else { (at, off(at, -0.25)) };
    let (Some(t), Some(b)) = (project(cam, top, w, h), project(cam, bottom, w, h)) else { return };
    let size = ((b.0 - t.0).powi(2) + (b.1 - t.1).powi(2)).sqrt();
    if !(0.5..=4096.0).contains(&size) {
        return;
    }
    let mid = off(top, -0.125);
    let seen = game.native_client_visual_clear(cam.position, mid);
    let alpha: u32 = if seen { 255 } else { 40 };
    let x = (t.0 + b.0) / 2.0 - size / 2.0;
    icons::draw(batch, &game.gui.atlas, icon, x, t.1, size, size, (alpha << 24) | 0x00FF_FFFF);
}

/// This frame's camera: `native_camera` when the game keeps it (it does only for API mods),
/// else built as the frame builds it (`Camera.setup` left `view`, then bobbing, the nausea
/// spin, the dynamic FOV and the reversed-Z projection).
fn camera(game: &Game<'_>, aspect: f32) -> Option<horizon_client::mod_api::NativeCamera> {
    if let Some(c) = game.native_camera() {
        return Some(c);
    }
    game.native_player()?;
    let partial = game.clock.camera_fraction() as f32;
    let (position, yaw, pitch) = (game.view.position, game.view.y_rot, game.view.x_rot);
    let fov = game.fov_degrees(partial, position);
    let view = game.bob_matrix(partial) * game.spinning_matrix(partial) * horizon_client::camera::view_matrix(yaw, pitch);
    let far = horizon_client::camera::depth_far(game.terrain_distance(), game.settings.cloud_range);
    Some(horizon_client::mod_api::NativeCamera {
        position,
        eye_height: game.view.eye_height,
        partial_tick: partial,
        yaw,
        pitch,
        fov_degrees: fov,
        near: 0.05,
        far,
        view: view.to_cols_array(),
        projection: glam::Mat4::perspective_rh(fov.to_radians(), aspect, far, 0.05).to_cols_array(),
        dimension: game.dim.id().to_string(),
    })
}

/// `renderPercent`: the volume as text over the player, 0.025 a text pixel, on the text
/// background.
fn percent_text(game: &Game<'_>, batch: &mut HudBatch, cam: &horizon_client::mod_api::NativeCamera, text: &str, top: [f64; 3]) {
    let (up, _) = axes(cam);
    let (w, h) = (batch.width, batch.height);
    let below = [top[0] - up[0] * 0.25, top[1] - up[1] * 0.25, top[2] - up[2] * 0.25];
    let (Some(t), Some(b)) = (project(cam, top, w, h), project(cam, below, w, h)) else { return };
    let scale = ((b.0 - t.0).powi(2) + (b.1 - t.1).powi(2)).sqrt() / 10.0;
    if !(0.01..=400.0).contains(&scale) {
        return;
    }
    let atlas = &game.gui.atlas;
    let line = Text::literal(text);
    let tw = atlas.font.width_text(&line);
    let start = batch.vertices.len();
    let x = t.0 - tw / 2.0;
    batch.fill(atlas, x - 1.0, t.1 - 1.0, tw + 1.0, 10.0, argb(((0.25f32 * 255.0) as u32) << 24));
    batch.text_styled(atlas, &line, x, t.1, u32::MAX, false);
    batch.scale_since(start, t.0, t.1, scale, scale);
}

/// The icons over players, entities and static sources (`EntityIconStateExtractor.extract`,
/// `StaticSourceIconStateExtractor`), when `show_source_icons` lets them show: 0 with the HUD,
/// 1 always, 2 never.
pub(super) fn world_icons(game: &Game<'_>, batch: &mut HudBatch, cfg: &Config, gui_hidden: bool) {
    let show = cfg.i("overlay.show_source_icons");
    if show == 2 || (show == 0 && gui_hidden) {
        return;
    }
    let Some(cam) = camera(game, batch.width / batch.height.max(1.0)) else { return };
    SNAP.with(|s| {
        let s = s.borrow();
        if !s.status.connected {
            return;
        }
        let partial = game.native_client_state().map_or(1.0, |st| st.world_partial_tick);
        let entities = game.native_client_entities();
        // (entity, icon, a listed player, the volume shown over them).
        let mut want: Vec<(u32, Option<String>, bool, Option<String>)> = Vec::new();
        for e in &entities {
            if e.invisible || !e.alive {
                continue;
            }
            // In the player list (`onlinePlayerIds`; the voice list too, which a single player's
            // own world may have alone).
            let online = |u: u128| s.skins.contains_key(&u) || s.status.players.iter().any(|p| p.id.0 == u);
            let icon = match e.player_uuid {
                // Not in the player list: a fake player, by its entity sources only.
                Some(uuid) if !online(uuid) => {
                    highest(s.status.sources.iter().filter(|src| matches!(src.kind, SourceKind::Entity(id) if id as u32 == e.id))).map(|src| src.line_icon.clone())
                }
                Some(uuid) if uuid == s.me => None,
                Some(uuid) => player_icon(&s, cfg, uuid, e.id),
                None => highest(s.status.sources.iter().filter(|src| matches!(src.kind, SourceKind::Entity(id) if id as u32 == e.id))).map(|src| src.line_icon.clone()),
            };
            // `PlayerVolumeAction.isShown`: the volume, a second after the wheel changed it.
            let percent = e.player_uuid.filter(|u| super::plasmovoice_glue::volume_shown(*u)).map(|u| {
                let v = cfg.f(&format!("voice.volumes.source_{}.volume", Uuid(u)));
                format!("{}%", (v * 100.0).round() as i64)
            });
            if icon.is_some() || percent.is_some() {
                want.push((e.id, icon, e.player_uuid.is_some_and(online), percent));
            }
        }
        let ids: Vec<u32> = want.iter().map(|w| w.0).collect();
        let heads = game.native_client_heads(&ids, partial);
        for (id, icon, player, percent) in want {
            // The name tag's attachment (`nameTagAttachment`) and whether a tag shows
            // (`EntityRenderState.nameTag`), else the top of the entity.
            let (a, tagged) = match heads.iter().find(|h| h.id == id) {
                // Horizon's anchor is where the tag is drawn, the attachment lifted 0.5; its half
                // width is 0 when no tag is drawn.
                Some(head) => ([head.name_anchor[0], head.name_anchor[1] - 0.5, head.name_anchor[2]], head.name_half_width > 0.0 || s.name_tags.contains(&id)),
                None => {
                    let Some(e) = entities.iter().find(|e| e.id == id) else { continue };
                    let p = e.position_at(partial);
                    ([p[0], p[1] + f64::from(e.height), p[2]], s.name_tags.contains(&id))
                }
            };
            let d2 = (a[0] - cam.position[0]).powi(2) + (a[1] - cam.position[1]).powi(2) + (a[2] - cam.position[2]).powi(2);
            if d2 > 4096.0 {
                continue;
            }
            // `translateEntityMatrix`: the server's offset for players, 0.3 over a name tag,
            // 0.5 up, and the quad 0.025 over that.
            let offset = if player { s.status.icon_offset } else { [0.0; 3] };
            let mut y = a[1] + offset[1] + 0.5 + 0.025;
            if tagged {
                y += 0.3;
            }
            if let Some(text) = &percent {
                percent_text(game, batch, &cam, text, [a[0] + offset[0], y, a[2] + offset[2]]);
                y += 0.3;
            }
            if let Some(icon) = icon {
                billboard(game, batch, &cam, &icon, [a[0] + offset[0], y, a[2] + offset[2]], false);
            }
        }
        // Static sources (`StaticSourceIconRenderer`): centred on the source.
        if cfg.b("overlay.show_static_source_icons") {
            for src in s.status.sources.iter().filter(|src| src.activated && src.icon_visible) {
                if let SourceKind::Static { pos, .. } = &src.kind {
                    let p = [pos.x, pos.y, pos.z];
                    let d2 = (p[0] - cam.position[0]).powi(2) + (p[1] - cam.position[1]).powi(2) + (p[2] - cam.position[2]).powi(2);
                    if d2 <= 4096.0 {
                        billboard(game, batch, &cam, &src.line_icon, p, true);
                    }
                }
            }
        }
    });
}
