// Particle Rain in the game: its settings and particles in memory and in their files
// (`ConfigManager`, `ParticleLoader`), the resource reload it asks for when a texture setting
// changed, the `/particlerain` command (the config screen, `debug`, `biometags`), its row and
// page in Options > Mods and what Mod Menu shows of it. Spliced into game.rs as
// `particlerain_glue`.
//
// Particle Rain by PigCart (https://github.com/PigCart/particle-rain), MIT.

use super::particlerain_core::{self as core, Config, ParticleData, Tags};
use super::particlerain_lang::{self as lang, t};
use super::particlerain_weather as weather;
use super::{Game, Screen};
use horizon_client::hud::particlerain_atlas as atlas;
use horizon_client::mod_api::NativeMod;
use horizon_client::text::Text;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) use core::ID;

/// The mod's settings and particles.
pub(super) struct State {
    /// `ConfigManager.config`.
    pub(super) config: Config,
    /// `ParticleLoader.particles`: the built-in ones with the player's changes, and the player's own.
    pub(super) particles: Vec<ParticleData>,
    /// `ParticleLoader.packParticles`: the built-in ones as the resources give them.
    pub(super) pack: Vec<ParticleData>,
    pub(super) tags: Option<Tags>,
    /// `ConfigScreen.shouldReloadResources`.
    pub(super) reload_resources: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static LOADED: AtomicBool = AtomicBool::new(false);

pub(super) fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut guard = STATE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let st = guard.get_or_insert_with(|| {
        let pack = core::pack_particles(None);
        State { config: Config::default(), particles: pack.clone(), pack, tags: None, reload_resources: false }
    });
    f(st)
}

fn config_path(name: &str) -> Option<std::path::PathBuf> {
    Game::native_config_dir().ok().map(|d| d.join("particlerain").join(name))
}

/// `ConfigManager.save` and `ParticleLoader.saveCustomParticles`.
pub(super) fn save() {
    let (config, particles) = with(|st| (st.config.to_json(), core::particles_json(&st.particles, &st.pack)));
    if let Some(path) = config_path("config.json") {
        if let Some(dir) = path.parent()
            && let Err(e) = std::fs::create_dir_all(dir)
        {
            log::error!("Particle Rain: couldn't create directory 'config/particlerain/': {e}");
        }
        if let Err(e) = std::fs::write(&path, config) {
            log::error!("Particle Rain: couldn't save config: {e}");
        }
    }
    if let Some(path) = config_path("particles.json")
        && let Err(e) = std::fs::write(&path, particles)
    {
        log::error!("Particle Rain: {e}");
    }
}

/// The resources' own `assets/particlerain/particles.json` (a resource pack's), if any.
fn pack_file(game: &Game<'_>) -> Option<String> {
    std::fs::read_to_string(game.options.resources.join("assets/particlerain/particles.json")).ok()
}

/// `ParticleLoader.onResourceManagerReload`: the built-in particles from the resources, the
/// player's changes and particles over them.
fn load_particles(game: &Game<'_>) {
    let pack = core::pack_particles(pack_file(game).as_deref());
    let mut particles = pack.clone();
    if let Some(text) = config_path("particles.json").and_then(|p| std::fs::read_to_string(p).ok())
        && !core::merge_particles(&text, &mut particles)
    {
        log::error!("Particle Rain: unable to load custom particles");
    }
    log::info!("Particle Rain: loaded {} particles", particles.len());
    with(|st| {
        st.pack = pack;
        st.particles = particles;
    });
}

/// `ConfigManager.load`, once: the file, or the defaults written when there is none (or it is
/// unreadable, or an older version's).
fn load(game: &Game<'_>) {
    if LOADED.swap(true, Ordering::Relaxed) {
        return;
    }
    let text = config_path("config.json").and_then(|p| std::fs::read_to_string(p).ok());
    let config = text.as_deref().and_then(Config::parse);
    let write = config.is_none();
    with(|st| st.config = config.unwrap_or_default());
    load_particles(game);
    if write {
        save();
    }
    publish();
}

