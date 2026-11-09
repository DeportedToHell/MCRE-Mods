// First-person Model's edges in the game: each place the original hooks the
// Java game, here on Horizon's functions. Spliced into game.rs as
// `firstpersonmodel_glue`.
use super::firstpersonmodel as fpm;
use super::Game;
use fpm::{Hands, Settings};
use horizon_client::mobs::anim::firstpersonmodel_pose as pose;
use horizon_client::mod_api::NativeMod;
use horizon_client::pose::Pose;
use std::cell::Cell;
use std::time::{Duration, Instant};

thread_local! {
    /// The frame draws the own body in first person (`shouldApplyThirdPerson`
    /// in first person, decided in `frame`).
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    /// `player_model_geometry` is drawing the own body moved behind the camera.
    static DRAWING: Cell<bool> = const { Cell::new(false) };
}

/// AvatarRenderer's layers (`getRenderLayers`), as the "Show Layers" tab
/// names them: the class names `hiddenLayers` keeps.
pub const LAYERS: [&str; 10] = [
    "net.minecraft.client.renderer.entity.layers.HumanoidArmorLayer",
    "net.minecraft.client.renderer.entity.layers.PlayerItemInHandLayer",
    "net.minecraft.client.renderer.entity.layers.ArrowLayer",
    "net.minecraft.client.renderer.entity.layers.Deadmau5EarsLayer",
    "net.minecraft.client.renderer.entity.layers.CapeLayer",
    "net.minecraft.client.renderer.entity.layers.CustomHeadLayer",
    "net.minecraft.client.renderer.entity.layers.WingsLayer",
    "net.minecraft.client.renderer.entity.layers.ParrotOnShoulderLayer",
    "net.minecraft.client.renderer.entity.layers.SpinAttackEffectLayer",
    "net.minecraft.client.renderer.entity.layers.BeeStingerLayer",
];

/// One of the layers above.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Armor,
    HeldItems,
    Arrows,
    Ears,
    Cape,
    CustomHead,
    Wings,
    Parrots,
    SpinAttack,
    BeeStingers,
}

impl Layer {
    fn class(self) -> &'static str {
        LAYERS[self as usize]
    }
}

fn item(stack: &Option<horizon_client::items::ItemStack>) -> Option<&str> {
    stack.as_ref().map(|s| &*s.item)
}

/// The local player's hands as `LogicHandler` reads them.
fn hands<'g>(game: &'g Game<'_>) -> Hands<'g> {
    Hands {
        main: game.inventory.get(game.selected).and_then(item),
        off: item(&game.offhand),
        swimming: game.player.as_ref().is_some_and(|p| p.swimming),
        pitch: game.pitch,
    }
}

/// `LivingEntity.isVisuallySwimming`.
fn visually_swimming(game: &Game<'_>) -> bool {
    game.player
        .as_ref()
        .is_some_and(|p| p.pose == Pose::Swimming || (!p.fall_flying && p.pose == Pose::FallFlying))
}

/// `LogicHandler.shouldApplyThirdPerson(false)` with its default activation
/// handler: the toggle is on and nothing makes the mod step aside.
fn should_apply(game: &Game<'_>, settings: &Settings, now: Instant) -> bool {
    let Some(p) = game.player.as_ref() else {
        return false;
    };
    let mut s = fpm::state();
    if !s.enabled {
        return false;
    }
    let swim = game.player_model.as_ref().map_or(0.0, |m| m.extra.swim.amount(1.0));
    if game.sleeping.is_some()
        || p.auto_spin_attack_ticks > 0
        || p.fall_flying
        || (swim != 0.0 && !visually_swimming(game))
    {
        s.timeout = Some(now + Duration::from_millis(100));
        return false;
    }
    // FIXME of the original: landing with an elytra flickers for a frame or so.
    if s.timeout.is_some_and(|t| now < t) {
        return false;
    }
    drop(s);
    let h = hands(game);
    if settings.disable_item(h.main, h.off) {
        return false;
    }
    !game.scoping()
}

// ---- the hooks -------------------------------------------------------------

/// Options > Mods: the mod's row with its key.
pub(super) fn list(game: &Game<'_>, mods: &mut Vec<NativeMod>) {
    let ru = russian(game);
    let (name, default) = fpm::TOGGLE_KEY;
    mods.push(NativeMod {
        id: fpm::ID.into(),
        title: format!("{} {}", fpm::NAME, fpm::VERSION),
        description: if ru {
            "Позволяет видеть модель персонажа от третьего лица в режиме первого лица".into()
        } else {
            "Enables the third-person Model in first-person".into()
        },
        keys: vec![[
            name.into(),
            if ru { "Переключить FirstPerson" } else { "Toggle Firstperson" }.into(),
            default.into(),
        ]],
    });
}

