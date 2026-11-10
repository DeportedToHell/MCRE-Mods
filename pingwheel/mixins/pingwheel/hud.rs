// Ping Wheel's markers on the HUD (`OverlayRenderer`, `PingLocationRenderer`,
// `DirectionIndicatorRenderer`, `DrawContext`), drawn as the original draws them through the GUI's
// pose stack: here each part is drawn at its local coordinates and its vertices are put through
// the same 2D matrix (`Affine`, Java's `Matrix3x2f` of translations, scales and rotations).
// Also this frame's camera and the projection of world points onto the GUI
// (`MathUtils.worldToScreen`). Spliced into game.rs as `pingwheel_hud`.
//
// Ping Wheel by Luken (https://github.com/LukenSkyne/Minecraft-Ping-Wheel), MIT.

use super::Game;
use horizon_client::hud::{HudAtlas, HudBatch, Rect, rgba};
use horizon_client::items::ItemStack;
use horizon_client::mod_api::NativeCamera;
use horizon_client::text::Text;
use pingwheel_core::config::{ClientConfig, PlayerInfoMode, TeamColorMode};
use pingwheel_core::math;

pub(super) const WHITE: u32 = 0xFFFF_FFFF;
/// `DrawContext.SHADOW_BLACK`: `ARGB.color(64, 0, 0, 0)`.
const SHADOW_BLACK: u32 = 0x4000_0000;

/// An ARGB colour as the batch packs colours.
pub(super) fn c(argb: u32) -> u32 {
    rgba((argb >> 16) as u8, (argb >> 8) as u8, argb as u8, (argb >> 24) as u8)
}

// ---- the camera ---------------------------------------------------------------------------------

