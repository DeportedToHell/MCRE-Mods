// BetterF3 for Horizon: the mod's settings and modules (`GeneralOptions`,
// `BaseModule` and its subclasses' fields, `ModConfigFile`), its texts
// (BetterF3's own en_us.json and ru_ru.json) and Component.translatable as
// the game formats it. Ported from BetterF3 by cominixo and TreyRuffy (MIT,
// https://github.com/TreyRuffy/BetterF3).
use horizon_client::text::{Span, Style, Text};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;

pub(super) const ID: &str = "betterf3";
pub(super) const VERSION: &str = "1.0.0";

// ---- texts ---------------------------------------------------------------------------------

const EN: &[u8] = horizon_client::native_data!("betterf3", "lang/en_us.json");
const RU: &[u8] = horizon_client::native_data!("betterf3", "lang/ru_ru.json");

fn table(bytes: &[u8]) -> HashMap<String, String> {
    serde_json::from_slice(bytes).unwrap_or_default()
}

fn en() -> &'static HashMap<String, String> {
    static T: OnceLock<HashMap<String, String>> = OnceLock::new();
    T.get_or_init(|| table(EN))
}

fn ru_table() -> &'static HashMap<String, String> {
    static T: OnceLock<HashMap<String, String>> = OnceLock::new();
    T.get_or_init(|| table(RU))
}

thread_local! {
    static RUSSIAN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Follows the game's language (once a frame).
pub(super) fn set_language(code: &str) {
    RUSSIAN.with(|r| r.set(code.starts_with("ru")));
}

pub(super) fn russian() -> bool {
    RUSSIAN.with(std::cell::Cell::get)
}

/// `Language.getOrDefault`: the translation, else the key.
pub(super) fn tr(key: &str) -> String {
    let found = if russian() { ru_table().get(key).or_else(|| en().get(key)) } else { en().get(key) };
    found.cloned().unwrap_or_else(|| key.to_owned())
}

/// `I18n.get(key, args)`: `String.format` of the translation (BetterF3's
/// formats there only take `%s`).
pub(super) fn trf(key: &str, args: &[&dyn std::fmt::Display]) -> String {
    let template = tr(key);
    let mut out = String::new();
    let mut next = 0;
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.peek() {
                Some('s') => {
                    chars.next();
                    if let Some(a) = args.get(next) {
                        out.push_str(&a.to_string());
                    }
                    next += 1;
                    continue;
                }
                Some('%') => {
                    chars.next();
                    out.push('%');
                    continue;
                }
                _ => {}
            }
        }
        out.push(c);
    }
    out
}

/// A literal in one colour (`Utils.styledText`).
pub(super) fn styled(s: impl Into<String>, color: u32) -> Text {
    let text = s.into();
    let mut t = Text::default();
    if !text.is_empty() {
        t.spans.push(Span { text, style: Style::color(color) });
    }
    t
}

/// A literal without a style of its own.
pub(super) fn plain(s: impl Into<String>) -> Text {
    let text = s.into();
    let mut t = Text::default();
    if !text.is_empty() {
        t.spans.push(Span { text, style: Style::default() });
    }
    t
}

/// `Component.translatable(key, args)` as `TranslatableContents` decomposes
/// it: `%s` and `%n$s` take the arguments, `%%` is a percent sign, and any
/// other `%` (or a missing argument) leaves the whole translation as it is.
pub(super) fn translatable(key: &str, args: &[Text]) -> Text {
    let template = tr(key);
    format_template(&template, args).unwrap_or_else(|| plain(template))
}

fn format_template(template: &str, args: &[Text]) -> Option<Text> {
    let chars: Vec<char> = template.chars().collect();
    let mut out = Text::default();
    let mut literal = String::new();
    let mut next = 0usize;
    let mut i = 0;
    let flush = |out: &mut Text, literal: &mut String| {
        if !literal.is_empty() {
            out.spans.push(Span { text: std::mem::take(literal), style: Style::default() });
        }
    };
    while i < chars.len() {
        if chars[i] != '%' {
            literal.push(chars[i]);
            i += 1;
            continue;
        }
        let mut j = i + 1;
        let start = j;
        while j < chars.len() && chars[j].is_ascii_digit() {
            j += 1;
        }
        let mut index = None;
        if j > start && chars.get(j) == Some(&'$') {
            index = chars[start..j].iter().collect::<String>().parse::<usize>().ok();
            j += 1;
        } else {
            j = start;
        }
        match chars.get(j) {
            Some('%') if index.is_none() => {
                literal.push('%');
                i = j + 1;
            }
            Some('s') => {
                let n = match index {
                    Some(n) => n.checked_sub(1)?,
                    None => {
                        next += 1;
                        next - 1
                    }
                };
                let arg = args.get(n)?;
                flush(&mut out, &mut literal);
                out.spans.extend(arg.spans.iter().cloned());
                i = j + 1;
            }
            // `%` at the very end, a lone `%`, `%d`, `%.0f`: unsupported.
            _ => return None,
        }
    }
    flush(&mut out, &mut literal);
    Some(out)
}

