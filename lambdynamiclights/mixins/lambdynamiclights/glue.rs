// LambDynamicLights' game side (`game::lambdynamiclights_glue`): the light sources
// of the world around the camera, by the original's rules (`runtime.rs`,
// `game::lambdynamiclights`), handed to Horizon's dynamic light shader in place
// of Horizon's own sources (`gather_dynamic_lights`), the client tick that
// refreshes them in the Fast modes, and the settings page and command.
//
// LambDynamicLights is by LambdAurora, under the Lambda License (LICENSE).

use super::Game;
use super::lambdynamiclights as ldl;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::NativeMod;
use std::cell::RefCell;
use std::collections::HashMap;

/// A light source this frame: where it is and its level.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Lit {
    at: [f64; 3],
    level: u8,
}

/// The kinds of entity a source's key names (with its entity id).
const PLAYER: u8 = 0;
const MOB: u8 = 1;
const ITEM: u8 = 2;
const SHOT: u8 = 3;
const DRAGON_FIREBALL: u8 = 4;
const TNT: u8 = 5;
const FRAME: u8 = 6;

/// One game's state. Splitscreen players are `Game`s of their own on the
/// window's thread, so each keeps its own, by its player's entity id.
#[derive(Default)]
struct PerGame {
    tick: u64,
    /// Fast modes: each source as it was last refreshed, by kind and id.
    held: HashMap<(u8, u32), Lit>,
    /// Active beacons' beams, from their base to their top (read each tick).
    beacons: Vec<([f64; 3], [f64; 3])>,
    /// `PrimedTnt.startFuseTimer`: each lit TNT's fuse when first seen.
    tnt_start: HashMap<u32, i32>,
}

#[derive(Default)]
struct State {
    /// The options, read once from the mod's settings folder.
    config: Option<ldl::Config>,
    games: Vec<(i32, PerGame)>,
}

thread_local! {
    // The window's thread runs every hook this mod uses.
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The options, read the first time they are needed.
fn config(game: &Game<'_>) -> ldl::Config {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.config
            .get_or_insert_with(|| ldl::Config::load(|key| game.native_setting(ldl::ID, key)))
            .clone()
    })
}

/// Keeps `config` and writes the keys that changed.
fn store(game: &Game<'_>, config: ldl::Config, changed: &[&str]) {
    for key in changed {
        game.native_set_setting(ldl::ID, key, &config.value(key));
    }
    STATE.with(|s| s.borrow_mut().config = Some(config));
}

/// This game's player, by the entity id its server gave it.
fn me(game: &Game<'_>) -> i32 {
    game.remote.as_ref().map_or(-1, |r| r.entity_id)
}

/// `f` on game `owner`'s state, made when it has none yet.
fn with_game<R>(owner: i32, f: impl FnOnce(&mut PerGame) -> R) -> R {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let i = match s.games.iter().position(|(o, _)| *o == owner) {
            Some(i) => i,
            None => {
                s.games.push((owner, PerGame::default()));
                s.games.len() - 1
            }
        };
        f(&mut s.games[i].1)
    })
}

fn forget(owner: i32) {
    STATE.with(|s| s.borrow_mut().games.retain(|(o, _)| *o != owner));
}

/// Russian labels when the game is in Russian.
fn ru() -> bool {
    horizon_client::lang::code().starts_with("ru")
}

fn lerp(a: &horizon_core::geometry::Vec3, b: &horizon_core::geometry::Vec3, t: f64) -> [f64; 3] {
    [
        a.x + (b.x - a.x) * t,
        a.y + (b.y - a.y) * t,
        a.z + (b.z - a.z) * t,
    ]
}

/// An item's light (`ItemLightSources.getLuminance`), its block's being
/// Horizon's table of block light by item.
fn item_light(game: &Game<'_>, cfg: &ldl::Config, item: &str, submerged: bool) -> u8 {
    ldl::item_luminance(cfg, item, submerged, game.luminance().of(item))
}

