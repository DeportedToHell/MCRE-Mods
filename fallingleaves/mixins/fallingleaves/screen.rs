// Falling Leaves' settings screen: the screen AutoConfig builds for
// `FallingLeavesConfig` (`AutoConfigClient.getConfigScreen`, through Mod
// Menu's `FallingLeavesMenu`), in Cloth Config's look (cloth.rs). Its three
// tabs hold the fields in the class's order, with the mod's GUI providers:
// the leaf blocks' sub-categories (`LeafSettingsGuiProvider`) and the sets
// as string lists (`StringSetGuiProvider`). Opened by Mod Menu's Configure
// (`MODMENU_CONFIGURE`) and by Configure... on the mod's page in Options >
// Mods; while it is up it covers the options. Save & Quit saves as
// AutoConfig does (`fallingleaves_glue::save`).
use super::fallingleaves_cloth::{Category, Entry, Request, Screen as Cloth, Value};
use super::{Game, Screen};
use horizon_client::animate_tick::fallingleaves as fl;
use horizon_client::clipboard::Clipboard as _;
use horizon_client::hud::HudBatch;
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page;
use std::cell::RefCell;

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after
/// it opened the mod's page.
pub(super) const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

struct State {
    screen: Option<Cloth>,
    /// Opened by Mod Menu: closing leaves the options too.
    modmenu: bool,
    /// The mod's row in Options > Mods.
    row: usize,
    /// The blocks of "Leaf Block Settings", with what each is.
    leaves: Vec<(String, fl::LeafClass)>,
}

thread_local! {
    static STATE: RefCell<State> = const {
        RefCell::new(State { screen: None, modmenu: false, row: 0, leaves: Vec::new() })
    };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().map(|mut s| f(&mut s)))
}

fn russian() -> bool {
    horizon_client::lang::code().starts_with("ru")
}

// ---- texts ----------------------------------------------------------------------------------

const OPTION: &str = "text.autoconfig.fallingleaves2.option.";