/// `MutableComponent.withStyle(color)` on a parent: every span without a
/// colour of its own takes it.
pub(super) fn with_color(mut t: Text, color: u32) -> Text {
    for s in &mut t.spans {
        if s.style.color.is_none() {
            s.style.color = Some(color);
        }
    }
    t
}

pub(super) fn append(mut a: Text, b: Text) -> Text {
    a.spans.extend(b.spans);
    a
}

// ---- legacy colours ------------------------------------------------------------------------

pub(super) const DARK_GREEN: u32 = 0x00AA00;
pub(super) const DARK_AQUA: u32 = 0x00AAAA;
pub(super) const GOLD: u32 = 0xFFAA00;
pub(super) const GRAY: u32 = 0xAAAAAA;
pub(super) const GREEN: u32 = 0x55FF55;
pub(super) const AQUA: u32 = 0x55FFFF;
pub(super) const RED: u32 = 0xFF5555;
pub(super) const YELLOW: u32 = 0xFFFF55;
pub(super) const WHITE: u32 = 0xFFFFFF;

// ---- modules -------------------------------------------------------------------------------

/// The module classes, by their id (`getSimpleName` without "Module").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Minecraft,
    Fps,
    Graphics,
    Server,
    Coords,
    Chunks,
    Location,
    Entity,
    Sound,
    Help,
    MiscLeft,
    MiscRight,
    System,
    Target,
    Empty,
}

/// A line as the module declares it: id, format key, `isCustom`,
/// `inReducedDebug` (a `DebugLineList`'s lines come as `Val::Strings`).
pub(super) struct LineDef {
    pub(super) id: &'static str,
    pub(super) format: &'static str,
    pub(super) custom: bool,
    pub(super) reduced: bool,
}

const fn line(id: &'static str) -> LineDef {
    LineDef { id, format: "format.betterf3.default_format", custom: false, reduced: false }
}
const fn reduced(id: &'static str) -> LineDef {
    LineDef { id, format: "format.betterf3.default_format", custom: false, reduced: true }
}
const fn custom(id: &'static str, format: &'static str, reduced: bool) -> LineDef {
    LineDef { id, format, custom: true, reduced }
}
const fn list(id: &'static str, reduced: bool) -> LineDef {
    LineDef { id, format: "format.betterf3.default_format", custom: false, reduced }
}

const MINECRAFT_LINES: [LineDef; 1] =
    [LineDef { id: "minecraft", format: "format.betterf3.default_no_colon", custom: false, reduced: true }];
const FPS_LINES: [LineDef; 1] = [custom("fps", "format.betterf3.no_format", true)];
const GRAPHICS_LINES: [LineDef; 5] =
    [line("render_distance"), line("graphics"), line("clouds"), line("biome_blend_radius"), line("shader")];
