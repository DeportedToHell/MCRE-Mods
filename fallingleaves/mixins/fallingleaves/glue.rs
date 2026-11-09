// Falling Leaves in the game: the settings file, the wind's tick, the colour
// of each leaf (`LeafUtil.getBlockTextureColor`), the particles the ambient
// tick asked for, leaves from a hit or a decayed block, the mod's textures in
// the atlas, and its row and page in Options > Mods.
use super::Game;
use horizon_client::animate_tick::fallingleaves as fl;
use horizon_client::mod_api::NativeMod;
use horizon_client::particles::fallingleaves_particle as particle;
use horizon_world::registry::BlockStateId;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

// ---- the settings file ------------------------------------------------------------------

/// The file in the game's `config/` folder, as AutoConfig keeps it.
fn config_path() -> Option<std::path::PathBuf> {
    Game::native_config_dir().ok().map(|d| d.join(fl::CONFIG_FILE))
}

/// What a block is to the mod, for the defaults of its leaf settings.
pub(super) fn class(game: &Game<'_>, id: &str) -> fl::LeafClass {
    let registry = game.pack.assets.registry;
    let facts = game.res.facts();
    let tables = game.res.animate();
    (0..registry.len() as u32)
        .map(BlockStateId)
        .find(|&s| fl::full_id(&facts.get(s).name) == id)
        .map_or(fl::LeafClass::NotLeaves, |s| fl::classify(tables, facts, s))
}

/// A block the game has (`BuiltInRegistries.BLOCK.containsKey`).
pub(super) fn is_block(game: &Game<'_>, id: &str) -> bool {
    let registry = game.pack.assets.registry;
    let facts = game.res.facts();
    (0..registry.len() as u32).map(BlockStateId).any(|s| fl::full_id(&facts.get(s).name) == id)
}

/// Every leaf block, with what it is: the rows of "Leaf Block Settings"
/// (`getRegisteredLeafBlocks`), by id.
pub(super) fn leaf_blocks(game: &Game<'_>) -> Vec<(String, fl::LeafClass)> {
    let registry = game.pack.assets.registry;
    let facts = game.res.facts();
    let tables = game.res.animate();
    let mut out: Vec<(String, fl::LeafClass)> = Vec::new();
    for s in (0..registry.len() as u32).map(BlockStateId) {
        let id = fl::full_id(&facts.get(s).name);
        if out.iter().any(|(i, _)| *i == id) {
            continue;
        }
        let class = fl::classify(tables, facts, s);
        if class != fl::LeafClass::NotLeaves || facts.get(s).name.ends_with("_leaves") {
            out.push((id, class));
        }
    }
    out
}

/// Saves the settings (`Config.save`) and starts the wind again, as its save
/// listener does.
pub(super) fn save(game: &Game<'_>, c: fl::Config) {
    let classes: HashMap<String, fl::LeafClass> = c.leaf_settings.keys().map(|id| (id.clone(), class(game, id))).collect();
    let json = c.to_json(&|id| classes.get(id).copied().unwrap_or(fl::LeafClass::NotLeaves));
    fl::set_config(c);
    if let Some(path) = config_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, json);
    }
    restart_wind();
}

static LOADED: AtomicBool = AtomicBool::new(false);

/// The settings from the file, once; AutoConfig writes the defaults when there
/// is none.
fn load(game: &Game<'_>) {
    if LOADED.swap(true, Ordering::Relaxed) {
        return;
    }
    match config_path().and_then(|p| std::fs::read_to_string(p).ok()) {
        Some(text) => fl::set_config(fl::Config::parse(&text)),
        None => save(game, fl::Config::default()),
    }
}

// ---- the wind ---------------------------------------------------------------------------

static WIND: Mutex<Option<fl::Wind>> = Mutex::new(None);
/// The client level's dimension, from `client_level_load`.
static DIMENSION: Mutex<String> = Mutex::new(String::new());

/// `Wind.init`.
fn restart_wind() {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as i64);
    if let Ok(mut w) = WIND.lock() {
        *w = Some(fl::Wind::new(seed));
    }
    particle::set_wind(0.0, 0.0);
}

/// `MinecraftClientMixin`: a new level starts the wind again.
pub(super) fn level_load(dimension: &str) {
    if let Ok(mut d) = DIMENSION.lock() {
        *d = dimension.to_owned();
    }
    restart_wind();
}

// ---- the tick ---------------------------------------------------------------------------

static TICKS: AtomicU32 = AtomicU32::new(0);
static FLOW_SET: AtomicBool = AtomicBool::new(false);