/// The mod's `en_us.json`.
fn english(key: &str) -> Option<&'static str> {
    let key = key.strip_prefix(OPTION).map_or(key.to_owned(), |k| format!("option.{k}"));
    Some(match key.as_str() {
        "text.autoconfig.fallingleaves2.title" => "Falling Leaves",
        "option.enabled" => "Enabled",
        "option.alwaysUseVanillaParticles" => "Always Use \"Vanilla\" Particle Implementation",
        "option.alwaysUseVanillaParticles.@Tooltip" => {
            "Note that vanilla particles are unaffected by:\nLeaf Size, Life Span, Needed vertical space, and Wind."
        }
        "option.useVanillaTextures" => "Use Vanilla Textures",
        "option.useVanillaTextures.@Tooltip" => "This applies to the \"Regular\" Particle Implementation",
        "option.leafSize" => "Leaf Size",
        "option.leafLifespan" => "Leaf Life Span (in Ticks)",
        "option.leafLifespan.@Tooltip" => "20 ticks are roughly 1 second",
        "option.leafSpawnRate" => "Leaf Spawn Rate",
        "option.leafSpawnRate.@Tooltip" => "How many leaves spawn over time",
        "option.coniferLeafSpawnRate" => "Conifer Leaf Spawn Rate",
        "option.coniferLeafSpawnRate.@Tooltip" => "How many conifer leaves spawn over time",
        "option.snowflakeSpawnRate" => "Snowflake Spawn Rate",
        "option.snowflakeSpawnRate.@Tooltip" => "How many snowflakes fall from snowy trees over time",
        "option.dropFromPlayerPlacedBlocks" => "Drop leaves from player-placed blocks?",
        "option.leavesOnBlockHit" => "Drop some leaves when hitting a block?",
        "option.minimumFreeSpaceBelow" => "Needed vertical space (in half-blocks)",
        "option.minimumFreeSpaceBelow.@Tooltip" => "Leaves only fall from a block if there's enough free space below it",
        "option.windEnabled" => "Enable Wind?",
        "option.windEnabled.@Tooltip" => "There's always some amount of wind, this option allows for stronger wind",
        "option.windlessDimensions" => "Windless Dimensions",
        "option.fallSpawnRateFactor" => "Fall Spawn Rate Multiplier",
        "option.fallSpawnRateFactor.@Tooltip" => "For Fabric Seasons and Serene Seasons",
        "option.winterSpawnRateFactor" => "Winter Spawn Rate Multiplier",
        "option.winterSpawnRateFactor.@Tooltip" => "For Fabric Seasons and Serene Seasons",
        "option.leafSpawners" => "Additional block states that drop leaves",
        "option.leafSpawners.@Tooltip[0]" => "E.g. minecraft:pink_wool or minecraft:bamboo[leaves=large].",
        "option.leafSpawners.@Tooltip[1]" => "After enabling this setting, a restart is required!",
        "option.startingSpawnRadius" => "Minimal Spawn Radius",
        "option.startingSpawnRadius.@Tooltip" => "Leaves will not spawn inside this radius around the player.",
        "option.decaySpawnRateFactor" => "Decaying Leaf Spawn Rate Multiplier",
        "option.decaySpawnRateFactor.@Tooltip" => "Adjusts the spawn rate of leaves which are about to decay.",
        "option.maxDecayLeaves" => "Decayed Leaf Max. Spawn Count",
        "option.maxDecayLeaves.@Tooltip[0]" => "When a leaf block decays/turns to air it'll spawn some leaves.",
        "option.maxDecayLeaves.@Tooltip[1]" => "Only works in singleplayer.",
        "option.registerParticles" => "Register Leaf Particles",
        "option.registerParticles.@Tooltip" => {
            "Allows the /particle command to be used (in singleplayer).\nCan cause issues when hosting LAN servers (e.g. via Essential)\nwhen the client doesn't have Falling Leaves installed.\nChanging this setting requires a restart."
        }
        "text.autoconfig.fallingleaves2.category.fallingleaves.general" => "General",
        "text.autoconfig.fallingleaves2.category.fallingleaves.leafsettings" => "Leaf Block Settings",
        "text.autoconfig.fallingleaves2.category.fallingleaves.experimental" => "Experimental",
        "config.fallingleaves2.particle_implementation" => "Particle Implementation",
        "config.fallingleaves2.particle_implementation.@Tooltip" => {
            "\"Vanilla\" refers to the use of a vanilla method, which mods\nmay use as well to add their own implementation.\n\"Regular\", \"Conifer\" and \"Cherry\" are custom implementations.\n\"Cherry\" is close to the vanilla implementation and uses\nvanilla textures."
        }
        "config.fallingleaves2.particle_implementation.VANILLA" => "Vanilla",
        "config.fallingleaves2.particle_implementation.REGULAR" => "Regular",
        "config.fallingleaves2.particle_implementation.CONIFER" => "Conifer",
        "config.fallingleaves2.particle_implementation.CHERRY" => "Cherry",
        "config.fallingleaves2.spawn_rate_factor" => "Spawn Rate Multiplier",
        "config.fallingleaves2.spawn_rate_factor.@Tooltip" => "0% turns spawning off, 100% means base spawn rate",
        "config.fallingleaves2.spawn_breaking_leaves" => "Drop some leaves when this block is hit or decays?",
        _ => return None,
    })
}