/// What a mob's own light depends on.
fn species_light(game: &Game<'_>, m: &horizon_client::mobs::Mob) -> ldl::MobLight {
    match m.kind.id.as_str() {
        "minecraft:allay" => ldl::MobLight::Allay,
        "minecraft:blaze" => {
            // `Entity.isInWaterOrRain`.
            let at = horizon_core::position::BlockPos::new(
                m.position.x.floor() as i32,
                m.position.y.floor() as i32,
                m.position.z.floor() as i32,
            );
            ldl::MobLight::Blaze {
                wet: m.fluids.in_water || game.raining_at(at),
            }
        }
        // `GlowSquid.getDarkTicksRemaining` (`aquatic::dark_ticks`).
        "minecraft:glow_squid" => ldl::MobLight::GlowSquid {
            dark_ticks: m.extra.counter,
        },
        // `AbstractCubeMob.squish` (`cube.rs`'s first animation value).
        "minecraft:magma_cube" => ldl::MobLight::MagmaCube {
            squish: m.anim.v[0],
        },
        "minecraft:creeper" => ldl::MobLight::Creeper {
            swelling: ldl::creeper_swelling(m.previous_swell, m.max_swell),
        },
        "minecraft:enderman" => ldl::MobLight::Enderman {
            carried: m
                .extra
                .carried
                .map_or(0, |state| game.res.facts().get(state).emission),
        },
        _ => ldl::MobLight::None,
    }
}

/// A guardian laser's target: its centre, half its height up.
fn beam_target(
    game: &Game<'_>,
    who: Option<horizon_client::mobs::Who>,
    alpha: f64,
) -> Option<[f64; 3]> {
    match who? {
        horizon_client::mobs::Who::Player => {
            let p = game.player.as_ref()?;
            let mut at = lerp(&game.previous_position, &p.position, alpha);
            at[1] += f64::from(p.dimensions.height) * 0.5;
            Some(at)
        }
        horizon_client::mobs::Who::Mob(id) => {
            let m = game.mobs.iter().find(|m| m.id == id)?;
            let mut at = lerp(&m.previous, &m.position, alpha);
            at[1] += f64::from(m.height()) * 0.5;
            Some(at)
        }
    }
}

/// A projectile's entity type, for the projectiles that may give light.
fn shot_type(s: &horizon_client::projectiles::Projectile) -> Option<&'static str> {
    use horizon_client::projectiles::ProjectileKind as K;
    Some(match &s.kind {
        K::Arrow { .. } => {
            if s.arrow.as_ref().is_some_and(|a| a.spectral) {
                "minecraft:spectral_arrow"
            } else {
                "minecraft:arrow"
            }
        }
        K::Trident => "minecraft:trident",
        K::Snowball => "minecraft:snowball",
        K::Egg => "minecraft:egg",
        K::EnderPearl => "minecraft:ender_pearl",
        K::SmallFireball => "minecraft:small_fireball",
        K::LargeFireball { .. } => "minecraft:fireball",
        K::WitherSkull { .. } => "minecraft:wither_skull",
        K::Potion(_, lingering) => {
            if *lingering {
                "minecraft:lingering_potion"
            } else {
                "minecraft:splash_potion"
            }
        }
        K::ExperienceBottle => "minecraft:experience_bottle",
        _ => return None,
    })
}