/// The texture settings for the atlas's next packing.
fn publish() {
    with(|st| {
        atlas::WATER_TINT.store(st.config.compat.water_tint, Ordering::Relaxed);
        atlas::RIPPLE_PACK_RES.store(st.config.compat.ripple_use_pack_res, Ordering::Relaxed);
        atlas::RIPPLE_RES.store(st.config.compat.ripple_res.max(0) as u32, Ordering::Relaxed);
    });
}

/// `Minecraft.reloadResourcePacks`, for a changed texture setting.
pub(super) fn reload_resources(game: &mut Game<'_>) {
    publish();
    game.reload_packs();
}

static CHECKED_ATLAS: AtomicBool = AtomicBool::new(false);
static WROTE_DATA: AtomicBool = AtomicBool::new(false);

/// Each frame of the window's game: the settings, the mod's data files, and once the first atlas
/// is packed, a reload if the settings ask for other sprites than it has.
pub(super) fn frame(game: &mut Game<'_>) {
    if game.window.is_none() {
        return;
    }
    load(game);
    write_data(game);
    publish_columns(game);
    let built = atlas::BUILT.load(Ordering::Relaxed);
    if built != 0 && !CHECKED_ATLAS.swap(true, Ordering::Relaxed) && built != atlas::wanted() {
        reload_resources(game);
    }
}

/// `client_resources_reloaded`: the particles again (a resource pack may bring its own).
pub(super) fn reloaded(game: &mut Game<'_>) {
    if game.window.is_none() || !LOADED.load(Ordering::Relaxed) {
        return;
    }
    load_particles(game);
}

/// `client_level_load` (`Minecraft.setLevel` clears the particles).
pub(super) fn level_load() {
    weather::clear();
}

// ---- the tick ------------------------------------------------------------------------------

/// The Horizon Weather option, held off through the game's particle tick so its own weather
/// splashes and rain sound make way for the mod's (`ClientLevelMixin`).
static WEATHER_ON: AtomicBool = AtomicBool::new(false);

/// The head of the game's particle tick.
pub(super) fn tick_head(game: &mut Game<'_>) {
    let on = game.window.is_some() && game.native_on(ID);
    WEATHER_ON.store(game.settings.horizon.weather, Ordering::Relaxed);
    if on {
        game.settings.horizon.weather = false;
    }
}

/// The end of the game's particle tick (`ParticleRain.onTick`, `END_CLIENT_TICK`).
pub(super) fn tick_tail(game: &mut Game<'_>) {
    if game.window.is_none() {
        return;
    }
    let weather = WEATHER_ON.load(Ordering::Relaxed);
    game.settings.horizon.weather = weather;
    if !game.native_on(ID) {
        return;
    }
    load(game);
    weather::tick(game, weather);
}

// ---- /particlerain -------------------------------------------------------------------------------

const RED: u32 = 0xFF_5555;

/// A line in this client's chat (`addClientSystemMessage`).
fn chat(game: &mut Game<'_>, line: Text) {
    let focused = game.screen == Screen::Chat;
    let ticks = game.visual_ticks();
    let atlas = &game.gui.atlas;
    game.ui.chat.add(atlas, line, None, horizon_client::chat::ChatSource::Client, ticks, focused);
}

/// Java's `Float.toString` of a float shown in the debug lines.
fn jf(v: f32) -> String {
    core::java_float(v)
}