const SERVER_LINES: [LineDef; 4] = [
    custom("server_tick", "format.betterf3.server_tick", true),
    reduced("packets_sent"),
    reduced("packets_received"),
    reduced("tick_manager_status"),
];
const COORDS: &str = "format.betterf3.coords";
const COORDS_LINES: [LineDef; 7] = [
    custom("player_coords", COORDS, false),
    custom("block_coords", COORDS, false),
    custom("chunk_relative_coords", COORDS, true),
    custom("chunk_coords", COORDS, false),
    custom("velocity", COORDS, false),
    line("absolute_velocity"),
    line("horizontal_velocity"),
];
const TOTAL: &str = "format.betterf3.total";
const CHUNKS_LINES: [LineDef; 11] = [
    custom("chunk_sections", TOTAL, true),
    line("chunk_culling"),
    reduced("pending_chunks"),
    reduced("pending_uploads"),
    reduced("available_buffers"),
    reduced("client_chunk_cache"),
    line("loaded_chunks"),
    line("loaded_chunks_server"),
    line("forceloaded_chunks"),
    line("spawn_chunks"),
    line("chunk_file"),
];
const LOCATION_LINES: [LineDef; 12] = [
    line("dimension"),
    line("facing"),
    line("rotation"),
    line("light"),
    line("light_server"),
    line("highest_block"),
    line("highest_block_server"),
    line("biome"),
    line("local_difficulty"),
    line("day_ticks"),
    line("days_played"),
    line("slime_chunk"),
];
const ENTITY_LINES: [LineDef; 10] = [
    reduced("particles"),
    custom("entities", TOTAL, true),
    line("monster"),
    line("creature"),
    line("ambient"),
    line("axolotls"),
    line("underground_water_creature"),
    line("water_creature"),
    line("water_ambient"),
    line("misc"),
];
const SOUND_LINES: [LineDef; 3] = [custom("sounds", TOTAL, false), custom("ambient_sounds", TOTAL, false), line("mood")];
const HELP_LINES: [LineDef; 4] = [reduced("pie_graph_new"), reduced("fps_tps_new"), reduced("ping"), reduced("help")];
const MISC_LEFT_LINES: [LineDef; 1] = [list("misc_left", true)];
const MISC_RIGHT_LINES: [LineDef; 1] = [list("misc_right", true)];
const SYSTEM_LINES: [LineDef; 11] = [
    reduced("time"),
    reduced("java_version"),
    reduced("memory_usage"),
    reduced("allocation_rate"),
    reduced("allocated_memory"),
    reduced("cpu"),
    reduced("display"),
    reduced("gpu"),
    reduced("gpu_utilization"),
    reduced("graphics_api"),
    reduced("gpu_driver"),
];
const TARGET_LINES: [LineDef; 11] = [
    line("targeted_block"),
    line("id_block"),
    list("block_states", false),
    list("block_tags", false),
    line("nothing"),
    line("targeted_fluid"),
    line("id_fluid"),
    list("fluid_states", false),
    list("fluid_tags", false),
    line("nothing2"),
    line("targeted_entity"),
];
/// `EmptyModule`: 20 blank lines, as many shown as it is set to.
const EMPTY_LINE: LineDef = LineDef { id: "nothing", format: "", custom: false, reduced: true };

/// A module's own colours besides name and value: config key, default,
/// option name and tooltip keys (`EditModulesScreen`'s order).
pub(super) struct ExtraColor {
    pub(super) key: &'static str,
    pub(super) default: u32,
    pub(super) name: &'static str,
    pub(super) tooltip: &'static str,
}

const fn extra(key: &'static str, default: u32, name: &'static str, tooltip: &'static str) -> ExtraColor {
    ExtraColor { key, default, name, tooltip }
}

const COORDS_COLORS: [ExtraColor; 3] = [
    extra("color_x", RED, "config.betterf3.color.x", "config.betterf3.color.x.tooltip"),
    extra("color_y", GREEN, "config.betterf3.color.y", "config.betterf3.color.y.tooltip"),
    extra("color_z", AQUA, "config.betterf3.color.z", "config.betterf3.color.z.tooltip"),
];
const FPS_COLORS: [ExtraColor; 3] = [
    extra("color_high", GREEN, "config.betterf3.color.fps.high", "config.betterf3.color.fps.high.tooltip"),
    extra("color_med", YELLOW, "config.betterf3.color.fps.medium", "config.betterf3.color.fps.medium.tooltip"),
    extra("color_low", RED, "config.betterf3.color.fps.low", "config.betterf3.color.fps.low.tooltip"),
];
const CHUNKS_COLORS: [ExtraColor; 3] = [
    extra("chunks_enabled_color", GREEN, "config.betterf3.color.chunks.enabled", "config.betterf3.color.chunks.enabled.tooltip"),
    extra("chunks_disabled_color", RED, "config.betterf3.color.chunks.disabled", "config.betterf3.color.chunks.disabled.tooltip"),
    extra("total_chunks_color", GOLD, "config.betterf3.color.chunks.total", "config.betterf3.color.chunks.total.tooltip"),
];
const ENTITY_COLORS: [ExtraColor; 1] =
    [extra("total_entities_color", GOLD, "config.betterf3.color.entities.total", "config.betterf3.color.entities.total.tooltip")];