pub(super) fn russian(game: &Game<'_>) -> bool {
    game.settings.language.starts_with("ru")
}

/// The mod's page: its config screen's button (Mod Menu's "Configure...").
pub(super) fn page(mod_id: &str, labels: &mut Vec<String>) {
    if mod_id == fpm::ID {
        labels.push(super::firstpersonmodel_screen::configure_label().to_string());
    }
}

pub(super) fn page_click(game: &Game<'_>, mod_id: &str, button: usize) {
    if mod_id == fpm::ID && (button == super::firstpersonmodel_screen::MODMENU_CONFIGURE || button == 0) {
        super::firstpersonmodel_screen::open(game, button != 0);
    }
}

/// Once a frame, before it is drawn: the settings at the first, whether the
/// own body shows, the swimming body turned with the head, and the screen.
pub(super) fn frame(game: &mut Game<'_>) {
    if !fpm::state().loaded {
        fpm::load();
    }
    let settings = fpm::settings();
    let active = game.native_on(fpm::ID)
        && game.camera_type.first_person()
        && game.camera_mob().is_none()
        && game.player.as_ref().is_some_and(|p| !p.health.dead_or_dying())
        && should_apply(game, &settings, Instant::now());
    ACTIVE.with(|a| a.set(active));
    arm_pitch(game, &settings);
    // LogicHandler.updatePositionOffset: crawling or swimming, the body
    // faces where the head does.
    if active && visually_swimming(game)
        && let Some(m) = game.player_model.as_mut()
    {
        m.body_yaw = m.head_yaw;
    }
    super::firstpersonmodel_screen::frame(game);
}

/// The toggle key (`KeybindHolder`, read each client tick while playing).
pub(super) fn key(game: &Game<'_>, key: &str, pressed: bool) {
    let (name, default) = fpm::TOGGLE_KEY;
    if !game.native_on(fpm::ID) || game.native_mod_key(fpm::ID, name, default) != key {
        return;
    }
    let mut s = fpm::state();
    if !pressed {
        s.held = false;
    } else if game.native_playing() && !s.held {
        s.held = true;
        s.enabled = !s.enabled;
    }
}

/// `own_body_in_view`: also while the mod draws the body in first person, so
/// the game draws it in the world view (not only into a shader's shadow), and
/// its fire and companions with it, as the original's extracted entity has.
pub(super) fn own_body_in_view(game: &Game<'_>, value: bool) -> bool {
    value || (ACTIVE.with(Cell::get) && game.camera_type.first_person())
}

/// `LogicHandler.updatePositionOffset`: how far the body moves from the
/// camera, along its yaw.
fn offset(game: &Game<'_>, settings: &Settings, alpha: f32) -> glam::DVec3 {
    let (Some(p), Some(m)) = (game.player.as_ref(), game.player_model.as_ref()) else {
        return glam::DVec3::ZERO;
    };
    if game.sleeping.is_some() {
        return glam::DVec3::ZERO;
    }
    let mut real_yaw = fpm::rot_lerp(alpha, m.previous_body_yaw, m.body_yaw);
    let pitch_o = m.previous_head_pitch;
    let swimming = visually_swimming(game);
    let body_offset = if swimming {
        if pitch_o > 0.0 { fpm::SWIM_UP_BODY_OFFSET } else { fpm::SWIM_DOWN_BODY_OFFSET }
    } else if p.pose == Pose::Crouching {
        fpm::SNEAK_BODY_OFFSET + settings.sneak_x_offset as f32 / 100.0
    } else if let Some(vehicle) = game.riding {
        // A living vehicle turns the rider's body with its own; boats,
        // minecarts and the rest keep the rider's.
        if let Some(v) = game.mobs.iter().find(|v| v.id == vehicle) {
            real_yaw = fpm::calculate_body_rot(fpm::rot_lerp(alpha, v.previous_body_yaw, v.body_yaw), game.yaw);
        }
        fpm::IN_VEHICLE_BODY_OFFSET + settings.sit_x_offset as f32 / 100.0
    } else {
        0.25 + settings.x_offset as f32 / 100.0
    };
    let body_offset = f64::from(body_offset);
    let yaw = f64::from(real_yaw).to_radians();
    let x = body_offset * yaw.sin();
    let z = -body_offset * yaw.cos();
    let mut y = 0.0;
    if swimming {
        let pitch = f64::from(pitch_o).to_radians();
        if pitch_o > 0.0 && p.is_under_water() {
            y += f64::from(0.6f32) * pitch.sin();
        } else {
            y += f64::from(0.01f32) * -pitch.sin();
        }
    }
    glam::DVec3::new(x, y, z)
}

