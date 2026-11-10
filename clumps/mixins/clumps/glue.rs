// Clumps in the game: experience orbs of any value clump together, and a
// player collects a touched orb whole (`MixinExperienceOrb`).
//
// - `canMerge` (both forms): any orb takes in any other, not only one of its
//   value and id slot: the game's merge question is answered yes.
// - `merge` and `tryMergeToExisting`: the orb that takes another in becomes
//   worth both (Clumps' `clumpedMap` resolved), counted as one orb. Done once
//   the merge is decided (after the other mods had their say), and refused
//   when the clump would be worth more than an orb can hold.
// - `playerTouch`: the whole orb taken at once, Mending repairing with it and
//   the rest given. (Clumps also lifts the player's pickup delay, so an orb
//   goes every tick; Horizon's server keeps its delay of two ticks between
//   orbs, which the mod can't change.)
//
// Only on the server's `Game` (single player's server thread, a LAN host, a
// dedicated server); as a client of a server the server decides.
// Ported from Clumps by Jaredlll08 (MIT, https://github.com/jaredlll08/Clumps).
use super::Game;
use super::clumps as c;
use horizon_client::mod_api::NativeMod;
use std::cell::Cell;

/// The original's icon (`common/src/main/resources/icon.png`), for Mod Menu.
const ICON: &[u8] = horizon_client::native_data!("clumps", "icon.png");

thread_local! {
    static DATA_WRITTEN: Cell<bool> = const { Cell::new(false) };
}

fn russian() -> bool {
    horizon_client::lang::code().starts_with("ru")
}

/// The mod is on and this game runs the world's orbs.
fn active(game: &Game<'_>) -> bool {
    game.remote.is_none() && game.native_on(c::ID)
}

// ---- Options > Mods and Mod Menu ------------------------------------------------------------

/// The mod's row in Options > Mods (no settings: Clumps has none).
pub(super) fn list(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: c::ID.into(),
        title: format!("{} {}", c::NAME, c::VERSION),
        description: c::description(russian()).into(),
        keys: Vec::new(),
    });
}

/// Mod Menu's icon and metadata in the mod's folder, each written when it
/// differs from what is there, at the first frame.
pub(super) fn frame(game: &Game<'_>) {
    if DATA_WRITTEN.with(|w| w.replace(true)) {
        return;
    }
    let dir = game.native_data_dir(c::ID);
    let _ = std::fs::create_dir_all(&dir);
    let json = format!(
        r#"{{
  "schema": 1,
  "name": "{}",
  "version": "{}",
  "description": "{}",
  "description_ru": "{}",
  "authors": ["Jaredlll08"],
  "contributors": ["Darkhax", "ChampionAsh5357", "Chloe Dawn", "FalseHonesty", "Griefed", "Hache_raw", "Paint_Ninja", "SkySom", "OathtakerPaladin", "Kastromovskiy (Horizon port)"],
  "license": "MIT",
  "links": {{
    "homepage": "https://www.curseforge.com/minecraft/mc-mods/clumps",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/clumps",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "badges": [],
  "config_screen": false
}}
"#,
        c::NAME,
        c::VERSION,
        c::description(false),
        c::description(true),
    );
    for (name, bytes) in [("icon.png", ICON), ("modmenu.json", json.as_bytes())] {
        let path = dir.join(name);
        if std::fs::read(&path).ok().as_deref() != Some(bytes) {
            let _ = std::fs::write(&path, bytes);
        }
    }
}

// ---- merging --------------------------------------------------------------------------------

/// `canMerge`: any two orbs (the game only asks about live ones).
pub(super) fn can_merge(game: &Game<'_>, can_merge: &mut bool) {
    if active(game) {
        *can_merge = true;
    }
}

/// The orb `into` takes in `from` (0: a new orb of `from_value` points, which
/// isn't an entity yet), the merge decided: `into` becomes worth both, as one
/// orb. The game then adds the orb taken in to `into`'s count (one for a new
/// orb, `from`'s count otherwise), so the counts are set for that to leave
/// one.
pub(super) fn merged(game: &mut Game<'_>, into: u32, from: u32, from_value: u32, canceled: &mut bool) {
    if *canceled || !active(game) {
        return;
    }
    let Some(i) = game.orbs.iter().position(|o| o.id == into) else {
        return;
    };
    let other = if from == 0 {
        None
    } else {
        match game.orbs.iter().position(|o| o.id == from) {
            Some(j) => Some(j),
            None => return,
        }
    };
    let into_points = c::points(game.orbs[i].value, game.orbs[i].count);
    let from_points = other.map_or(u64::from(from_value), |j| c::points(game.orbs[j].value, game.orbs[j].count));
    let Some(value) = c::merged(into_points, from_points) else {
        // Worth more than one orb can hold: both stay as they are.
        *canceled = true;
        return;
    };
    game.orbs[i].value = value;
    match other {
        // `scanForMerges` adds the count of the orb it took in: none.
        Some(j) => {
            game.orbs[i].count = 1;
            game.orbs[j].count = 0;
        }
        // `tryMergeToExisting` counts one orb more.
        None => game.orbs[i].count = 0,
    }
}

// ---- the player's pickup ------------------------------------------------------------------

/// The server asks the pickup hook only when a mod listens.
pub(super) fn pickup_listened(game: &Game<'_>, listened: &mut bool) {
    if active(game) {
        *listened = true;
    }
}

/// `playerTouch`: the orb is taken whole, all its points at once (an orb
/// counting several gives them in one go, and goes).
pub(super) fn pickup(game: &Game<'_>, value: &mut u32, count: &mut u32) {
    if !active(game) {
        return;
    }
    *value = c::points(*value, *count).clamp(1, c::MAX_POINTS) as u32;
    *count = 1;
}