/// `ParticleRain.getDebugLines`.
fn debug_lines(game: &Game<'_>) -> Vec<String> {
    let Some(player) = game.player.as_ref() else { return Vec::new() };
    let b = [player.position.x, player.position.y, player.position.z].map(|v| v.floor() as i32);
    let (rain, thunder) = game.weather_levels(1.0);
    let biome = game.world.chunks.get(&(b[0] >> 4, b[2] >> 4)).map(|c| c.biome(b[0], b[1], b[2]));
    let biomes = match &game.world_biomes {
        Some(x) => x,
        None => game.res.biomes(),
    };
    let precipitation = match biome.and_then(|i| biomes.get(i).snows_at(b[0], b[1], b[2])) {
        None => "NONE",
        Some(true) => "SNOW",
        Some(false) => "RAIN",
    };
    let speed = (player.velocity.x * player.velocity.x + player.velocity.y * player.velocity.y + player.velocity.z * player.velocity.z).sqrt() as f32;
    let (after, block_fx, sky_fx, surface_fx) = weather::counters();
    let amount = with(|st| st.config.perf.particle_amount);
    let default_height = super::render_tick::motion_blocking_height(&game.world, &game.pack.assets.properties, b[0], b[2]);
    vec![
        format!("Tracked particles: {}/{}", weather::count(), amount),
        format!("after Weather Ticks Left: {after}"),
        format!("spawn Attempts Until Block FX Idle: {block_fx}"),
        format!("ticks Until Sky FX Idle: {sky_fx}"),
        format!("ticks Until Surface FX Idle: {surface_fx}"),
        format!("is Raining: {}", game.can_have_weather() && rain > 0.2),
        format!("Biome Precipitation: {precipitation}"),
        format!("Wind multiplier: {}", jf(weather::y_level_wind_multiplier(f64::from(b[1])))),
        "Wind source: particlerain".to_owned(),
        format!("cloud height: {}", horizon_client::clouds::CLOUD_HEIGHT as i32),
        format!("surface height: {}", weather::height_at(game, b[0], b[2])),
        format!("default surface height: {default_height}"),
        format!("speed: {}", jf(speed)),
        format!("sky particle density: {}", weather::sky_density(thunder, speed)),
        format!("Rain level: {}", jf(rain)),
        format!("Thunder level: {}", jf(thunder)),
    ]
}

/// `ParticleRain.getBiomeTags`.
fn biome_tags(game: &Game<'_>) -> Vec<String> {
    let Some(player) = game.player.as_ref() else { return Vec::new() };
    let b = [player.position.x, player.position.y, player.position.z].map(|v| v.floor() as i32);
    let biomes = match &game.world_biomes {
        Some(x) => x,
        None => game.res.biomes(),
    };
    let Some(id) = game.world.chunks.get(&(b[0] >> 4, b[2] >> 4)).map(|c| c.biome(b[0], b[1], b[2])) else {
        return Vec::new();
    };
    let name = biomes.get(id).name.clone();
    let tags = with(|st| st.tags.get_or_insert_with(|| Tags::load(&game.vanilla_root())).biome_tags(&name));
    let mut out = vec![format!("{name} has the following tags:")];
    out.extend(tags.into_iter().map(|t| format!("#{t}")));
    out
}

/// Asked by the command, done on the next frame (`VersionUtil.schedule`: the chat closes first).
static OPEN_SCREEN: AtomicBool = AtomicBool::new(false);

/// Hook `command`: `/particlerain [debug|biometags]`.
pub(super) fn command(game: &mut Game<'_>, input: &str, ran: &mut bool) {
    if game.window.is_none() || !game.native_on(ID) || input.split(' ').next() != Some(ID) {
        return;
    }
    *ran = true;
    load(game);
    let rest = &input[ID.len()..];
    match rest {
        "" => OPEN_SCREEN.store(true, Ordering::Relaxed),
        " debug" => {
            for line in debug_lines(game) {
                chat(game, Text::literal(line));
            }
        }
        " biometags" => {
            for line in biome_tags(game) {
                chat(game, Text::literal(line));
            }
        }
        _ => {
            // Brigadier: "Incorrect argument for command" with the input up to the bad part.
            let at = if rest.starts_with(' ') { ID.len() + 1 } else { ID.len() };
            let context = format!("{}{}", if at > 10 { "..." } else { "" }, &input[at.saturating_sub(10)..at]);
            let line = Text::translate(
                "command.context.parse_error",
                &[
                    Text::translate("command.unknown.argument", &[]),
                    Text::literal(at.to_string()),
                    Text::literal(context).append(Text::translate("command.context.here", &[])),
                ],
            );
            chat(game, Text::empty().append(line).with_color(RED));
        }
    }
}

/// Hook `commands`: `/particlerain` is the client's own.
pub(super) fn commands(commands: &mut Vec<&'static str>) {
    commands.push(ID);
}

/// Whether the command asked for the screen (taken).
pub(super) fn take_open_screen() -> bool {
    OPEN_SCREEN.swap(false, Ordering::Relaxed)
}