const HELP_COLORS: [ExtraColor; 2] = [
    extra("enabled_color", GREEN, "config.betterf3.color.help.enabled", "config.betterf3.color.help.enabled.tooltip"),
    extra("disabled_color", RED, "config.betterf3.color.help.disabled", "config.betterf3.color.help.disabled.tooltip"),
];
const SOUND_COLORS: [ExtraColor; 1] =
    [extra("maximum_color", DARK_AQUA, "config.betterf3.color.sound.maximum", "config.betterf3.color.sound.maximum.tooltip")];

impl Kind {
    pub(super) const ALL: [Kind; 15] = [
        Kind::Minecraft,
        Kind::Fps,
        Kind::Graphics,
        Kind::Server,
        Kind::Coords,
        Kind::Chunks,
        Kind::Location,
        Kind::Entity,
        Kind::Sound,
        Kind::Help,
        Kind::MiscLeft,
        Kind::MiscRight,
        Kind::System,
        Kind::Target,
        Kind::Empty,
    ];

    pub(super) fn id(self) -> &'static str {
        match self {
            Kind::Minecraft => "minecraft",
            Kind::Fps => "fps",
            Kind::Graphics => "graphics",
            Kind::Server => "server",
            Kind::Coords => "coords",
            Kind::Chunks => "chunks",
            Kind::Location => "location",
            Kind::Entity => "entity",
            Kind::Sound => "sound",
            Kind::Help => "help",
            Kind::MiscLeft => "miscleft",
            Kind::MiscRight => "miscright",
            Kind::System => "system",
            Kind::Target => "target",
            Kind::Empty => "empty",
        }
    }

    pub(super) fn from_id(id: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.id() == id)
    }

    /// `BaseModule.toString`: the module's name in the game's language.
    pub(super) fn name(self) -> String {
        tr(&format!("text.betterf3.module.{}", self.id()))
    }

    /// The default name and value colours.
    pub(super) fn colors(self) -> (u32, u32) {
        match self {
            Kind::Minecraft => (0xA0522D, DARK_GREEN),
            Kind::Fps | Kind::Empty => (WHITE, WHITE),
            Kind::Graphics | Kind::Sound | Kind::System => (GOLD, AQUA),
            Kind::Server => (GRAY, YELLOW),
            Kind::Coords => (RED, WHITE),
            Kind::Chunks | Kind::Target => (0x00AAFF, YELLOW),
            Kind::Location => (DARK_GREEN, AQUA),
            Kind::Entity => (RED, YELLOW),
            Kind::Help | Kind::MiscLeft | Kind::MiscRight => (0xFDFD96, AQUA),
        }
    }

    pub(super) fn line_defs(self) -> &'static [LineDef] {
        match self {
            Kind::Minecraft => &MINECRAFT_LINES,
            Kind::Fps => &FPS_LINES,
            Kind::Graphics => &GRAPHICS_LINES,
            Kind::Server => &SERVER_LINES,
            Kind::Coords => &COORDS_LINES,
            Kind::Chunks => &CHUNKS_LINES,
            Kind::Location => &LOCATION_LINES,
            Kind::Entity => &ENTITY_LINES,
            Kind::Sound => &SOUND_LINES,
            Kind::Help => &HELP_LINES,
            Kind::MiscLeft => &MISC_LEFT_LINES,
            Kind::MiscRight => &MISC_RIGHT_LINES,
            Kind::System => &SYSTEM_LINES,
            Kind::Target => &TARGET_LINES,
            Kind::Empty => std::slice::from_ref(&EMPTY_LINE),
        }
    }

    pub(super) fn extra_colors(self) -> &'static [ExtraColor] {
        match self {
            Kind::Coords => &COORDS_COLORS,
            Kind::Fps => &FPS_COLORS,
            Kind::Chunks => &CHUNKS_COLORS,
            Kind::Entity => &ENTITY_COLORS,
            Kind::Help => &HELP_COLORS,
            Kind::Sound => &SOUND_COLORS,
            _ => &[],
        }
    }

    /// `updatesEveryFrame`: false for the modules whose lines rarely change.
    pub(super) fn every_frame(self) -> bool {
        !matches!(self, Kind::Minecraft | Kind::Graphics | Kind::Empty)
    }
}