/// The mod's `ru_ru.json` (it lacks some of the keys).
fn russian_text(key: &str) -> Option<&'static str> {
    let key = key.strip_prefix(OPTION).map_or(key.to_owned(), |k| format!("option.{k}"));
    Some(match key.as_str() {
        "text.autoconfig.fallingleaves2.title" => "Falling leaves",
        "option.enabled" => "Включено",
        "option.leafSize" => "Размер листьев",
        "option.leafLifespan" => "Продолжительность жизни листьев (в тиках)",
        "option.leafLifespan.@Tooltip" => "20 тиков - это примерно 1 секунда",
        "option.leafSpawnRate" => "Скорость спавна листьев",
        "option.leafSpawnRate.@Tooltip" => "Сколько листьев спавнится с течением времени",
        "option.coniferLeafSpawnRate" => "Коэффициент спавна хвойных листьев",
        "option.coniferLeafSpawnRate.@Tooltip" => "Сколько листьев хвойных деревьев спавнится с течением времени",
        "option.dropFromPlayerPlacedBlocks" => "Включить падение листьев из рамещённых игроком блоков?",
        "option.leavesOnBlockHit" => "Сбросить несколько листьев при ударе о блок?",
        "option.minimumFreeSpaceBelow" => "Необходимое вертикальное пространство (в полублоках)",
        "option.minimumFreeSpaceBelow.@Tooltip" => {
            "Листья падают с блока только в том случае, если под ним достаточно свободного пространства"
        }
        "option.windEnabled" => "Включить ветер?",
        "option.windEnabled.@Tooltip" => {
            "Всегда есть некоторое количество ветра, этот вариант позволяет использовать более сильный ветер"
        }
        "option.windlessDimensions" => "Безветренные измерения",
        "option.fallSpawnRateFactor" => "Множитель спавна осенью",
        "option.fallSpawnRateFactor.@Tooltip" => "Для Fabric Seasons",
        "option.winterSpawnRateFactor" => "Множитель спавна зимой",
        "option.winterSpawnRateFactor.@Tooltip" => "Для Fabric Seasons",
        "option.leafSpawners" => "Дополнительные блоки, сбрасывающие листья",
        "option.leafSpawners.@Tooltip[0]" => "Например: minecraft:pink_wool, minecraft:bamboo[leaves=large].",
        "option.leafSpawners.@Tooltip[1]" => "После включения этой настройки требуется перезапуск!",
        "option.startingSpawnRadius" => "Минимальный радиус спавна",
        "option.startingSpawnRadius.@Tooltip" => "Листья не будут появляться внутри этого радиуса вокруг игрока.",
        "option.decaySpawnRateFactor" => "Множитель скорости спавна распадающихся листьев",
        "option.decaySpawnRateFactor.@Tooltip" => "Регулирует скорость спавна листьев, которые скоро распадутся.",
        "option.maxDecayLeaves" => "Максимальное количество распадающихся листьев. Множитель спавна.",
        "option.maxDecayLeaves.@Tooltip[0]" => "Когда блок листьев распадается, он порождает несколько листьев.",
        "option.maxDecayLeaves.@Tooltip[1]" => "Работает только в одиночной игре.",
        "text.autoconfig.fallingleaves2.category.fallingleaves.general" => "Общее",
        "text.autoconfig.fallingleaves2.category.fallingleaves.leafsettings" => "Настройки блока листьев",
        "text.autoconfig.fallingleaves2.category.fallingleaves.experimental" => "Экспериментальное",
        "config.fallingleaves2.spawn_rate_factor" => "Множитель скорости спавна",
        "config.fallingleaves2.spawn_rate_factor.@Tooltip" => "0% отключает спавн, 100% означает базовый спавн",
        _ => return None,
    })
}

/// `Component.translatable(key)`: the Russian text when the game is in
/// Russian, English for a key `ru_ru.json` lacks (the game loads `en_us`
/// under every language), the key itself for none.
fn t(ru: bool, key: &str) -> String {
    let found = if ru { russian_text(key).or_else(|| english(key)) } else { english(key) };
    found.unwrap_or(key).to_owned()
}

/// A tooltip's lines: one per `\n`.
fn lines(ru: bool, keys: &[&str]) -> Vec<String> {
    keys.iter().flat_map(|k| t(ru, k).split('\n').map(str::to_owned).collect::<Vec<_>>()).collect()
}

// ---- building ---------------------------------------------------------------------------------

/// The spawn rate multiplier as the slider counts it: tens of percent,
/// `(int) (factor * 100.0) / 10`.
fn rate_steps(factor: f32) -> i64 {
    (f64::from(factor) * 100.0) as i64 / 10
}

/// `LeafSettingsGuiProvider.getModName`: the namespace's mod, or the
/// namespace itself.
fn mod_name(id: &str) -> String {
    match id.split_once(':').map_or("minecraft", |(ns, _)| ns) {
        "minecraft" => "Minecraft".to_owned(),
        fl::ID => fl::NAME.to_owned(),
        ns => ns.to_owned(),
    }
}

/// The block's name (`Block.getDescriptionId`), when the language has it.
fn block_name(id: &str) -> (String, Option<String>) {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    let key = format!("block.{ns}.{}", path.replace('/', "."));
    let name = horizon_client::lang::has(&key).then(|| horizon_client::lang::get(&key));
    (key, name)
}