/// Every entity light source (`dynamicLightTick` of each), positions at
/// `alpha` between ticks, and the guardians' lasers as lines.
fn entity_lights(
    game: &Game<'_>,
    cfg: &ldl::Config,
    alpha: f64,
    owner: i32,
    tnt_start: &mut HashMap<u32, i32>,
    out: &mut Vec<((u8, u32), Lit)>,
    lines: &mut Vec<([f64; 3], [f64; 3], u8)>,
) {
    // This client's player (`PlayerEntityMixin`), at its eyes.
    if let Some(p) = game.player.as_ref()
        && !p.spectator
        && cfg.can_light_up("minecraft:player", true)
    {
        let eye = f64::from(p.dimensions.eye_height);
        let submerged = game.eye_in_water(p.position.x, p.position.y + eye, p.position.z);
        let hands = [
            game.stack_in(game.selected),
            game.stack_in(super::shield::OFFHAND_SLOT),
        ];
        let items = hands
            .into_iter()
            .flatten()
            .chain(game.armor.iter().flatten())
            .map(|s| item_light(game, cfg, &s.item, submerged))
            .max()
            .unwrap_or(0);
        let glowing = game.potions.effects.get("glowing").is_some();
        let level = ldl::living(cfg, p.fire_ticks > 0, glowing, items);
        if level > 0 {
            let mut at = lerp(
                &game.previous_position,
                &p.position,
                game.clock.camera_fraction(),
            );
            at[1] += cfg.reach.height(eye);
            out.push(((PLAYER, owner as u32), Lit { at, level }));
        }
    }
    // Mobs and the other players (`LivingEntityMixin`): equipment, fire, the
    // glowing effect, and their species' own light.
    for m in game.mobs.iter() {
        let id = m.kind.id.as_str();
        if m.removed || !cfg.can_light_up(id, false) {
            continue;
        }
        if id == "minecraft:player" && m.extra.spectator {
            continue;
        }
        let guardian = matches!(id, "minecraft:guardian" | "minecraft:elder_guardian");
        if guardian
            && cfg.guardian_laser
            && let Some((time, scale)) = horizon_client::mobs::guardian::beam(m, 0.0)
            && let Some(target) = beam_target(game, m.ai.target, alpha)
        {
            let start = lerp(&m.previous, &m.position, alpha);
            lines.push((start, target, ldl::guardian_laser(time, scale)));
        }
        let submerged = m.fluids.eye_in_water;
        let items = [
            &m.extra.main_hand,
            &m.extra.off_hand,
            &m.extra.head,
            &m.extra.chest,
            &m.extra.legs,
            &m.extra.feet,
            &m.extra.body,
        ]
        .into_iter()
        .flatten()
        .map(|item| item_light(game, cfg, item, submerged))
        .max()
        .unwrap_or(0);
        let level = ldl::living(cfg, m.fire_ticks > 0, m.glowing, items)
            .max(ldl::mob_luminance(cfg, species_light(game, m)));
        if level > 0 {
            let mut at = lerp(&m.previous, &m.position, alpha);
            at[1] += cfg.reach.height(m.eye_height());
            out.push(((MOB, m.id), Lit { at, level }));
        }
    }
    // Dropped items (`item.json`): their item, or fire.
    if cfg.can_light_up("minecraft:item", false) {
        for d in game.dropped.iter() {
            // `EntityType.ITEM`'s eye height: 0.85 of 0.25.
            let eye = 0.2125;
            let submerged = game.eye_in_water(d.position.x, d.position.y + eye, d.position.z);
            let fire = if d.fire_ticks > 0 { 15 } else { 0 };
            let level = item_light(game, cfg, &d.stack.item, submerged).max(fire);
            if level > 0 {
                let mut at = lerp(&d.previous, &d.position, alpha);
                at[1] += eye;
                out.push(((ITEM, d.id), Lit { at, level }));
            }
        }
    }
    // Projectiles: fireballs and skulls, spectral arrows, thrown items' own
    // light, and anything on fire (a Flame arrow).
    for s in game.creatures.projectiles.iter() {
        let Some(id) = shot_type(s) else {
            continue;
        };
        if !cfg.can_light_up(id, false) {
            continue;
        }
        let eye = s.kind.eye_height();
        let submerged = game.eye_in_water(s.position.x, s.position.y + eye, s.position.z);
        let mut level = ldl::projectile_luminance(id, submerged);
        if s.fire_ticks > 0 {
            level = 15;
        }
        if id == "minecraft:spectral_arrow"
            && let Some(a) = s.arrow.as_ref()
        {
            level = level.max(item_light(game, cfg, &a.pickup_item.item, submerged));
        }
        if let Some(item) = s.item.as_ref() {
            level = level.max(item_light(game, cfg, &item.item, submerged));
        }
        if level > 0 {
            let mut at = lerp(&s.previous, &s.position, alpha);
            at[1] += eye;
            out.push(((SHOT, s.id), Lit { at, level }));
        }
    }
    if cfg.can_light_up("minecraft:dragon_fireball", false) {
        for f in &game.dragon_fireballs {
            let mut at = lerp(&f.previous, &f.position, alpha);
            at[1] += 0.85;
            let level = ldl::projectile_luminance("minecraft:dragon_fireball", false);
            out.push(((DRAGON_FIREBALL, f.id), Lit { at, level }));
        }
    }
    // Lit TNT (`PrimedTntEntityMixin`), with the TNT option on.
    if cfg.tnt != ldl::Explosive::Off && cfg.can_light_up("minecraft:tnt", false) {
        for t in game.tnts.iter() {
            let start = *tnt_start.entry(t.id).or_insert(t.fuse);
            let level = ldl::tnt_luminance(cfg, t.fuse, start);
            if level > 0 {
                let mut at = lerp(&t.previous, &t.position, alpha);
                at[1] += 0.15;
                out.push(((TNT, t.id), Lit { at, level }));
            }
        }
    }
    // Item frames (`item_frame.json`): their item; a glow item frame 12.
    for f in &game.item_frames {
        let ty = if f.glow {
            "minecraft:glow_item_frame"
        } else {
            "minecraft:item_frame"
        };
        if !cfg.can_light_up(ty, false) {
            continue;
        }
        let at = f.pos.map(|v| f64::from(v) + 0.5);
        let submerged = game.eye_in_water(at[0], at[1], at[2]);
        let item: Option<&ItemStack> = f.item.as_ref();
        let mut level = item.map_or(0, |s| item_light(game, cfg, &s.item, submerged));
        if f.glow {
            level = level.max(12);
        }
        if level > 0 {
            out.push(((FRAME, f.id), Lit { at, level }));
        }
    }
}