/// A line's value (`DebugLine.value`): a plain value in the module's value
/// colour, a component with its own colours, a custom line's arguments, or
/// a `DebugLineList`'s strings.
#[derive(Clone, Debug)]
pub(super) enum Val {
    Unset,
    Str(String),
    Comp(Text),
    List(Vec<Text>),
    Strings(Vec<String>),
}

/// One line of a module: its declaration, the config's on/off and what the
/// last update gave it.
pub(super) struct Line {
    pub(super) def: &'static LineDef,
    pub(super) enabled: bool,
    pub(super) active: bool,
    pub(super) reduced: bool,
    pub(super) value: Val,
    /// `DebugLine.format(...)`: a format set by the update.
    pub(super) format: Option<&'static str>,
}

impl Line {
    /// `DebugLine.value(...)`.
    pub(super) fn set(&mut self, v: Val) {
        self.active = true;
        self.value = v;
    }
    pub(super) fn str(&mut self, s: impl Into<String>) {
        self.set(Val::Str(s.into()));
    }
}

/// What a module remembers between updates.
#[derive(Default)]
pub(super) struct Memory {
    /// `CoordsModule`'s last position, velocity and when the position last
    /// changed (game time).
    pub(super) prev_pos: [f64; 3],
    pub(super) velocity: [f64; 3],
    pub(super) pos_time: i64,
    /// `SystemModule.allocationRate`'s last reading.
    pub(super) alloc_at: Option<std::time::Instant>,
    pub(super) alloc_bytes: u64,
    pub(super) alloc_rate: i64,
    /// `DebugRenderer.LAST_UPDATE_TIMES`.
    pub(super) updated: Option<std::time::Instant>,
}

/// A module (`BaseModule` and its subclass's own fields).
pub(super) struct Module {
    pub(super) kind: Kind,
    pub(super) name_color: u32,
    pub(super) value_color: u32,
    pub(super) enabled: bool,
    pub(super) lines: Vec<Line>,
    /// The module's own colours (`Kind::extra_colors`).
    pub(super) extra: Vec<u32>,
    /// `EmptyModule.emptyLines`.
    pub(super) empty_lines: i32,
    /// `SystemModule.memoryColorToggle` and `timeFormat`.
    pub(super) memory_color: bool,
    pub(super) time_format: String,
    pub(super) memory: Memory,
    /// Set when something the lines show changed (`markDirty`).
    pub(super) dirty: bool,
}

pub(super) type ModuleRef = Rc<RefCell<Module>>;

pub(super) const DEFAULT_TIME_FORMAT: &str = "HH:mm:ss";

impl Module {
    pub(super) fn new(kind: Kind) -> Module {
        let (name_color, value_color) = kind.colors();
        let defs = kind.line_defs();
        let count = if kind == Kind::Empty { 20 } else { defs.len() };
        let lines = (0..count)
            .map(|i| {
                let def = &defs[i.min(defs.len() - 1)];
                Line { def, enabled: true, active: true, reduced: def.reduced, value: Val::Unset, format: None }
            })
            .collect();
        Module {
            kind,
            name_color,
            value_color,
            enabled: true,
            lines,
            extra: kind.extra_colors().iter().map(|c| c.default).collect(),
            empty_lines: 1,
            memory_color: true,
            time_format: DEFAULT_TIME_FORMAT.to_owned(),
            memory: Memory::default(),
            dirty: true,
        }
    }

    pub(super) fn shared(kind: Kind) -> ModuleRef {
        Rc::new(RefCell::new(Module::new(kind)))
    }

    /// `BaseModule.line(id)`: the first line with that id.
    pub(super) fn line_mut(&mut self, id: &str) -> Option<&mut Line> {
        self.lines.iter_mut().find(|l| l.def.id == id)
    }

    pub(super) fn extra_color(&self, i: usize) -> u32 {
        self.extra.get(i).copied().unwrap_or(WHITE)
    }