/// The blocks of "Leaf Block Settings": `leafSettings` holds every leaf
/// block, the spawners and what the file names, and the screen shows those
/// the game has.
fn leaf_rows(game: &Game<'_>, cfg: &fl::Config) -> Vec<(String, fl::LeafClass)> {
    let mut rows = super::fallingleaves_glue::leaf_blocks(game);
    let extra: Vec<String> = cfg.spawner_ids().chain(cfg.leaf_settings.keys().cloned()).collect();
    for id in extra {
        if !rows.iter().any(|(r, _)| *r == id) && super::fallingleaves_glue::is_block(game, &id) {
            rows.push((id.clone(), super::fallingleaves_glue::class(game, &id)));
        }
    }
    // `TranslationComparator`: by the names, by the keys when one has none.
    let mut keyed: Vec<_> = rows.into_iter().map(|r| (block_name(&r.0), r)).collect();
    keyed.sort_by(|((k1, n1), _), ((k2, n2), _)| match (n1, n2) {
        (Some(a), Some(b)) => a.encode_utf16().cmp(b.encode_utf16()),
        _ => k1.cmp(k2),
    });
    keyed.into_iter().map(|(_, r)| r).collect()
}

/// One leaf block's sub-category (`LeafSettingsGuiProvider.get`): its
/// particle implementation, spawn rate multiplier and leaves when hit.
fn leaf_entry(ru: bool, cfg: &fl::Config, id: &str, class: fl::LeafClass) -> Entry {
    let defaults = fl::LeafSettings::defaults(id, class);
    let now = cfg.leaf_settings(id, class).unwrap_or(defaults);
    let (key, name) = block_name(id);
    let mut title = name.unwrap_or(key);
    if now != defaults {
        title.push('*');
    }
    let at = |i: fl::Implementation| fl::Implementation::ALL.iter().position(|x| *x == i).unwrap_or(0);
    let names = fl::Implementation::ALL
        .iter()
        .map(|i| t(ru, &format!("config.fallingleaves2.particle_implementation.{}", i.name())))
        .collect();
    let implementation = Entry::select(
        &format!("{id}|implementation"),
        &t(ru, "config.fallingleaves2.particle_implementation"),
        names,
        at(now.implementation),
        at(defaults.implementation),
    );
    let tip = lines(ru, &["config.fallingleaves2.particle_implementation.@Tooltip"]);
    let implementation = implementation.tooltip(&tip.iter().map(String::as_str).collect::<Vec<_>>());
    let tip = lines(ru, &["config.fallingleaves2.spawn_rate_factor.@Tooltip"]);
    let rate = Entry::slider(
        &format!("{id}|rate"),
        &t(ru, "config.fallingleaves2.spawn_rate_factor"),
        rate_steps(now.spawn_rate_factor),
        rate_steps(defaults.spawn_rate_factor),
        0,
        150,
    )
    .slider_text(|v| format!("{}%", v * 10))
    .tooltip(&tip.iter().map(String::as_str).collect::<Vec<_>>());
    let breaking = Entry::boolean(
        &format!("{id}|breaking"),
        &t(ru, "config.fallingleaves2.spawn_breaking_leaves"),
        now.spawn_breaking_leaves,
        defaults.spawn_breaking_leaves,
    );
    Entry::sub(&title, vec![implementation, rate, breaking], false).tooltip(&[&mod_name(id)])
}