/// The active beacons' beams (`BeaconBlockEntityMixin`): from above the
/// beacon up the height of its sections.
fn beacon_beams(game: &Game<'_>) -> Vec<([f64; 3], [f64; 3])> {
    let mut out = Vec::new();
    for (pos, entity) in &game.world.block_entities {
        if let horizon_client::containers::BlockEntity::Beacon(b) = entity
            && b.levels > 0
            && !b.sections.is_empty()
        {
            let height: i32 = b.sections.iter().map(|s| s.1).sum();
            let (x, y, z) = *pos;
            let (x, z) = (f64::from(x) + 0.5, f64::from(z) + 0.5);
            let base = f64::from(y) + 1.0;
            out.push(([x, base, z], [x, base + f64::from(height.max(1)), z]));
        }
    }
    out
}

/// Lines as points near the camera, every 3 blocks within 9 of it.
fn line_sources(
    lines: &[([f64; 3], [f64; 3], u8)],
    camera: [f64; 3],
    out: &mut Vec<([f64; 3], u8)>,
) {
    for (a, b, level) in lines {
        for p in ldl::line_points(*a, *b, camera, 3.0, 9.0) {
            out.push((p, *level));
        }
    }
}

fn source(lit: &Lit) -> ([f64; 3], u8) {
    (lit.at, lit.level)
}

/// In place of `gather_dynamic_lights`: this mod's sources for Horizon's
/// dynamic light shader (the 16 nearest the camera, at the levels the light
/// reach option fits to the original's falloff). Off in the settings, or the
/// mod turned off: Horizon gathers its own (its Extras option).
pub(super) fn gather(
    game: &mut Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
) -> core::ops::ControlFlow<()> {
    if !game.native_on(ldl::ID) {
        return core::ops::ControlFlow::Continue(());
    }
    let cfg = config(game);
    if cfg.mode == ldl::Mode::Off {
        return core::ops::ControlFlow::Continue(());
    }
    let owner = me(game);
    let cam = camera.to_array();
    let mut sources = Vec::new();
    let mut lines = Vec::new();
    {
        let game: &Game<'_> = game;
        with_game(owner, |st| {
            let mut lits = Vec::new();
            entity_lights(
                game,
                &cfg,
                alpha,
                owner,
                &mut st.tnt_start,
                &mut lits,
                &mut lines,
            );
            if cfg.mode == ldl::Mode::Fancy {
                sources.extend(lits.iter().map(|(_, lit)| source(lit)));
            } else {
                // The Fast modes: as last refreshed by the client tick.
                sources.extend(st.held.values().map(source));
            }
            if cfg.beam {
                let beams: Vec<_> = st
                    .beacons
                    .iter()
                    .map(|(a, b)| (*a, *b, ldl::BEAM_LUMINANCE))
                    .collect();
                line_sources(&beams, cam, &mut sources);
            }
        });
    }
    line_sources(&lines, cam, &mut sources);
    let mut out = std::mem::take(&mut game.dynamic_lights);
    out.clear();
    out.extend(ldl::select(&sources, cam, cfg.reach));
    game.dynamic_lights = out;
    core::ops::ControlFlow::Break(())
}

