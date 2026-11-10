// Dynamic Lights in the game (not in the Cargo library): the light sources
// gathered from the game's own entity lists for Horizon's dynamic lights,
// the mod's row and page in Options > Mods, and its toggle key.
//
// Horizon gathers its dynamic lights once a frame in `gather_dynamic_lights`
// (game/dynamic_lights.rs); while the mod is on, it gathers them instead, and
// Horizon's renderer lights the world with them as it does its own.
use super::dynamiclights as dl;
use super::{Game, shield};
use glam::DVec3;
use horizon_client::blocks::Fluid;
use horizon_client::dynamic_light::{self, Source};
use horizon_client::mod_api::NativeMod;
use horizon_client::projectiles::ProjectileKind;
use horizon_client::text::Text;
use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};

pub(super) fn settings(game: &Game<'_>) -> dl::Settings {
    dl::settings(|key| game.native_setting(dl::ID, key))
}

/// The values kept: for the lights at once, and in the mod's settings.
pub(super) fn store(game: &Game<'_>, s: dl::Settings) {
    dl::set_settings(s);
    for (key, value) in s.stored() {
        game.native_set_setting(dl::ID, key, &value);
    }
}

pub(super) fn russian() -> bool {
    dl::russian(&horizon_client::lang::code())
}

/// The toggle key's presses since the last frame.
static TOGGLES: AtomicU32 = AtomicU32::new(0);

// ---- the mod's row and page ------------------------------------------------------------

/// The mod's row in Options > Mods, with its toggle key (unbound at first).
pub(super) fn list(mods: &mut Vec<NativeMod>) {
    let ru = russian();
    mods.push(NativeMod {
        id: dl::ID.into(),
        title: format!("{} {}", dl::NAME, dl::VERSION),
        description: dl::description(ru).into(),
        keys: vec![[dl::TOGGLE_KEY.into(), dl::key_label(ru).into(), dl::UNBOUND.into()]],
    });
}

/// The mod's page: Configure... opens the settings screen; the main switch.
pub(super) fn page(game: &Game<'_>, mod_id: &str, labels: &mut Vec<String>) {
    if mod_id == dl::ID {
        labels.extend(settings(game).page_labels(russian()));
    }
}

/// A click on the page: the first button, or Mod Menu's Configure, opens
/// the settings screen; the second flips the main switch.
pub(super) fn page_click(game: &Game<'_>, mod_id: &str, button: usize) {
    if mod_id != dl::ID {
        return;
    }
    if button == 0 || button == super::dynamiclights_screen::MODMENU_CONFIGURE {
        super::dynamiclights_screen::open(game, button != 0);
    } else if button == 1 {
        let mut s = settings(game);
        s.flip(dl::Opt::Enabled);
        store(game, s);
    }
}

// ---- the toggle key ---------------------------------------------------------------------

/// The key the player chose for turning the lights on and off.
pub(super) fn toggle_key(game: &Game<'_>) -> String {
    game.native_mod_key(dl::ID, dl::TOGGLE_KEY, dl::UNBOUND)
}

/// A key went down or up: the toggle key while playing.
pub(super) fn key(game: &Game<'_>, key: &str, pressed: bool) {
    if pressed && key != dl::UNBOUND && game.native_on(dl::ID) && game.native_playing() && toggle_key(game) == key {
        TOGGLES.fetch_add(1, Ordering::Relaxed);
    }
}

/// Each frame: the toggle key's presses flip the main switch, with a line
/// in the action bar.
pub(super) fn frame(game: &mut Game<'_>) {
    let presses = TOGGLES.swap(0, Ordering::Relaxed);
    if presses % 2 == 0 {
        return;
    }
    let mut s = settings(game);
    s.flip(dl::Opt::Enabled);
    store(game, s);
    let state = Text::translate(if s.enabled { "options.on" } else { "options.off" }, &[]);
    let line = Text::option_name_value(Text::literal(dl::Opt::Enabled.caption(russian())), state);
    game.native_client_action_bar(line, false);
}

// ---- the light sources ------------------------------------------------------------------

thread_local! {
    /// This frame's candidate sources, kept between frames.
    static SOURCES: RefCell<Vec<Source>> = const { RefCell::new(Vec::new()) };
}