/// AutoConfig's screen for the settings now and their defaults.
fn build(ru: bool, cfg: &fl::Config, leaves: &[(String, fl::LeafClass)], in_world: bool) -> Cloth {
    let d = fl::Config::default();
    let name = |field: &str| t(ru, &format!("{OPTION}{field}"));
    // `@ConfigEntry.Gui.Tooltip`: `<option>.@Tooltip`, or `[0]`, `[1]` for
    // two lines.
    let tip = |entry: Entry, field: &str, count: usize| {
        let keys: Vec<String> = if count == 1 {
            vec![format!("{OPTION}{field}.@Tooltip")]
        } else {
            (0..count).map(|i| format!("{OPTION}{field}.@Tooltip[{i}]")).collect()
        };
        let text = lines(ru, &keys.iter().map(String::as_str).collect::<Vec<_>>());
        entry.tooltip(&text.iter().map(String::as_str).collect::<Vec<_>>())
    };
    let boolean = |field: &str, v: bool, dv: bool| Entry::boolean(field, &name(field), v, dv);
    // `@BoundedDiscrete`: a slider.
    let slider = |field: &str, v: i32, dv: i32, min: i64, max: i64| {
        Entry::slider(field, &name(field), i64::from(v), i64::from(dv), min, max)
    };
    let float = |field: &str, v: f32, dv: f32| Entry::float(field, &name(field), &fl::java_float(v), &fl::java_float(dv));
    // `StringSetGuiProvider`: `startStrList`, closed, new items last.
    let list = |field: &str, v: &[String], dv: &[String]| Entry::list(field, &name(field), v.to_vec(), dv.to_vec(), false, false);

    let general = vec![
        boolean("enabled", cfg.enabled, d.enabled),
        tip(boolean("alwaysUseVanillaParticles", cfg.always_use_vanilla_particles, d.always_use_vanilla_particles), "alwaysUseVanillaParticles", 1),
        tip(boolean("useVanillaTextures", cfg.use_vanilla_textures, d.use_vanilla_textures), "useVanillaTextures", 1),
        slider("leafSize", cfg.leaf_size, d.leaf_size, 1, 10),
        tip(slider("leafLifespan", cfg.leaf_lifespan, d.leaf_lifespan, 100, 600), "leafLifespan", 1),
        tip(slider("leafSpawnRate", cfg.leaf_spawn_rate, d.leaf_spawn_rate, 0, 40), "leafSpawnRate", 1),
        tip(slider("coniferLeafSpawnRate", cfg.conifer_leaf_spawn_rate, d.conifer_leaf_spawn_rate, 0, 40), "coniferLeafSpawnRate", 1),
        tip(slider("snowflakeSpawnRate", cfg.snowflake_spawn_rate, d.snowflake_spawn_rate, 0, 40), "snowflakeSpawnRate", 1),
        boolean("dropFromPlayerPlacedBlocks", cfg.drop_from_player_placed_blocks, d.drop_from_player_placed_blocks),
        boolean("leavesOnBlockHit", cfg.leaves_on_block_hit, d.leaves_on_block_hit),
        tip(slider("minimumFreeSpaceBelow", cfg.minimum_free_space_below, d.minimum_free_space_below, 1, 20), "minimumFreeSpaceBelow", 1),
        tip(boolean("windEnabled", cfg.wind_enabled, d.wind_enabled), "windEnabled", 1),
        list("windlessDimensions", &cfg.windless_dimensions, &d.windless_dimensions),
    ];
    let leaf_settings = leaves.iter().map(|(id, class)| leaf_entry(ru, cfg, id, *class)).collect();
    let experimental = vec![
        tip(list("leafSpawners", &cfg.leaf_spawners, &d.leaf_spawners), "leafSpawners", 2),
        tip(float("fallSpawnRateFactor", cfg.fall_spawn_rate_factor, d.fall_spawn_rate_factor), "fallSpawnRateFactor", 1),
        tip(float("winterSpawnRateFactor", cfg.winter_spawn_rate_factor, d.winter_spawn_rate_factor), "winterSpawnRateFactor", 1),
        tip(slider("startingSpawnRadius", cfg.starting_spawn_radius, d.starting_spawn_radius, 0, 23), "startingSpawnRadius", 1),
        tip(float("decaySpawnRateFactor", cfg.decay_spawn_rate_factor, d.decay_spawn_rate_factor), "decaySpawnRateFactor", 1),
        tip(slider("maxDecayLeaves", cfg.max_decay_leaves, d.max_decay_leaves, 0, 30), "maxDecayLeaves", 2),
        tip(boolean("registerParticles", cfg.register_particles, d.register_particles), "registerParticles", 1),
    ];
    let category = |key: &str, entries: Vec<Entry>| Category {
        name: t(ru, &format!("text.autoconfig.fallingleaves2.category.fallingleaves.{key}")),
        entries,
    };
    Cloth::new(
        &t(ru, "text.autoconfig.fallingleaves2.title"),
        vec![category("general", general), category("leafsettings", leaf_settings), category("experimental", experimental)],
        in_world,
    )
}