/// The end of a client tick: the beacons are read again, and in the Fast
/// modes each source refreshes in its turn (`LambDynLights.shouldTick`).
pub(super) fn tick(game: &mut Game<'_>) {
    let owner = me(game);
    if game.title.is_some() || game.player.is_none() || !game.native_on(ldl::ID) {
        forget(owner);
        return;
    }
    let cfg = config(game);
    let game: &Game<'_> = game;
    with_game(owner, |st| {
        st.tick = st.tick.wrapping_add(1);
        if cfg.mode != ldl::Mode::Off && cfg.beam {
            st.beacons = beacon_beams(game);
        } else {
            st.beacons.clear();
        }
        st.tnt_start
            .retain(|id, _| game.tnts.iter().any(|t| t.id == *id));
        if matches!(cfg.mode, ldl::Mode::Fast | ldl::Mode::Fastest) {
            let mut lits = Vec::new();
            let mut lines = Vec::new();
            entity_lights(
                game,
                &cfg,
                1.0,
                owner,
                &mut st.tnt_start,
                &mut lits,
                &mut lines,
            );
            let mut next = HashMap::with_capacity(lits.len());
            for (key, lit) in lits {
                let kept = if ldl::due(cfg.mode, st.tick, key.1) {
                    None
                } else {
                    st.held.get(&key).copied()
                };
                next.insert(key, kept.unwrap_or(lit));
            }
            st.held = next;
        } else {
            st.held.clear();
        }
    });
}

// ---------------------------------------------------------------------------
// Options > Mods, the settings page and the command
// ---------------------------------------------------------------------------

/// The mod's row in Options > Mods.
pub(super) fn listed(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: ldl::ID.into(),
        title: format!("{} {}", ldl::NAME, ldl::VERSION),
        description: ldl::DESCRIPTION.into(),
        keys: Vec::new(),
    });
}

/// The settings page: a button per option and per listed entity.
pub(super) fn page(game: &mut Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id != ldl::ID {
        return;
    }
    let cfg = config(game);
    let ru = ru();
    buttons.extend(ldl::page().into_iter().map(|s| cfg.label(s, ru)));
}

/// A click on the page: the option's next value (Shift: the previous one).
pub(super) fn click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != ldl::ID {
        return;
    }
    let Some(setting) = ldl::page().get(button).copied() else {
        return;
    };
    let mut cfg = config(game);
    let changed = cfg.click(setting, game.has_shift_down());
    store(game, cfg, &changed);
}

/// The command's name, for the chat box's completion.
pub(super) fn commands(commands: &mut Vec<&'static str>) {
    commands.push(ldl::COMMAND);
    commands.push(ldl::SHORT_COMMAND);
}

/// A line in chat, in the game's language.
fn say(game: &mut Game<'_>, en: impl AsRef<str>, rus: impl AsRef<str>) {
    let line = if ru() { rus.as_ref() } else { en.as_ref() };
    game.native_chat(line);
}

/// `/lambdynamiclights ...` (or `/lambdynlights ...`).
pub(super) fn command(game: &mut Game<'_>, line: &str, ran: &mut bool) {
    let Some(command) = ldl::parse_command(line) else {
        return;
    };
    if !game.native_on(ldl::ID) {
        return;
    }
    *ran = true;
    match command {
        ldl::Command::Open => {
            game.list_native_mods();
            let index = game
                .settings
                .mods
                .installed
                .iter()
                .position(|m| m.id == ldl::ID && m.native.is_some());
            if let Some(index) = index {
                game.open_options_at(false, horizon_client::options_ui::Page::NativeMod(index));
            }
        }
        ldl::Command::Entity(id, on) => {
            let mut cfg = config(game);
            let changed = cfg.set_entity(&id, on);
            if changed {
                store(game, cfg, &["light_sources.disabled_entities"]);
            }
            let (en, rus) = if on {
                ("on", "включён")
            } else {
                ("off", "выключен")
            };
            say(
                game,
                format!("Light of {id}: {en}"),
                format!("Свет от {id} {rus}"),
            );
        }
        ldl::Command::Disabled => {
            let list = config(game).disabled.join(", ");
            if list.is_empty() {
                say(
                    game,
                    "No entity is switched off",
                    "Выключенных сущностей нет",
                );
            } else {
                say(
                    game,
                    format!("Switched off: {list}"),
                    format!("Выключены: {list}"),
                );
            }
        }
        ldl::Command::Help => say(game, ldl::help(false), ldl::help(true)),
    }
}