/// At the head of the game's particle tick (`ParticleManagerMixin`): the wind,
/// then the particles asked for since the last tick.
pub(super) fn tick(game: &mut Game<'_>) {
    // Only the window's game: single player's server ticks a game of its own.
    if game.window.is_none() {
        return;
    }
    load(game);
    fl::set_client_engine(&game.particles);
    let on = game.native_on(fl::ID);
    fl::set_active(on);
    if !on {
        return;
    }
    if !FLOW_SET.swap(true, Ordering::Relaxed) {
        let res = game.res.clone();
        particle::set_flow(Box::new(move |world, b| horizon_client::entities::fluid_flow(world, &res.motion().fluids, b)));
    }
    // The textures again now and then: a new renderer starts the atlas blank.
    if TICKS.fetch_add(1, Ordering::Relaxed) % 20 == 0 {
        upload_textures(game);
    }
    fl::set_player(game.player.as_ref().map(|p| [p.position.x, p.position.y, p.position.z].map(|v| v.floor() as i32)));
    let (rain, thunder) = game.weather_levels(1.0);
    particle::set_raining(rain > 0.2);
    let cfg = fl::config();
    if !cfg.enabled {
        let _ = fl::take_requests();
        let _ = fl::take_decayed();
        return;
    }
    let dimension = DIMENSION.lock().map(|d| d.clone()).unwrap_or_default();
    let windless = !cfg.wind_enabled || cfg.windless_dimensions.iter().any(|d| fl::full_id(d) == dimension);
    if let Ok(mut w) = WIND.lock() {
        let wind = w.get_or_insert_with(|| fl::Wind::new(0x5EED));
        wind.tick(windless, rain > 0.2, thunder > 0.9);
        particle::set_wind(wind.x, wind.z);
    }
    for (state, pos) in fl::take_decayed() {
        with_area(game, |engine, area, tables| fl::decayed(engine, area, tables, state, pos));
    }
    let look = particle::Look {
        lifespan: cfg.leaf_lifespan,
        size: cfg.leaf_size(),
        vanilla_textures: cfg.use_vanilla_textures,
    };
    for r in fl::take_requests() {
        let rgb = match r.kind {
            particle::Kind::Leaf | particle::Kind::Conifer => leaf_color(game, r.state, r.at.map(|v| v.floor() as i32)),
            particle::Kind::Cherry | particle::Kind::Snow => [1.0; 3],
        };
        particle::spawn(&mut game.particles, r.kind, r.at, rgb, look);
    }
}