    /// `BaseModule.loadConfig`.
    fn load(&mut self, c: &serde_json::Value) {
        if let Some(lines) = c.get("lines").and_then(|l| l.as_object()) {
            for (id, on) in lines {
                if let (Some(line), Some(on)) = (self.line_mut(id), on.as_bool()) {
                    line.enabled = on;
                }
            }
        }
        let (name, value) = self.kind.colors();
        self.name_color = int(c, "name_color").map_or(name, |v| v as u32 & 0xFF_FFFF);
        self.value_color = int(c, "value_color").map_or(value, |v| v as u32 & 0xFF_FFFF);
        for (i, e) in self.kind.extra_colors().iter().enumerate() {
            self.extra[i] = int(c, e.key).map_or(e.default, |v| v as u32 & 0xFF_FFFF);
        }
        match self.kind {
            Kind::Empty => self.empty_lines = int(c, "empty_lines").map_or(1, |v| v as i32),
            Kind::System => {
                self.memory_color = c.get("memory_color_toggle").and_then(|v| v.as_bool()).unwrap_or(true);
                self.time_format =
                    c.get("time_format").and_then(|v| v.as_str()).unwrap_or(DEFAULT_TIME_FORMAT).to_owned();
            }
            _ => {}
        }
        self.enabled = c.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    }

    /// `BaseModule.saveConfig`.
    fn save(&self) -> serde_json::Value {
        let mut m = serde_json::Map::new();
        m.insert("name".into(), self.kind.id().into());
        m.insert("name_color".into(), self.name_color.into());
        m.insert("value_color".into(), self.value_color.into());
        for (i, e) in self.kind.extra_colors().iter().enumerate() {
            m.insert(e.key.into(), self.extra_color(i).into());
        }
        match self.kind {
            Kind::Empty => {
                m.insert("empty_lines".into(), self.empty_lines.into());
            }
            Kind::System => {
                m.insert("memory_color_toggle".into(), self.memory_color.into());
                m.insert("time_format".into(), self.time_format.clone().into());
            }
            _ => {}
        }
        m.insert("enabled".into(), self.enabled.into());
        let mut lines = serde_json::Map::new();
        for l in &self.lines {
            lines.insert(l.def.id.into(), l.enabled.into());
        }
        m.insert("lines".into(), serde_json::Value::Object(lines));
        serde_json::Value::Object(m)
    }
}

fn int(c: &serde_json::Value, key: &str) -> Option<i64> {
    c.get(key).and_then(serde_json::Value::as_i64)
}

// ---- general options -----------------------------------------------------------------------

/// `GeneralOptions`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct General {
    pub(super) disable_mod: bool,
    pub(super) auto_f3: bool,
    pub(super) space_modules: bool,
    pub(super) shadow_text: bool,
    pub(super) animations: bool,
    pub(super) animation_speed: f64,
    pub(super) font_scale: f64,
    pub(super) background_color: u32,
    pub(super) hide_debug_crosshair: bool,
    pub(super) hide_sidebar: bool,
    pub(super) hide_bossbar: bool,
    pub(super) always_profiler: bool,
    pub(super) always_tps: bool,
    pub(super) always_ping: bool,
    pub(super) performance: bool,
}

pub(super) const DEFAULT_BACKGROUND: u32 = 0x6F50_5050;

impl Default for General {
    fn default() -> General {
        General {
            disable_mod: false,
            auto_f3: false,
            space_modules: false,
            shadow_text: true,
            animations: true,
            animation_speed: 1.0,
            font_scale: 1.0,
            background_color: DEFAULT_BACKGROUND,
            hide_debug_crosshair: false,
            hide_sidebar: true,
            hide_bossbar: true,
            always_profiler: false,
            always_tps: false,
            always_ping: false,
            performance: true,
        }
    }
}

/// Everything the mod keeps: the general options and both columns.
pub(super) struct Config {
    pub(super) general: General,
    pub(super) left: Vec<ModuleRef>,
    pub(super) right: Vec<ModuleRef>,
    pub(super) loaded: bool,
}

/// `BetterF3Fabric.onInitializeClient`: the default order.
fn default_columns() -> (Vec<ModuleRef>, Vec<ModuleRef>) {
    let left = [
        Kind::Minecraft,
        Kind::Fps,
        Kind::Graphics,
        Kind::Server,
        Kind::Coords,
        Kind::Chunks,
        Kind::Location,
        Kind::Entity,
        Kind::Sound,
        Kind::Help,
        Kind::Empty,
        Kind::MiscLeft,
    ];
    let right = [Kind::System, Kind::MiscRight, Kind::Empty, Kind::Target];
    (left.into_iter().map(Module::shared).collect(), right.into_iter().map(Module::shared).collect())
}

thread_local! {
    pub(super) static CONFIG: RefCell<Config> = RefCell::new({
        let (left, right) = default_columns();
        Config { general: General::default(), left, right, loaded: false }
    });
}