// ---- Options > Mods and Mod Menu --------------------------------------------------------------

/// `mod.description`.
const DESCRIPTION: &str = "Replaces weather with prettier particle effects";
const DESCRIPTION_RU: &str = "Заменяет погоду более красивыми эффектами частиц";

/// Hook `mods`: the mod's row in Options > Mods.
pub(super) fn list(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: ID.into(),
        title: format!("{} {}", core::NAME, core::VERSION),
        description: if lang::russian() { DESCRIPTION_RU } else { DESCRIPTION }.into(),
        keys: Vec::new(),
    });
}

/// Hook `settings`: the mod's page, Configure... first.
pub(super) fn page(mod_id: &str, labels: &mut Vec<String>) {
    if mod_id == ID {
        labels.push(if lang::russian() { "Настройки..." } else { "Configure..." }.into());
    }
}

/// The icon (the original's) and what Mod Menu shows of the mod.
const ICON: &[u8] = horizon_client::native_data!("particlerain", "icon.png");

fn modmenu_json() -> String {
    let s = |v: &str| serde_json::to_string(v).unwrap_or_default();
    format!(
        r#"{{
  "schema": 1,
  "name": "{name}",
  "version": "{version}",
  "description": {description},
  "description_ru": {description_ru},
  "authors": ["pigcart"],
  "contributors": ["plunderpixels", "HigorFr", "LilleboHaugen", "CH4ACKO3", "megatocha", "Sasaki-Akari", "good7777865", "DeportedToHell (Horizon port)"],
  "license": "MIT",
  "links": {{
    "homepage": "https://modrinth.com/mod/particle-rain",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/particlerain",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues",
    "modmenu.discord": "https://pigc.art/discord"
  }},
  "badges": ["client"],
  "config_screen": true
}}
"#,
        name = core::NAME,
        version = core::VERSION,
        description = s(DESCRIPTION),
        description_ru = s(DESCRIPTION_RU),
    )
}

/// `icon.png` and `modmenu.json` in the mod's data folder, once (each written only when it differs).
fn write_data(game: &Game<'_>) {
    if WROTE_DATA.swap(true, Ordering::Relaxed) {
        return;
    }
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

/// The settings' title (`particlerain.title`).
pub(super) fn title() -> String {
    t("particlerain.title")
}

// ---- the game's rain and snow -------------------------------------------------------------------

/// Whether the mod is on in the window's game, and "Render default rain" / "snow" (bits 0, 1, 2),
/// for the game's weather columns.
static COLUMNS: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

fn publish_columns(game: &Game<'_>) {
    let (rain, snow) = with(|st| (st.config.compat.render_default_rain, st.config.compat.render_default_snow));
    let on = game.native_on(ID);
    COLUMNS.store(u8::from(on) | u8::from(rain) << 1 | u8::from(snow) << 2, Ordering::Relaxed);
}

/// `WeatherEffectRendererMixin`: the game's rain columns only with "Render default rain", its
/// snow columns only with "Render default snow".
pub(super) fn columns(
    game: &mut Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
) -> std::ops::ControlFlow<Vec<horizon_client::weather::Vertex>> {
    let flags = COLUMNS.load(Ordering::Relaxed);
    if flags & 1 == 0 || game.window.is_none() || IN_COLUMNS.load(Ordering::Relaxed) {
        return std::ops::ControlFlow::Continue(());
    }
    let (rain, snow) = (flags & 2 != 0, flags & 4 != 0);
    if rain && snow {
        return std::ops::ControlFlow::Continue(());
    }
    if !rain && !snow {
        return std::ops::ControlFlow::Break(Vec::new());
    }
    IN_COLUMNS.store(true, Ordering::Relaxed);
    let mut value = game.weather_geometry(camera, alpha);
    IN_COLUMNS.store(false, Ordering::Relaxed);
    value.retain(|v| if v.snow != 0 { snow } else { rain });
    std::ops::ControlFlow::Break(value)
}

/// Set while the game's own weather geometry is made for the filter.
static IN_COLUMNS: AtomicBool = AtomicBool::new(false);