/// This frame's camera: `native_camera` when the game keeps it (it does only for API mods), else
/// built as the frame builds it (the camera's position and turn, the view bobbing, the nausea
/// spin, the FOV and the reversed-Z projection), as `GameRendererMixin` takes the projection with
/// `bobHurt`, `bobView` and the spin.
pub(super) fn camera(game: &Game<'_>, aspect: f32) -> Option<NativeCamera> {
    if let Some(c) = game.native_camera() {
        return Some(c);
    }
    game.native_player()?;
    let partial = game.clock.camera_fraction() as f32;
    let (position, yaw, pitch) = (game.view.position, game.view.y_rot, game.view.x_rot);
    let fov = game.fov_degrees(partial, position);
    let view = game.bob_matrix(partial) * game.spinning_matrix(partial) * horizon_client::camera::view_matrix(yaw, pitch);
    let far = horizon_client::camera::depth_far(game.terrain_distance(), game.settings.cloud_range);
    Some(NativeCamera {
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

/// `MathUtils.worldToScreen`: GUI x, y and the depth (`w`; at or below 0 behind the camera, where
/// the point comes out mirrored as Java's does).
pub(super) fn project(cam: &NativeCamera, p: [f64; 3], w: f32, h: f32) -> [f32; 3] {
    let rel = [(p[0] - cam.position[0]) as f32, (p[1] - cam.position[1]) as f32, (p[2] - cam.position[2]) as f32, 1.0];
    let mul = |m: &[f32; 16], v: [f32; 4]| -> [f32; 4] {
        let mut out = [0.0; 4];
        for (r, o) in out.iter_mut().enumerate() {
            *o = (0..4).map(|k| m[k * 4 + r] * v[k]).sum();
        }
        out
    };
    let clip = mul(&cam.projection, mul(&cam.view, rel));
    let depth = clip[3];
    let (x, y) = if depth != 0.0 { (clip[0] / depth, clip[1] / depth) } else { (clip[0], clip[1]) };
    [w * (0.5 + x * 0.5), h * (0.5 - y * 0.5), depth]
}

// ---- the pose matrix ----------------------------------------------------------------------------

/// `Matrix3x2f`: x' = a x + c y + tx, y' = b x + d y + ty; each step multiplies on the right.
#[derive(Clone, Copy, Debug)]
struct Affine {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    tx: f32,
    ty: f32,
}

impl Affine {
    const IDENTITY: Self = Self { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };
    fn translate(self, x: f32, y: f32) -> Self {
        Self { tx: self.a * x + self.c * y + self.tx, ty: self.b * x + self.d * y + self.ty, ..self }
    }
    fn scale(self, s: f32) -> Self {
        Self { a: self.a * s, b: self.b * s, c: self.c * s, d: self.d * s, ..self }
    }
    /// `rotateLocal(theta)` multiplied on: y down, so a positive angle turns clockwise on screen.
    fn rotate(self, theta: f32) -> Self {
        let (s, co) = theta.sin_cos();
        Self { a: self.a * co + self.c * s, b: self.b * co + self.d * s, c: -self.a * s + self.c * co, d: -self.b * s + self.d * co, ..self }
    }
    fn apply(self, [x, y]: [f32; 2]) -> [f32; 2] {
        [self.a * x + self.c * y + self.tx, self.b * x + self.d * y + self.ty]
    }
    /// What was drawn since `start` at local coordinates, put through the matrix.
    fn since(self, batch: &mut HudBatch, start: usize) {
        for v in &mut batch.vertices[start..] {
            v.pos = self.apply(v.pos);
        }
    }
}

// ---- what is drawn ------------------------------------------------------------------------------

/// Who pinged, as the player list knows them (`PlayerInfo`).
pub(super) struct Author {
    /// `PlayerTeam.formatNameForTeam`.
    pub name: Text,
    /// The name without the team's colour (`withStyle(RESET)`).
    pub plain: Text,
    pub skin: String,
    /// `getTeamColor`: the team's colour, else white (ARGB).
    pub team_color: u32,
}

/// A ping to draw this frame (`PingView`).
pub(super) struct Marker {
    pub screen: [f32; 3],
    pub distance: f64,
    pub scale: f32,
    pub item: Option<ItemStack>,
    pub author: Option<Author>,
}

/// The textures: `arrow.png` and a resource pack's `ping.png`, as rectangles of the atlas.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Sprites {
    pub arrow: Option<Rect>,
    pub ping: Option<Rect>,
}

struct Ctx<'a, 'g> {
    game: &'a Game<'g>,
    atlas: &'a HudAtlas,
    cfg: &'a ClientConfig,
    sprites: Sprites,
    /// The player list key is held (`keyPlayerList.isDown()`).
    list_held: bool,
    /// The distance's grouping and decimal marks.
    marks: (&'static str, &'static str),
}

/// `OverlayRenderer.draw`: the pings of this dimension, far ones first.
pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch, cfg: &ClientConfig, markers: &[Marker], sprites: Sprites, list_held: bool) {
    if markers.is_empty() {
        return;
    }
    let marks = if super::pingwheel_lang::russian() { ("\u{a0}", ",") } else { (",", ".") };
    let ctx = Ctx { game, atlas: &game.gui.atlas, cfg, sprites, list_held, marks };
    let (w, h) = (batch.width, batch.height);
    // `DirectionIndicatorRenderer.prepareSafeZone`.
    let tl = [cfg.safe_zone_left as f32, cfg.safe_zone_top as f32];
    let br = [w - cfg.safe_zone_right as f32, h - cfg.safe_zone_bottom as f32];
    let show_indicator = cfg.direction_indicator_visible;
    for m in markers {
        let behind = m.screen[2] <= 0.0;
        if behind && !show_indicator {
            continue;
        }
        if show_indicator {
            indicator(&ctx, batch, m, (w, h), tl, br);
        }
        if !behind {
            location(&ctx, batch, m);
        }
    }
}

fn team_colors(cfg: &ClientConfig, m: &Marker) -> (u32, u32) {
    let team = m.author.as_ref().map_or(WHITE, |a| a.team_color);
    let labels = matches!(cfg.team_color_mode, TeamColorMode::Full | TeamColorMode::LabelsOnly);
    let ping = matches!(cfg.team_color_mode, TeamColorMode::Full | TeamColorMode::PingOnly);
    (if labels { team } else { WHITE }, if ping { team } else { WHITE })
}

/// `PingLocationRenderer.draw`: the distance over the icon, the player under it.
fn location(ctx: &Ctx<'_, '_>, batch: &mut HudBatch, m: &Marker) {
    let pose = Affine::IDENTITY.translate(m.screen[0], m.screen[1]).scale(m.scale);
    let (distance_color, ping_color) = team_colors(ctx.cfg, m);
    let compact = if ctx.cfg.player_info_mode == PlayerInfoMode::Compact { m.author.as_ref() } else { None };
    let distance = super::pingwheel_lang::tf("unit.pingwheel.meters", &[&math::format_distance(m.distance, ctx.marks.0, ctx.marks.1)]);
    label(ctx, batch, pose, &Text::literal(distance), -1.5, compact.map(|a| a.skin.as_str()), distance_color);
    ping_icon(ctx, batch, pose, m.item.as_ref(), ping_color);
    let held = ctx.cfg.player_info_mode == PlayerInfoMode::Hold && ctx.list_held;
    let verbose = ctx.cfg.player_info_mode == PlayerInfoMode::Always || held;
    if verbose && let Some(a) = &m.author {
        let use_team = matches!(ctx.cfg.team_color_mode, TeamColorMode::Full | TeamColorMode::LabelsOnly);
        let name = if use_team { &a.name } else { &a.plain };
        label(ctx, batch, pose, name, 1.75, Some(&a.skin), WHITE);
    }
}

/// `DirectionIndicatorRenderer.draw`: for a ping off the screen or behind, its icon at the edge
/// of the safe zone, pulled 12 in, and an arrow pointing out at it.
fn indicator(ctx: &Ctx<'_, '_>, batch: &mut HudBatch, m: &Marker, (w, h): (f32, f32), tl: [f32; 2], br: [f32; 2]) {
    let [x, y, depth] = m.screen;
    let behind = depth <= 0.0;
    let in_bounds = x >= 0.0 && x <= w && y >= 0.0 && y <= h;
    if !behind && in_bounds {
        return;
    }
    let centre = [(br[0] - tl[0]) * 0.5, (br[1] - tl[1]) * 0.5];
    let mut dir = [x - tl[0] - centre[0], y - tl[1] - centre[1]];
    if behind {
        dir = [-dir[0], -dir[1]];
    }
    let size = ctx.cfg.ping_size as f32 / 100.0;
    let angle = dir[1].atan2(dir[0]);
    let edge = math::angle_rect_intersection(angle, tl, br);
    let back = f64::from(angle) + std::f64::consts::PI;
    let (ox, oy) = ((back.cos() * 12.0) as f32, (back.sin() * 12.0) as f32);
    let pose = Affine::IDENTITY.translate(edge[0], edge[1]);
    let (_, ping_color) = team_colors(ctx.cfg, m);
    ping_icon(ctx, batch, pose.scale(m.scale).translate(ox, oy), m.item.as_ref(), ping_color);
    let arrow = pose.rotate(angle).scale(size).scale(0.25).translate(-5.0, 0.0);
    if let Some(r) = ctx.sprites.arrow {
        texture(batch, arrow, r, 10.0, ping_color);
    }
}

/// `DrawContext.renderPing`: the item, a resource pack's `ping.png`, or the white diamond.
fn ping_icon(ctx: &Ctx<'_, '_>, batch: &mut HudBatch, pose: Affine, item: Option<&ItemStack>, color: u32) {
    if let Some(stack) = item.filter(|_| ctx.cfg.item_icon_visible) {
        // `guiGraphics.item(stack, -8, -8)`: drawn where the matrix puts it, at its scale.
        let [cx, cy] = pose.apply([0.0, 0.0]);
        let start = batch.vertices.len();
        ctx.game.draw_item_model(batch, stack, cx - 8.0, cy - 8.0);
        batch.scale_since(start, cx, cy, pose.a, pose.d);
    } else if let Some(r) = ctx.sprites.ping {
        texture(batch, pose, r, 12.0, color);
    } else {
        // `renderDefaultPingIcon`: a 5 x 5 square turned 45 degrees.
        let p = pose.rotate(std::f32::consts::FRAC_PI_4).translate(-2.5, -2.5);
        let corners = [p.apply([0.0, 0.0]), p.apply([5.0, 0.0]), p.apply([5.0, 5.0]), p.apply([0.0, 5.0])];
        batch.fill_quad(ctx.atlas, corners, c(color));
    }
}

/// `DrawContext.renderTexture`: `size` square, centred, tinted.
fn texture(batch: &mut HudBatch, pose: Affine, r: Rect, size: f32, color: u32) {
    let o = size / -2.0;
    let p = [pose.apply([o, o]), pose.apply([o + size, o]), pose.apply([o + size, o + size]), pose.apply([o, o + size])];
    batch.quad(p, [[r.u0, r.v0], [r.u1, r.v0], [r.u1, r.v1], [r.u0, r.v1]], c(color));
}

/// `DrawContext.renderLabel`: the text on a dark box, `y_offset` lines from the middle, with the
/// player's head before it.
fn label(ctx: &Ctx<'_, '_>, batch: &mut HudBatch, pose: Affine, text: &Text, y_offset: f32, head: Option<&str>, color: u32) {
    let extra = if head.is_some() { 10.0 } else { 0.0 };
    let (tw, th) = (ctx.atlas.font.width_text(text) + extra, 9.0f32);
    let pose = pose.translate(-tw * 0.5, -th * 0.5 + th * y_offset);
    let start = batch.vertices.len();
    // `fill(-2, -2, (int) width + 1, (int) height, SHADOW_BLACK)`.
    let right = (tw as i32 + 1) as f32;
    batch.fill(ctx.atlas, -2.0, -2.0, right + 2.0, th + 2.0, c(SHADOW_BLACK));
    batch.text_styled(ctx.atlas, text, extra, 0.0, c(color), false);
    pose.since(batch, start);
    if let Some(skin) = head {
        // `renderPlayerHead`: the face and the hat, 8 x 8, half a pixel up and left.
        let start = batch.vertices.len();
        let mut draws = Vec::new();
        horizon_client::tab_list::face(&mut draws, skin, 0, 0, 8, true, false, horizon_client::hud_layout::WHITE);
        ctx.game.native_gui_paint(batch, &draws);
        pose.translate(-0.5, -0.5).since(batch, start);
    }
}