/// The level's blocks for the mod's spawning, beside the particle engine.
fn with_area(
    game: &mut Game<'_>,
    f: impl FnOnce(&mut horizon_client::particles::ParticleEngine, &fl::Area<'_>, &horizon_client::animate_tick::AnimateTables),
) {
    let biomes = match &game.world_biomes {
        Some(b) => b,
        None => game.res.biomes(),
    };
    let area = fl::Area {
        world: &game.world,
        facts: game.res.facts(),
        collision: game.collision,
        registry: game.pack.assets.registry,
        biomes,
    };
    f(&mut game.particles, &area, game.res.animate());
}

/// `registerAttackBlockLeaves`: the player hit the block at `pos`.
pub(super) fn hit(game: &mut Game<'_>, pos: [i32; 3]) {
    if !game.native_on(fl::ID) {
        return;
    }
    let state = game.world.block(pos[0], pos[1], pos[2]);
    with_area(game, |engine, area, tables| fl::hit(engine, area, tables, state, pos));
}

// ---- the colour of a leaf ---------------------------------------------------------------

/// The averaged texture of each block state's leaf (`TextureCache`), and
/// whether its face takes the block's tint, for the resources loaded now.
#[derive(Default)]
struct Colors {
    version: u64,
    states: HashMap<u32, Option<([f32; 3], bool)>>,
    textures: HashMap<String, [f32; 3]>,
}
static COLORS: Mutex<Option<Colors>> = Mutex::new(None);

/// `getBlockTextureColor`: the average colour of the block's first bottom
/// face's texture (or of its particle texture), times the block's tint at `pos`.
fn leaf_color(game: &Game<'_>, state: BlockStateId, pos: [i32; 3]) -> [f32; 3] {
    let Ok(mut guard) = COLORS.lock() else {
        return [1.0; 3];
    };
    let colors = guard.get_or_insert_with(Colors::default);
    let version = game.native_block_sprites_version();
    if colors.version != version {
        *colors = Colors { version, ..Colors::default() };
    }
    let known = colors.states.get(&state.0).copied();
    let found = match known {
        Some(f) => f,
        None => {
            let f = texture_color(game, state, &mut colors.textures);
            colors.states.insert(state.0, f);
            f
        }
    };
    let Some((mut rgb, tinted)) = found else {
        // A deep pink, as the original shows for a model it can't read.
        return [1.0, 0.1, 0.6];
    };
    if tinted && let Some((_, tint)) = game.particle_sprite(state, pos) {
        for (c, shift) in rgb.iter_mut().zip([16, 8, 0]) {
            *c *= ((tint >> shift) & 255) as f32 / 255.0;
        }
    }
    rgb
}

/// The block's texture, averaged, and whether it is tinted.
fn texture_color(game: &Game<'_>, state: BlockStateId, textures: &mut HashMap<String, [f32; 3]>) -> Option<([f32; 3], bool)> {
    let states = game.native_block_states();
    let sprites = states.sprites(state)?;
    // The first bottom face, else the particle texture (always tinted then).
    let (sprite, tinted) = match sprites.sides[0].first() {
        Some(face) => (face.sprite, face.tint.is_some()),
        None => (sprites.particle?, true),
    };
    let name = states.texture(sprite)?.to_owned();
    if let Some(c) = textures.get(&name) {
        return Some((*c, tinted));
    }
    let pixels = game.native_client_resources().texture(&name)?;
    let (mut sum, mut n) = ([0.0f64; 3], 0u32);
    for px in pixels.rgba.chunks_exact(4) {
        if px[3] != 0 {
            for i in 0..3 {
                sum[i] += f64::from(px[i]);
            }
            n += 1;
        }
    }
    let c = if n == 0 { [1.0; 3] } else { sum.map(|s| (s / f64::from(n) / 255.0) as f32) };
    textures.insert(name, c);
    Some((c, tinted))
}

/// The resources changed: colours again, and the textures into the new atlas.
pub(super) fn reloaded(game: &mut Game<'_>) {
    if let Ok(mut c) = COLORS.lock() {
        *c = None;
    }
    upload_textures(game);
}

// ---- the mod's textures -----------------------------------------------------------------

/// `falling_leaf_1..5`, `falling_leaf_conifer_1..3` and `falling_snow`, side
/// by side with a clear pixel between them (tools/tiles.mjs makes it).
const TILES: &[u8] = horizon_client::native_data!("fallingleaves", "tiles.rgba");

/// The strip into its place in the mod-image slot.
fn upload_textures(game: &mut Game<'_>) {
    let width = particle::TILES * particle::TILE_STEP;
    let height = particle::TILE;
    if TILES.len() != (width * height * 4) as usize {
        return;
    }
    let Some(rect) = game.gui.atlas.sprites.get(horizon_client::hud::MOD_IMAGE_SPRITE).copied() else {
        return;
    };
    particle::set_slot(&mut game.particles, rect);
    let (w, h) = (game.gui.atlas.width as f32, game.gui.atlas.height as f32);
    game.map_patches.push(horizon_client::font::Patch {
        x: (rect.u0 * w).round() as u32,
        y: (rect.v0 * h).round() as u32 + particle::TILE_ROW,
        width,
        height,
        rgba: TILES.to_vec(),
    });
}

// ---- Options > Mods and Mod Menu --------------------------------------------------------

fn russian() -> bool {
    horizon_client::lang::code().starts_with("ru")
}

/// The mod's description: the original's poem.
pub(super) const DESCRIPTION: &str = "\"How the gentle wind \nbeckons through the leaves \nas autumn colors fall,\ndancing in a swirl \nof golden memories:\nthe loveliest lies of all.\"";

/// The mod's row in Options > Mods.
pub(super) fn list(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: fl::ID.into(),
        title: format!("{} {}", fl::NAME, fl::VERSION),
        description: DESCRIPTION.replace(" \n", " ").replace('\n', " "),
        keys: Vec::new(),
    });
}

/// The mod's page: its first button opens the settings screen.
pub(super) fn page(mod_id: &str, labels: &mut Vec<String>) {
    if mod_id == fl::ID {
        labels.push(if russian() { "Настройки..." } else { "Configure..." }.into());
    }
}

/// A click on the page, or Mod Menu's Configure: the settings screen.
pub(super) fn page_click(game: &Game<'_>, mod_id: &str, button: usize) {
    if mod_id != fl::ID {
        return;
    }
    if button == 0 || button == super::fallingleaves_screen::MODMENU_CONFIGURE {
        super::fallingleaves_screen::open(game, button != 0);
    }
}

/// The icon (the original's) and what Mod Menu shows of the mod.
const ICON: &[u8] = horizon_client::native_data!("fallingleaves", "icon.png");

fn modmenu_json() -> String {
    let description = serde_json::to_string(DESCRIPTION).unwrap_or_default();
    format!(
        r#"{{
  "schema": 1,
  "name": "{name}",
  "version": "{version}",
  "description": {description},
  "authors": ["Fourmisain", "BrekiTomasson", "RandomMcSomethin"],
  "contributors": ["DeportedToHell"],
  "license": "MIT",
  "links": {{
    "homepage": "https://www.curseforge.com/minecraft/mc-mods/falling-leaves-fabric",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/fallingleaves",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "badges": ["client"],
  "config_screen": true
}}
"#,
        name = fl::NAME,
        version = fl::VERSION,
    )
}

static WROTE_DATA: AtomicBool = AtomicBool::new(false);

/// Each frame: `icon.png` and `modmenu.json` in the mod's data folder, once
/// (each written only when it differs).
pub(super) fn frame(game: &mut Game<'_>) {
    load(game);
    if WROTE_DATA.swap(true, Ordering::Relaxed) {
        return;
    }
    let dir = game.native_data_dir(fl::ID);
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