/// Save & Quit: every entry's save consumer, then `ConfigHolder.save` and
/// its listener (`validatePostLoad`, the wind started again).
fn save(game: &Game<'_>, values: Vec<(String, Value)>, leaves: &[(String, fl::LeafClass)]) {
    let mut c = fl::config();
    // A set's strings as the set takes them: identifiers parsed, each once.
    let set = |v: &Value, identifiers: bool| {
        let mut out: Vec<String> = Vec::new();
        for s in v.as_list() {
            let s = if identifiers { fl::full_id(&s) } else { s };
            if !out.contains(&s) {
                out.push(s);
            }
        }
        out
    };
    let int = |v: &Value| v.as_int() as i32;
    let mut leaf: std::collections::HashMap<String, fl::LeafSettings> = std::collections::HashMap::new();
    for (id, v) in &values {
        if let Some((block, part)) = id.split_once('|') {
            let class = leaves.iter().find(|(b, _)| b == block).map_or(fl::LeafClass::NotLeaves, |(_, c)| *c);
            let defaults = fl::LeafSettings::defaults(block, class);
            let s = leaf.entry(block.to_owned()).or_insert_with(|| c.leaf_settings(block, class).unwrap_or(defaults));
            match part {
                "implementation" => {
                    s.implementation = fl::Implementation::ALL.get(v.as_choice()).copied().unwrap_or(defaults.implementation);
                }
                "rate" => s.spawn_rate_factor = (v.as_int() * 10) as f32 / 100.0,
                "breaking" => s.spawn_breaking_leaves = v.as_bool(),
                _ => {}
            }
            continue;
        }
        match id.as_str() {
            "enabled" => c.enabled = v.as_bool(),
            "alwaysUseVanillaParticles" => c.always_use_vanilla_particles = v.as_bool(),
            "useVanillaTextures" => c.use_vanilla_textures = v.as_bool(),
            "leafSize" => c.leaf_size = int(v),
            "leafLifespan" => c.leaf_lifespan = int(v),
            "leafSpawnRate" => c.leaf_spawn_rate = int(v),
            "coniferLeafSpawnRate" => c.conifer_leaf_spawn_rate = int(v),
            "snowflakeSpawnRate" => c.snowflake_spawn_rate = int(v),
            "dropFromPlayerPlacedBlocks" => c.drop_from_player_placed_blocks = v.as_bool(),
            "leavesOnBlockHit" => c.leaves_on_block_hit = v.as_bool(),
            "minimumFreeSpaceBelow" => c.minimum_free_space_below = int(v),
            "windEnabled" => c.wind_enabled = v.as_bool(),
            "windlessDimensions" => c.windless_dimensions = set(v, true),
            "leafSpawners" => c.leaf_spawners = set(v, false),
            "fallSpawnRateFactor" => c.fall_spawn_rate_factor = v.as_float(),
            "winterSpawnRateFactor" => c.winter_spawn_rate_factor = v.as_float(),
            "startingSpawnRadius" => c.starting_spawn_radius = int(v),
            "decaySpawnRateFactor" => c.decay_spawn_rate_factor = v.as_float(),
            "maxDecayLeaves" => c.max_decay_leaves = int(v),
            "registerParticles" => c.register_particles = v.as_bool(),
            _ => {}
        }
    }
    // The core keeps only the entries that differ from their block's
    // defaults, and the spawners'.
    for (block, s) in leaf {
        let class = leaves.iter().find(|(b, _)| *b == block).map_or(fl::LeafClass::NotLeaves, |(_, c)| *c);
        if s == fl::LeafSettings::defaults(&block, class) && !c.is_leaf_spawner(&block) {
            c.leaf_settings.remove(&block);
        } else {
            c.leaf_settings.insert(block, s);
        }
    }
    c.validate();
    super::fallingleaves_glue::save(game, c);
}

// ---- opening, covering, closing -----------------------------------------------------------

/// The mod's row in Options > Mods.
fn row(game: &Game<'_>) -> Option<usize> {
    game.settings.mods.installed.iter().position(|m| m.id == fl::ID && m.native.is_some())
}