/// `player_model_geometry` (HEAD): in first person the original extracts the
/// camera entity moved by the offset, its head hidden; here the same draw
/// again from a camera moved the other way, with the body's flags set for
/// every pose of the model. Break when it drew.
pub(super) fn player_geometry(
    game: &Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) -> bool {
    if DRAWING.with(Cell::get) || !ACTIVE.with(Cell::get) || !game.camera_type.first_person() {
        return false;
    }
    let settings = fpm::settings();
    let h = hands(game);
    let main_map = h.main == Some("minecraft:filled_map");
    let flags = settings.body_flags(h, main_map);
    let offset = offset(game, &settings, alpha as f32);
    DRAWING.with(|d| d.set(true));
    pose::set(Some(flags));
    game.player_model_geometry(camera - offset, alpha, out);
    pose::set(None);
    DRAWING.with(|d| d.set(false));
    true
}

/// A layer of the own body in first person that stays undrawn: the hidden
/// layers of "Show Layers", and what the original's layer mixins skip.
pub(super) fn skip_layer(game: &Game<'_>, layer: Layer) -> bool {
    if !DRAWING.with(Cell::get) {
        return false;
    }
    let s = fpm::state();
    let settings = &s.settings;
    if settings.layer_hidden(layer.class()) {
        return true;
    }
    let h = hands(game);
    match layer {
        // CustomHeadLayerMixin, and the head slot read as empty (PlayerMixin).
        Layer::CustomHead => true,
        // PlayerMixin empties both hands; HeldItemFeatureRendererMixin skips
        // the layer with both arms hidden while not looking down.
        Layer::HeldItems => {
            if settings.hide_arms_and_items(h) {
                return true;
            }
            let f = settings.body_flags(h, h.main == Some("minecraft:filled_map"));
            f.hide_left_arm && f.hide_right_arm && !settings.looking_down(h)
        }
        // ElytraLayerMixin.
        Layer::Wings => visually_swimming(game),
        // `showCape = false` while swimming.
        Layer::Cape => h.swimming,
        // StuckInBodyLayerMixin.
        Layer::Arrows | Layer::BeeStingers => !settings.render_stuck_features,
        _ => false,
    }
}

/// `stuck_in_body_geometry` (HEAD): the arrows and the bee stingers are two
/// layers, each hidden on its own. Break when it drew (or skipped) them.
pub(super) fn stuck_in_body(
    game: &Game<'_>,
    m: &horizon_client::mobs::Mob,
    id: i32,
    camera: glam::DVec3,
    partial: f32,
    light: u32,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) -> bool {
    if !DRAWING.with(Cell::get) {
        return false;
    }
    let arrows = !skip_layer(game, Layer::Arrows);
    let stingers = !skip_layer(game, Layer::BeeStingers);
    if arrows && stingers {
        return false;
    }
    let sprites = &game.gui.atlas.sprites;
    let layers = [
        (arrows, m.extra.arrow_count, false, game.arrow_model.as_ref(), "entity/projectiles/arrow"),
        (stingers, m.extra.stinger_count, true, game.bee_stinger_model.as_ref(), "entity/bee/bee_stinger"),
    ];
    for (shown, count, on_surface, model, texture) in layers {
        let (true, true, Some(model), Some(rect)) = (shown, count > 0, model, sprites.get(texture)) else {
            continue;
        };
        horizon_client::mobs::render::stuck_in_body(
            m,
            id,
            partial,
            camera,
            game.res.mob_data(),
            count,
            on_surface,
            model,
            [rect.u0, rect.v0, rect.u1, rect.v1],
            light,
            out,
        );
    }
    true
}

/// The vanilla first-person hands (`HeldItemRendererMixin.renderFirstPersonItem`):
/// true cancels this hand.
pub(super) fn cancel_hand(game: &Game<'_>) -> bool {
    if !game.native_on(fpm::ID) || !fpm::state().enabled {
        return false;
    }
    let settings = fpm::settings();
    let h = hands(game);
    if !settings.show_vanilla_hands(h) {
        return true;
    }
    if game.player.is_none() {
        return false;
    }
    // While actively swimming, no vanilla hands.
    if settings.vanilla_hands_skip_swimming && h.swimming {
        return true;
    }
    // An item held too low hides.
    settings.dynamic_hands_enabled(h) && game.pitch > 35.0
}

