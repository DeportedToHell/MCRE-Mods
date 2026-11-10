// The emote previews (`PlayerPreview`, the wheel's tiles and the lists' mannequin): a mannequin
// with the player's skin playing an emote, drawn in a box of the screen as the inventory draws
// the player (`InventoryScreen.extractEntityInInventoryFollowsMouse`), turned toward a point.
// Each preview has a model id of its own, whose pose this frame `emotecraft_pose` gives.
//
// This Source Code Form is a port of Emotecraft (https://github.com/KosmX/emotes), GPL-3.0.

use super::Game;
use emotecraft_engine::anim::{Animation, LoopType};
use emotecraft_engine::controller::Controller;
use horizon_client::hud::HudBatch;
use std::sync::Arc;

/// The first preview model id (well past any entity's).
pub(super) const BASE_ID: u32 = 0xEC00_0000;

/// A mannequin (`PlayerPreview` / `UnsafeMannequin`) and its fade (`animTime`, `alpha`).
pub(super) struct Mannequin {
    pub ctrl: Controller,
    pub anim_time: f32,
    pub alpha: f32,
}

impl Default for Mannequin {
    fn default() -> Mannequin {
        Mannequin { ctrl: Controller::new(), anim_time: 1.0, alpha: 0.0 }
    }
}

impl Mannequin {
    /// `playAnimation`: false if `check` and it plays that emote already.
    pub fn play(&mut self, anim: Option<&Arc<Animation>>, loop_type: Option<LoopType>, check: bool, tick: f32) -> bool {
        if check
            && let Some(a) = anim
            && self.ctrl.current().is_some_and(|c| **c == **a)
        {
            return false;
        }
        self.ctrl.stop();
        if let Some(a) = anim {
            self.ctrl.play(a.clone(), loop_type, tick);
        }
        true
    }
    pub fn pause(&mut self, paused: bool) {
        if paused {
            self.ctrl.pause();
        } else {
            self.ctrl.unpause();
        }
    }
    pub fn playing(&self) -> bool {
        self.ctrl.is_active()
    }
    /// `PlayerPreview.tick`.
    pub fn tick(&mut self, visible: bool) {
        if visible && self.playing() {
            self.anim_time = 0.0;
            self.alpha = 1.0;
            self.ctrl.tick();
        } else {
            self.anim_time = (self.anim_time + 0.1).min(1.0);
        }
    }
    /// After a draw: the fade's alpha (`EASE_OUT_QUART`).
    pub fn after_draw(&mut self) {
        if self.anim_time > 0.0 {
            self.alpha = 1.0 - emotecraft_engine::easing::Easing::OutQuart.apply_simple(0.0, 1.0, self.anim_time);
        }
    }
}

/// The mannequin's model: the player's own skin and cape, standing, holding nothing.
fn model(game: &Game<'_>, id: u32) -> Option<horizon_client::mobs::Mob> {
    let kind = game.res.player_type().clone()?;
    let mut m = horizon_client::mobs::Mob::new(id, kind, Default::default(), 0.0);
    if let Some(own) = game.player_model.as_ref() {
        m.skin = own.skin.clone();
        m.slim = own.slim;
        m.cape = own.cape.clone();
    }
    Some(m)
}

/// Draws mannequin `id` in the box `[x, y, width, height]` at `size` pixels a block, turned
/// toward `look` (`extractEntityInInventoryFollowsMouse`).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch, id: u32, rect: [f32; 4], size: f32, offset_y: f32, look: (f32, f32)) {
    if size <= 0.0 {
        return;
    }
    let [x0, y0, x1, y1] = [rect[0], rect[1], rect[0] + rect[2], rect[1] + rect[3]];
    let Some(mut m) = model(game, id) else { return };
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let x_angle = ((cx - look.0) / 40.0).atan();
    let y_angle = ((cy - look.1) / 40.0).atan();
    m.extra.model_scale = 0.0;
    m.body_yaw = 180.0 + x_angle * 20.0;
    m.previous_body_yaw = m.body_yaw;
    m.head_yaw = m.body_yaw + x_angle * 20.0;
    m.previous_head_yaw = m.head_yaw;
    m.head_pitch = -y_angle * 20.0;
    m.previous_head_pitch = m.head_pitch;
    m.previous = m.position;
    let camera = glam::DVec3::new(m.position.x, m.position.y, m.position.z);
    let atlas = &game.gui.atlas;
    let lookup = |name: &str| atlas.sprites.get(name).map(|r| [r.u0, r.v0, r.u1, r.v1]);
    let mut verts = Vec::new();
    let light = 240 | 240 << 8;
    horizon_client::mobs::cem_anim::in_gui(|| {
        horizon_client::mobs::render::render_with(&m, 1.0, camera, &lookup, light, game.res.mob_data(), &[], &mut verts);
    });
    game.player_cape_geometry(&m, None, camera, 1.0, light, &mut verts);
    let gui = glam::Mat4::from_translation(glam::Vec3::new(cx, cy, 0.0))
        * glam::Mat4::from_scale(glam::Vec3::new(size, size, -size))
        * glam::Mat4::from_translation(glam::Vec3::new(0.0, 1.8 / 2.0 + offset_y, 0.0))
        * glam::Mat4::from_rotation_z(std::f32::consts::PI)
        * glam::Mat4::from_rotation_x((y_angle * 20.0).to_radians());
    let quads = super::perspective::player_preview_quads_no_cull(verts, gui, None, &atlas.sprites, 0..0);
    let start = batch.vertices.len();
    for (_, p, uv, color) in quads {
        batch.quad_3d(p, uv, color);
    }
    batch.depth_since(start, [x0, y0, x1, y1]);
}