/// Opens the screen over the mod's page: `from_modmenu` when Mod Menu's
/// Configure asked.
pub(super) fn open(game: &Game<'_>, from_modmenu: bool) {
    let Some(row) = row(game) else {
        return;
    };
    let ru = russian();
    let cfg = fl::config();
    let leaves = leaf_rows(game, &cfg);
    let screen = build(ru, &cfg, &leaves, game.ui.options.in_world);
    with(|s| {
        s.screen = Some(screen);
        s.modmenu = from_modmenu;
        s.row = row;
        s.leaves = leaves;
    });
}

/// The screen is up and the options show the mod's page under it.
pub(super) fn covers(game: &Game<'_>) -> bool {
    let (open, row) = with(|s| (s.screen.is_some(), s.row)).unwrap_or((false, 0));
    open && game.screen == Screen::Options
        && game.ui.options.page() == Page::NativeMod(row)
        && game.settings.mods.installed.get(row).is_some_and(|m| m.id == fl::ID)
}

fn close(game: &mut Game<'_>) {
    let Some((modmenu, row)) = with(|s| {
        s.screen = None;
        (s.modmenu, s.row)
    }) else {
        return;
    };
    if modmenu {
        // As Escape on the page Mod Menu opened alone: back to Mod Menu.
        let events = game.ui.options.back(&mut game.settings);
        let _ = game.handle_option_events(events);
    } else {
        game.native_settings_refresh(row);
    }
}

/// Once a frame: what the screen asked for, and the screen gone when its
/// page is.
pub(super) fn frame(game: &mut Game<'_>) {
    if !with(|s| s.screen.is_some()).unwrap_or(false) {
        return;
    }
    if !covers(game) {
        // The page changed some other way: closed, nothing saved.
        with(|s| s.screen = None);
        return;
    }
    let requests = with(|s| s.screen.as_mut().map(Cloth::take_requests).unwrap_or_default()).unwrap_or_default();
    for r in requests {
        match r {
            Request::Click => game.audio.play_button_click(),
            Request::Paste => {
                let text = game.clipboard.get();
                with(|s| {
                    if let Some(screen) = s.screen.as_mut() {
                        screen.typed(&text);
                    }
                });
            }
            Request::Copy(text) => {
                if !text.is_empty() {
                    game.clipboard.set(&text);
                }
            }
            Request::Close(saved) => {
                if saved {
                    let values = with(|s| s.screen.as_ref().map(Cloth::values)).flatten().unwrap_or_default();
                    let leaves = with(|s| s.leaves.clone()).unwrap_or_default();
                    save(game, values, &leaves);
                }
                close(game);
                return;
            }
        }
    }
}

// ---- the screen hooks -----------------------------------------------------------------------

/// The screen over the options, after they drew (nothing: `draw_options` is
/// held back while the screen covers them).
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if screen.id != "options" || !covers(game) {
        return;
    }
    with(|s| {
        if let Some(c) = s.screen.as_mut() {
            c.draw(game, batch);
        }
    });
}

/// Runs `f` on the open screen when it covers `screen`: true when it did
/// (the event goes no further).
fn input(game: &Game<'_>, screen: &NativeScreenView<'_>, f: impl FnOnce(&mut Cloth, f32, f32, (f32, f32))) -> bool {
    if screen.id != "options" || !covers(game) {
        return false;
    }
    let (w, h) = (screen.rect[2].floor(), screen.rect[3].floor());
    let p = (screen.mouse[0], screen.mouse[1]);
    with(|s| {
        if let Some(c) = s.screen.as_mut() {
            f(c, w, h, p);
        }
    });
    true
}

pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, modifiers: u32) -> bool {
    input(game, screen, |c, w, h, p| c.press(game, w, h, p, button, modifiers))
}

pub(super) fn released(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    input(game, screen, |c, _, _, _| c.release(button))
}

pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    input(game, screen, |c, w, h, p| c.dragged(game, w, h, p))
}

pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    input(game, screen, |c, w, h, p| c.scrolled(game, w, h, p, scroll[1] as f32))
}

pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    input(game, screen, |c, w, h, _| c.key_pressed(game, w, h, key, modifiers))
}

pub(super) fn key_released(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    input(game, screen, |c, _, _, _| c.key_released())
}

pub(super) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char) -> bool {
    input(game, screen, |s, _, _, _| s.typed(&c.to_string()))
}