/// `gather_dynamic_lights`: this frame's lights, the `max_sources` nearest
/// `camera` within `range`, as Horizon's shaders read them. False while the
/// mod is off in Options > Mods: Horizon's own extra gathers them then.
pub(super) fn gather(game: &mut Game<'_>, camera: DVec3, alpha: f64) -> bool {
    if !game.native_on(dl::ID) {
        return false;
    }
    let s = settings(game);
    let mut out = std::mem::take(&mut game.dynamic_lights);
    out.clear();
    if s.enabled {
        let reach = f64::from(s.range) * f64::from(s.range);
        SOURCES.with(|sources| {
            let mut sources = sources.borrow_mut();
            sources.clear();
            collect(game, &s, alpha, &mut |source| {
                if source.level > 0 && source.at.distance_squared(camera) <= reach {
                    sources.push(source);
                }
            });
            dynamic_light::select_iter(sources.drain(..), camera, &mut out);
        });
        out.truncate(usize::from(s.max_sources));
        for light in &mut out {
            light[3] = s.shown(light[3]);
        }
    }
    game.dynamic_lights = out;
    true
}

/// Inside the water of its block, as `Entity.isEyeInFluid` measures it.
fn underwater(game: &Game<'_>, at: DVec3) -> bool {
    let (x, y, z) = (at.x.floor() as i32, at.y.floor() as i32, at.z.floor() as i32);
    let facts = game.res.facts();
    let here = facts.get(game.world.block(x, y, z));
    if here.fluid != Fluid::Water {
        return false;
    }
    let above = facts.get(game.world.block(x, y + 1, z));
    let height = if above.fluid == Fluid::Water {
        1.0
    } else {
        f64::from(here.fluid_level) / 9.0
    };
    at.y <= f64::from(y) + height
}