/// "Always Both Hands" (`ALL_DOUBLE`, HeldItemRendererMixin): the off hand,
/// empty, draws its arm as the main hand's empty arm draws
/// (`renderPlayerArm` on the other arm), unless the main hand holds a filled
/// map, the player is scoping or invisible. Added to `out`, the hand pass's
/// vertices, as `hand_geometry` would add it.
pub(super) fn off_hand_arm(game: &Game<'_>, partial: f32, out: &mut Vec<horizon_client::mesher::Vertex>) {
    use horizon_client::first_person::Hand;
    if !game.native_on(fpm::ID) || !fpm::state().enabled || game.player.is_none() {
        return;
    }
    let settings = fpm::settings();
    let h = hands(game);
    if settings.vanilla_hands_mode != fpm::VanillaHands::AllDouble
        || h.main == Some("minecraft:filled_map")
        || game.scoping()
        || game.own_invisible()
        || cancel_hand(game)
    {
        return;
    }
    // hand_geometry's own conditions: the hands drawn at all, the off hand
    // shown and empty.
    if game.studio_replay.is_some()
        || !game.ui.hands_drawn(
            game.spectator(),
            game.sleeping.is_some(),
            game.camera_type.first_person(),
            game.player.as_ref().is_some_and(|p| !p.health.dead_or_dying()),
            game.screen == super::Screen::Dead,
        )
        || !game.hands_to_render()[1]
        || game.visible_hand_items()[1].is_some()
    {
        return;
    }
    let attack = if game.swing.attack_hand() == Hand::Off { game.swing.animation(partial) } else { 0.0 };
    let heights = game.heights.interpolated(partial);
    let inverse_height = horizon_client::first_person::equip_displacement(heights[1], 1.0);
    let to_world = game.sway.to_world(partial, &game.view);
    let eye = game.eye_position().unwrap_or_default();
    let light = game.world.light(eye[0].floor() as i32, eye[1].floor() as i32, eye[2].floor() as i32);
    let packed = (u32::from(light >> 4) * 16) | ((u32::from(light & 15) * 16) << 8);
    let left = !game.settings.main_hand_left;
    let m = to_world * horizon_client::first_person::player_arm_pose(left, inverse_height, attack);
    game.first_person_arm(left, m, packed, out);
}

/// Once a frame: the vanilla hands follow the view while the mod is on, the
/// hands show and Dynamic Mode applies (`modifyInverseArmHeight`).
fn arm_pitch(game: &Game<'_>, settings: &Settings) {
    let on = game.native_on(fpm::ID) && fpm::state().enabled && game.player.is_some();
    let h = hands(game);
    let follow = on && settings.show_vanilla_hands(h) && settings.dynamic_hands_enabled(h);
    pose::set_arm_pitch(follow.then_some(game.pitch));
}

/// `FishingBobberRendererMixin.doCorrect`: the line leaves the body's hand
/// (the third-person position) when the model's arms show.
fn fishing_correct(game: &Game<'_>) -> bool {
    game.native_on(fpm::ID)
        && fpm::state().enabled
        && game.camera_type.first_person()
        && !fpm::settings().hide_arms_and_items(hands(game))
}

/// `FishingHookRenderer.getPlayerHandPos` while corrected: its
/// `getCameraType()` read as third person, so the hand beside the body
/// (`rod_hand_position`'s other branch), then `getPlayerHandPosOffset`'s
/// body offset. None leaves the game's own (first person: on the near plane).
/// `Some(None)`: no hand.
pub(super) fn fishing_hand(game: &Game<'_>, alpha: f64) -> Option<Option<glam::DVec3>> {
    if !fishing_correct(game) {
        return None;
    }
    let (Some(p), Some(model)) = (game.player.as_ref(), game.player_model.as_ref()) else {
        return Some(None);
    };
    let right_arm = game.main_hand_rod() != game.settings.main_hand_left;
    let invert: f32 = if right_arm { 1.0 } else { -1.0 };
    let feet = game.previous_position.lerp(p.position, alpha);
    let eye = glam::DVec3::new(feet.x, feet.y + f64::from(p.dimensions.eye_height), feet.z);
    let body = horizon_client::camera::lerp(alpha as f32, model.previous_body_yaw, model.body_yaw);
    let angle = f64::from(body * (std::f64::consts::PI / 180.0) as f32);
    let (sin, cos) = (
        f64::from(horizon_core::geometry::trig::sin(angle)),
        f64::from(horizon_core::geometry::trig::cos(angle)),
    );
    let right = f64::from(invert) * 0.35;
    let crouch = if p.pose == Pose::Crouching { -0.1875 } else { 0.0 };
    let hand = eye + glam::DVec3::new(-cos * right - sin * 0.8, crouch - 0.45, -sin * right + cos * 0.8);
    let offset = if ACTIVE.with(Cell::get) {
        offset(game, &fpm::settings(), alpha as f32)
    } else {
        glam::DVec3::ZERO
    };
    Some(Some(hand + offset))
}

/// HumanoidArmorLayer hidden on the own body in first person ("Show Layers").
pub(super) fn armor_hidden() -> bool {
    DRAWING.with(Cell::get) && fpm::state().settings.layer_hidden(Layer::Armor.class())
}