/// Runs `f` on the config (None when it is already borrowed).
pub(super) fn with<R>(f: impl FnOnce(&mut Config) -> R) -> Option<R> {
    CONFIG.with(|c| c.try_borrow_mut().ok().map(|mut c| f(&mut c)))
}

pub(super) fn general() -> General {
    with(|c| c.general.clone()).unwrap_or_default()
}

/// `config/betterf3.json` of the Java mod, in the mod's folder.
fn config_path(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join("betterf3.json")
}

/// `ModConfigFile.load`, once, from the mod's folder.
pub(super) fn load(dir: &std::path::Path) {
    let text = std::fs::read(config_path(dir)).ok();
    with(|c| {
        c.loaded = true;
        let Some(json) = text.and_then(|t| serde_json::from_slice::<serde_json::Value>(&t).ok()) else {
            return;
        };
        let column = |key: &str| -> Vec<ModuleRef> {
            json.get(key)
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|m| {
                            let name = m.get("name")?.as_str()?;
                            // `loadModule`: an unknown module is a separator.
                            let mut module = Module::new(Kind::from_id(name).unwrap_or(Kind::Empty));
                            module.load(m);
                            Some(Rc::new(RefCell::new(module)))
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        let (left, right) = (column("modules_left"), column("modules_right"));
        if !left.is_empty() || !right.is_empty() {
            c.left = left;
            c.right = right;
        }
        if let Some(g) = json.get("general") {
            let b = |k: &str, d: bool| g.get(k).and_then(|v| v.as_bool()).unwrap_or(d);
            let f = |k: &str, d: f64| g.get(k).and_then(|v| v.as_f64()).unwrap_or(d);
            c.general = General {
                disable_mod: b("disable_mod", false),
                auto_f3: b("auto_start", false),
                space_modules: b("space_modules", false),
                shadow_text: b("shadow_text", true),
                animations: b("animations", true),
                animation_speed: f("animationSpeed", 1.0),
                font_scale: f("fontScale", 1.0),
                background_color: g.get("background_color").and_then(|v| v.as_i64()).map_or(DEFAULT_BACKGROUND, |v| v as u32),
                hide_debug_crosshair: b("hide_debug_crosshair", false),
                hide_sidebar: b("hide_sidebar", true),
                hide_bossbar: b("hide_bossbar", true),
                always_profiler: b("always_show_profiler", false),
                always_tps: b("always_show_tps", false),
                always_ping: b("always_show_ping", false),
                performance: b("performance_optimizations", true),
            };
        }
    });
}

/// `ModConfigFile.saveRunnable`: both columns and the general options, then
/// every module's lines formatted again.
pub(super) fn save(dir: &std::path::Path) {
    let json = with(|c| {
        let g = &c.general;
        let column = |m: &[ModuleRef]| serde_json::Value::Array(m.iter().map(|m| m.borrow().save()).collect());
        // The background as night-config writes an int: signed.
        let background = g.background_color as i32;
        let general = serde_json::json!({
            "disable_mod": g.disable_mod,
            "auto_start": g.auto_f3,
            "space_modules": g.space_modules,
            "shadow_text": g.shadow_text,
            "animations": g.animations,
            "animationSpeed": g.animation_speed,
            "fontScale": g.font_scale,
            "background_color": background,
            "hide_debug_crosshair": g.hide_debug_crosshair,
            "hide_sidebar": g.hide_sidebar,
            "hide_bossbar": g.hide_bossbar,
            "always_show_profiler": g.always_profiler,
            "always_show_tps": g.always_tps,
            "always_show_ping": g.always_ping,
            "performance_optimizations": g.performance,
        });
        for m in c.left.iter().chain(&c.right) {
            m.borrow_mut().dirty = true;
        }
        serde_json::json!({
            "modules_left": column(&c.left),
            "modules_right": column(&c.right),
            "general": general,
        })
    });
    if let Some(json) = json {
        let _ = std::fs::create_dir_all(dir);
        if let Ok(text) = serde_json::to_string_pretty(&json) {
            let _ = std::fs::write(config_path(dir), text);
        }
    }
}

/// `BaseModule.distinctModules`: one of each module, sorted by name.
pub(super) fn distinct_kinds() -> Vec<Kind> {
    let mut kinds = Kind::ALL.to_vec();
    kinds.sort_by_key(|k| k.name());
    kinds
}