/// Every light source of the kinds turned on, where the frame draws it.
fn collect(game: &Game<'_>, s: &dl::Settings, alpha: f64, push: &mut impl FnMut(Source)) {
    let lum = game.luminance();
    // An item's light where it is: its block's, unless water puts it out.
    let item = |id: &str, at: DVec3| -> u8 {
        let level = lum.of(id).max(dl::item_extra(id));
        if level > 0 && s.water && dl::fire_item(id) && underwater(game, at) {
            0
        } else {
            level
        }
    };
    let hands = |ids: [Option<&str>; 2], at: DVec3| ids.into_iter().flatten().map(|id| item(id, at)).max().unwrap_or(0);
    let burning = s.on(dl::Kind::Burning);
    let fire = |on_fire: bool| if burning && on_fire { dl::BURNING } else { 0 };
    let glow = |g: dl::Glow, at: DVec3| {
        if !s.on(g.kind()) || (s.water && g.fire() && underwater(game, at)) {
            0
        } else {
            g.level()
        }
    };

    // The player, at hand height (spectators give none).
    if let Some(p) = game.player.as_ref().filter(|p| !p.spectator) {
        let feet = game.previous_position.lerp(p.position, game.clock.camera_fraction());
        let at = DVec3::new(feet.x, feet.y + 1.2, feet.z);
        let held = if s.on(dl::Kind::OwnHands) {
            let stacks = [game.stack_in(game.selected), game.stack_in(shield::OFFHAND_SLOT)];
            hands(stacks.map(|st| st.filter(|st| st.count > 0).map(|st| &*st.item)), at)
        } else {
            0
        };
        push(Source { at, level: held.max(fire(p.fire_ticks > 0)) });
    }

    // Other players and mobs: their hands, fire and own glow.
    for m in game.mobs.iter().filter(|m| !m.removed) {
        let kind = m.kind.id.as_str();
        let player = kind == "minecraft:player";
        if player && m.extra.spectator {
            continue;
        }
        let feet = m.previous.lerp(m.position, alpha);
        let at = DVec3::new(feet.x, feet.y + f64::from(m.height()) * 0.6, feet.z);
        let mut level = fire(m.fire_ticks > 0);
        if s.on(if player { dl::Kind::Players } else { dl::Kind::Mobs }) {
            level = level.max(hands([m.extra.main_hand.as_deref(), m.extra.off_hand.as_deref()], at));
        }
        if s.on(dl::Kind::Creatures) {
            let dark = if kind == "minecraft:glow_squid" { horizon_client::mobs::aquatic::dark_ticks(m) } else { 0 };
            let own = dl::creature_light(kind, m.extra.size, dark);
            if own > 0 && !(s.water && dl::fire_creature(kind) && underwater(game, at)) {
                level = level.max(own);
            }
        }
        push(Source { at, level });
    }

    // Armor stands hold items as mobs do.
    for a in game.armor_stands.iter() {
        let feet = a.previous.lerp(a.position, alpha);
        let at = DVec3::new(feet.x, feet.y + if a.small { 0.6 } else { 1.2 }, feet.z);
        let mut level = fire(a.fire_ticks > 0);
        if s.on(dl::Kind::Mobs) {
            let [main, off, ..] = &a.equipment;
            level = level.max(hands([main.as_ref().map(|st| &*st.item), off.as_ref().map(|st| &*st.item)], at));
        }
        push(Source { at, level });
    }

    // Dropped items.
    let dropped = s.on(dl::Kind::Dropped);
    for d in game.dropped.iter().filter(|d| d.stack.count > 0) {
        let p = d.previous.lerp(d.position, alpha);
        let at = DVec3::new(p.x, p.y + 0.25, p.z);
        let own = if dropped { item(&d.stack.item, at) } else { 0 };
        push(Source { at, level: own.max(fire(d.fire_ticks > 0)) });
    }

    // Projectiles: fireballs, skulls, bullets and spectral arrows glow; any
    // on fire burns.
    for p in game.creatures.projectiles.iter() {
        let c = p.previous.lerp(p.position, alpha);
        let (kind, lift) = match &p.kind {
            ProjectileKind::LargeFireball { .. } => (Some(dl::Glow::LargeFireball), 0.5),
            ProjectileKind::SmallFireball => (Some(dl::Glow::SmallFireball), 0.15),
            ProjectileKind::WitherSkull { .. } => (Some(dl::Glow::WitherSkull), 0.15),
            ProjectileKind::ShulkerBullet { .. } => (Some(dl::Glow::ShulkerBullet), 0.15),
            ProjectileKind::Arrow { .. } if p.is_spectral_arrow() => (Some(dl::Glow::SpectralArrow), 0.25),
            _ => (None, 0.25),
        };
        let at = DVec3::new(c.x, c.y + lift, c.z);
        let on_fire = p.fire_ticks > 0 || p.arrow.as_ref().is_some_and(|a| a.on_fire());
        let own = kind.map_or(0, |g| glow(g, at));
        push(Source { at, level: own.max(fire(on_fire)) });
    }
    for f in game.dragon_fireballs.iter() {
        let c = f.previous.lerp(f.position, alpha);
        let at = DVec3::new(c.x, c.y + 0.5, c.z);
        push(Source { at, level: glow(dl::Glow::DragonFireball, at) });
    }
    for r in game.rockets.iter() {
        let c = r.previous.lerp(r.position, alpha);
        let at = DVec3::new(c.x, c.y + 0.125, c.z);
        push(Source { at, level: glow(dl::Glow::Rocket, at) });
    }
    // A trident only burns.
    for t in game.tridents.iter().filter(|t| t.fire_ticks > 0) {
        let c = t.previous.lerp(t.position, alpha);
        push(Source { at: DVec3::new(c.x, c.y + 0.25, c.z), level: fire(true) });
    }

    // Primed TNT, lit TNT minecarts and running furnace minecarts.
    for t in game.tnts.iter() {
        let c = t.previous.lerp(t.position, alpha);
        let at = DVec3::new(c.x, c.y + 0.5, c.z);
        push(Source { at, level: glow(dl::Glow::PrimedTnt, at) });
    }
    for c in game.carts.iter() {
        let g = match c.kind {
            super::minecarts::CartKind::Furnace if c.fuel > 0 => dl::Glow::FurnaceMinecart,
            super::minecarts::CartKind::Tnt if c.fuse >= 0 => dl::Glow::TntMinecart,
            _ => continue,
        };
        let p = c.previous.lerp(c.position, alpha);
        let at = DVec3::new(p.x, p.y + 0.5, p.z);
        push(Source { at, level: glow(g, at) });
    }
}
