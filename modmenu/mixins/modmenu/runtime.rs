// Mod Menu for Horizon: what needs no game types. The settings
// (`ModMenuConfig`), the mods as the list shows them (`Mod`, its badges and
// icon), the search (`ModSearch`), sorting and the library filter, the counts
// ("Showing 25 mods", " (25 Mods)"), the pause menu's layout with the Mods
// button, and the labels in English and Russian (Mod Menu's own `en_us` and
// `ru_ru`). `modmenu_glue` fills it from the game, `modmenu_view` draws it.
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

pub const ID: &str = "modmenu";
pub const VERSION: &str = "1.1.2";

/// Configure on a mod with a config screen of its own (`config_screen` in its
/// `modmenu.json`): Mod Menu opens the mod's page on Options > Mods, then
/// clicks this button of it, and the mod shows its screen over the page.
pub const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

/// Mod Menu's own credits (its `fabric.mod.json`), and the port's.
pub const AUTHORS: &[&str] = &["Prospector", "haykam821", "gniftygnome", "TerraformersMC"];
pub const CONTRIBUTORS: &[&str] = &[
    "shedaniel", "Draylar", "Mordna", "Madis0", "Yanis48", "LemmaEOF", "fewizz", "XuyuEre", "geniiii", "thiagokenis",
    "Juuxel", "Chloe Dawn", "modmuss50", "Patbox", "Lykrast", "Pyrofab", "UpcraftLP", "swordglowsblue", "hinataaki",
    "LeonXu98", "magneticflux", "vanja-san", "AlexIIL", "masoncook16", "el97", "Hephaestus-Dev", "zabi94", "dexman545",
    "dhouck", "Hambaka", "po-stulate", "FlashyReese", "egeesin", "TheGlitch76", "ludg1e", "williambl", "PepperCode1",
    "ThatTrollzer", "ToffeeMax", "kalucky0", "Maaster", "MartrixX", "NotSteven", "Dolphin 2.1", "ENDERZOMBI102",
    "anatom3000", "OroArmor", "jackassmc", "Vaerian", "RDKRACZ", "Hulenkius", "XfedeX", "spnda", "Jab125", "SolidBlock",
    "Tkain", "nfitzen", "DenaryDev", "legenden", "NaiNonTH", "MagnusHJensen", "Benjamin-Norton", "triphora", "Pyrrha",
    "sakura-ryoko", "lowercasebtw", "tnoctua", "litetex", "Starmoe(Celestia)", "Coredex", "DeportedToHell",
];

/// The game's language is Russian (`ru_ru`).
pub fn russian(language: &str) -> bool {
    language.eq_ignore_ascii_case("ru_ru")
}

// ---- the pause menu (`MixinPauseScreen`) --------------------------------------------

/// `game_menu_button_style`: 0 Insert, 1 Icon; `NONE` when `modify_game_menu` is off.
pub static PAUSE_STYLE: AtomicU8 = AtomicU8::new(1);
pub const NONE: u8 = 255;
/// The mods screen is over the pause menu: the menu isn't drawn.
pub static PAUSE_COVER: AtomicBool = AtomicBool::new(false);
/// The Mods button's id among the pause menu's buttons.
pub const PAUSE_BUTTON: u8 = 200;
/// Where the pause menu put its Mods icon the last time it laid itself out
/// (f32 bits; NaN before).
static PAUSE_ICON_X: AtomicU32 = AtomicU32::new(u32::MAX);
static PAUSE_ICON_Y: AtomicU32 = AtomicU32::new(u32::MAX);

/// The pause menu's server links (or server dialog) button the last time it
/// laid itself out: x, y, width (f32 bits; NaN when it has none).
static PAUSE_LINKS: [AtomicU32; 3] = [const { AtomicU32::new(u32::MAX) }; 3];

/// The pause menu's server links (or server dialog) button: x, y, width.
/// Its tooltip (`menu.custom_options.tooltip`) can reach up over the Mods icon.
pub fn pause_links_button() -> Option<(f32, f32, f32)> {
    let [x, y, w] = [0, 1, 2].map(|i| f32::from_bits(PAUSE_LINKS[i].load(Ordering::Relaxed)));
    (x.is_finite() && y.is_finite() && w.is_finite()).then_some((x, y, w))
}

/// The pause menu's Mods icon (x, y), for drawing its sprite and tooltip.
pub fn pause_icon() -> Option<(f32, f32)> {
    let x = f32::from_bits(PAUSE_ICON_X.load(Ordering::Relaxed));
    let y = f32::from_bits(PAUSE_ICON_Y.load(Ordering::Relaxed));
    (x.is_finite() && y.is_finite() && PAUSE_STYLE.load(Ordering::Relaxed) == 1).then_some((x, y))
}

/// The pause menu grid's corner (`PauseScreen`'s GridLayout aligned at
/// (0.5, 0.25)), `extra` rows below the icons.
fn pause_grid(w: f32, h: f32, extra: u8) -> (f32, f32) {
    (((w - 212.0) * 0.5).floor(), ((h - (166.0 + 24.0 * f32::from(extra))) * 0.25).floor())
}

/// The pause menu's buttons with the Mods button: a fifth icon after the
/// four (Icon), or a full-width row under them that pushes the rest down
/// (Insert).
pub fn pause_buttons(mut v: Vec<(f32, f32, f32, &'static str, u8, bool)>, w: f32, h: f32, extra: u8) -> Vec<(f32, f32, f32, &'static str, u8, bool)> {
    match PAUSE_STYLE.load(Ordering::Relaxed) {
        1 => {
            let (x0, y0) = pause_grid(w, h, extra);
            // Five icons, 20 px and 4 apart, centred: 48 px in, the Mods icon last.
            let (x, y) = (x0 + 48.0 + 4.0 * 24.0, y0 + 98.0);
            PAUSE_ICON_X.store(x.to_bits(), Ordering::Relaxed);
            PAUSE_ICON_Y.store(y.to_bits(), Ordering::Relaxed);
            v.push((x, y, 20.0, "", PAUSE_BUTTON, true));
        }
        0 => {
            let (_, old_y0) = pause_grid(w, h, extra);
            let (x0, y0) = pause_grid(w, h, extra + 1);
            let dy = y0 - old_y0;
            for b in &mut v {
                let below = b.1 > old_y0 + 98.0;
                b.1 += dy + if below { 24.0 } else { 0.0 };
            }
            v.push((x0 + 4.0, y0 + 122.0, 204.0, "horizon.pages.mods", PAUSE_BUTTON, true));
        }
        _ => {}
    }
    let links = v.iter().find(|b| b.4 == 11 || b.4 == 14).map_or([f32::NAN; 3], |b| [b.0, b.1, b.2]);
    for (slot, value) in PAUSE_LINKS.iter().zip(links) {
        slot.store(value.to_bits(), Ordering::Relaxed);
    }
    v
}

/// The pause menu's icon row: 12 px left for the Mods icon (Icon), or down
/// with the grid (Insert).
pub fn pause_icons(mut icons: [(f32, f32, &'static str); 4], w: f32, h: f32, extra: u8) -> [(f32, f32, &'static str); 4] {
    match PAUSE_STYLE.load(Ordering::Relaxed) {
        1 => {
            for i in &mut icons {
                i.0 -= 12.0;
            }
        }
        0 => {
            let dy = pause_grid(w, h, extra + 1).1 - pause_grid(w, h, extra).1;
            for i in &mut icons {
                i.1 += dy;
            }
        }
        _ => {}
    }
    icons
}


// ---- the settings (`ModMenuConfig`) --------------------------------------------------

/// One option of the options screen: its key in the settings, its caption
/// and its values (stored token, English, Russian) in the cycle's order.
pub struct Opt {
    pub key: &'static str,
    pub caption: (&'static str, &'static str),
    pub values: &'static [(&'static str, &'static str, &'static str)],
    pub default: &'static str,
}

const COUNTED: &[(&str, &str, &str)] = &[("true", "Counted", "Учитывать"), ("false", "Not Counted", "Не учитывать")];
const SHOWN_HIDDEN: &[(&str, &str, &str)] = &[("true", "Hidden", "Скрыты"), ("false", "Shown", "Показаны")];
const ENABLED: &[(&str, &str, &str)] = &[("true", "Enabled", "Вкл"), ("false", "Disabled", "Выкл")];

/// The options screen's options, in its order (`ModMenuConfig`'s fields).
pub const OPTIONS: &[Opt] = &[
    Opt { key: "sorting", caption: ("Sort", "Сортировка"), values: &[("ascending", "A-Z", "A-Z"), ("descending", "Z-A", "Z-A"), ("has_update", "Has Update", "Has Update")], default: "ascending" },
    Opt { key: "count_libraries", caption: ("Libraries", "Библиотеки"), values: COUNTED, default: "true" },
    Opt { key: "compact_list", caption: ("List", "Список"), values: &[("true", "Compact", "Компактный"), ("false", "Standard", "Обычный")], default: "false" },
    Opt { key: "count_children", caption: ("Children", "Дочерние моды"), values: COUNTED, default: "true" },
    Opt {
        key: "mods_button_style",
        caption: ("Title Screen", "Кнопка"),
        values: &[("classic", "Below Realms", "Под «Realms»"), ("replace_realms", "Replace Realms", "Вместо «Realms»"), ("shrink", "Adjacent", "Рядом с «Realms»"), ("icon", "Icon", "Отдельный значок")],
        default: "icon",
    },
    Opt { key: "game_menu_button_style", caption: ("Game Menu", "В игре"), values: &[("insert", "Insert", "Посередине"), ("icon", "Icon", "Отдельный значок")], default: "icon" },
    Opt { key: "count_hidden_mods", caption: ("Hidden Mods", "Скрытые моды"), values: COUNTED, default: "true" },
    Opt {
        key: "mod_count_location",
        caption: ("Mod Count", "Счётчик модов"),
        values: &[("title_screen", "Title Corner", "В углу меню"), ("mods_button", "Mods Button", "На кнопке"), ("title_screen_and_mods_button", "Both", "Везде"), ("none", "Neither", "Отключён")],
        default: "title_screen",
    },
    Opt { key: "hide_mod_links", caption: ("Mod Links", "Ссылки мода"), values: SHOWN_HIDDEN, default: "false" },
    Opt { key: "show_libraries", caption: ("Libraries", "Библиотеки"), values: &[("false", "Hidden", "Скрыты"), ("with_config", "With Config", "With Config"), ("true", "Shown", "Показаны")], default: "with_config" },
    Opt { key: "hide_mod_license", caption: ("Mod License", "Лицензия мода"), values: &[("true", "Hidden", "Скрыта"), ("false", "Shown", "Показана")], default: "false" },
    Opt { key: "hide_badges", caption: ("Mod Badges", "Метки модов"), values: SHOWN_HIDDEN, default: "false" },
    Opt { key: "hide_mod_credits", caption: ("Mod Credits", "Авторы мода"), values: SHOWN_HIDDEN, default: "false" },
    Opt { key: "easter_eggs", caption: ("Easter Eggs", "Пасхалки"), values: &[("true", "Enabled", "Включены"), ("false", "Disabled", "Выключены")], default: "true" },
    Opt { key: "random_java_colors", caption: ("Java Color", "Цвет Java"), values: &[("true", "Vendor", "От поставщика"), ("false", "Always Red", "Всегда красный")], default: "false" },
    Opt { key: "translate_names", caption: ("Names", "Локализация названий"), values: &[("true", "Localized", "Вкл"), ("false", "Not Localized", "Выкл")], default: "true" },
    Opt { key: "translate_descriptions", caption: ("Descriptions", "Локализация описаний"), values: &[("true", "Localized", "Вкл"), ("false", "Not Localized", "Выкл")], default: "true" },
    Opt { key: "update_checker", caption: ("Update Checker", "Проверка обновлений"), values: ENABLED, default: "true" },
    Opt { key: "button_update_badge", caption: ("Update Indicator", "Индикатор обновлений"), values: &[("true", "Shown", "Вкл"), ("false", "Hidden", "Выкл")], default: "true" },
    Opt { key: "update_channel", caption: ("Update Channel", "Канал обновлений"), values: &[("alpha", "All", "Все"), ("beta", "Release & Beta", "Релиз и Бета"), ("release", "Release", "Релиз")], default: "release" },
    Opt { key: "quick_configure", caption: ("Quick Configure", "Быстрые настройки"), values: ENABLED, default: "true" },
];

pub fn option(key: &str) -> Option<&'static Opt> {
    OPTIONS.iter().find(|o| o.key == key)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sorting {
    Ascending,
    Descending,
    HasUpdate,
}

/// `show_libraries`: hidden, shown with a config screen, shown.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Libraries {
    Hidden,
    WithConfig,
    Shown,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CountAt {
    TitleScreen,
    ModsButton,
    Both,
    Neither,
}

impl CountAt {
    pub fn title(self) -> bool {
        matches!(self, CountAt::TitleScreen | CountAt::Both)
    }
    pub fn button(self) -> bool {
        matches!(self, CountAt::ModsButton | CountAt::Both)
    }
}

/// What the settings change in Horizon. Children, Java Color and the
/// translation switches are on the options screen as in Mod Menu, but Horizon
/// has no child mods, no Java entry and no translated mod names to apply
/// them to.
#[derive(Clone, Debug)]
pub struct Config {
    pub sorting: Sorting,
    pub count_libraries: bool,
    pub compact_list: bool,
    /// 0 Below Realms, 1 Replace Realms, 2 Adjacent, 3 Icon.
    pub title_style: u8,
    /// 0 Insert, 1 Icon.
    pub game_menu_style: u8,
    pub count_hidden_mods: bool,
    pub count_at: CountAt,
    pub hide_mod_links: bool,
    pub show_libraries: Libraries,
    pub hide_mod_license: bool,
    pub hide_badges: bool,
    pub hide_mod_credits: bool,
    pub easter_eggs: bool,
    pub update_checker: bool,
    pub button_update_badge: bool,
    pub quick_configure: bool,
    // File-only (`@FileOnlyConfig`).
    pub modify_title_screen: bool,
    pub modify_game_menu: bool,
    pub hide_config_buttons: bool,
    pub hidden_mods: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config::read(|_| None)
    }
}

impl Config {
    /// Reads the settings, `get` giving a key's stored token; Mod Menu's
    /// defaults where none is stored or it is not one of the option's values.
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Config {
        let token = |key: &str| -> String {
            let stored = get(key).map(|v| v.trim().to_owned());
            match option(key) {
                Some(o) => stored.filter(|v| o.values.iter().any(|x| x.0 == v)).unwrap_or_else(|| o.default.to_owned()),
                None => stored.unwrap_or_default(),
            }
        };
        let on = |key: &str| token(key) == "true";
        let flag = |key: &str, default: bool| match get(key).as_deref().map(str::trim) {
            Some("true") => true,
            Some("false") => false,
            _ => default,
        };
        let index = |key: &str| -> u8 {
            let t = token(key);
            option(key).and_then(|o| o.values.iter().position(|v| v.0 == t)).unwrap_or(0) as u8
        };
        Config {
            sorting: match token("sorting").as_str() {
                "descending" => Sorting::Descending,
                "has_update" => Sorting::HasUpdate,
                _ => Sorting::Ascending,
            },
            count_libraries: on("count_libraries"),
            compact_list: on("compact_list"),
            title_style: index("mods_button_style"),
            game_menu_style: index("game_menu_button_style"),
            count_hidden_mods: on("count_hidden_mods"),
            count_at: match token("mod_count_location").as_str() {
                "mods_button" => CountAt::ModsButton,
                "title_screen_and_mods_button" => CountAt::Both,
                "none" => CountAt::Neither,
                _ => CountAt::TitleScreen,
            },
            hide_mod_links: on("hide_mod_links"),
            show_libraries: match token("show_libraries").as_str() {
                "false" => Libraries::Hidden,
                "true" => Libraries::Shown,
                _ => Libraries::WithConfig,
            },
            hide_mod_license: on("hide_mod_license"),
            hide_badges: on("hide_badges"),
            hide_mod_credits: on("hide_mod_credits"),
            easter_eggs: on("easter_eggs"),
            update_checker: on("update_checker"),
            button_update_badge: on("button_update_badge"),
            quick_configure: on("quick_configure"),
            modify_title_screen: flag("modify_title_screen", true),
            modify_game_menu: flag("modify_game_menu", true),
            hide_config_buttons: flag("hide_config_buttons", false),
            hidden_mods: get("hidden_mods").map(|v| v.split(',').map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect()).unwrap_or_default(),
        }
    }
}

/// The next value of an option (`CycleButton`; Shift goes back).
pub fn cycle(key: &str, current: &str, back: bool) -> String {
    let Some(o) = option(key) else {
        return current.to_owned();
    };
    let n = o.values.len();
    let i = o.values.iter().position(|v| v.0 == current).unwrap_or(0);
    let next = if back { (i + n - 1) % n } else { (i + 1) % n };
    o.values[next].0.to_owned()
}

// ---- the mods -------------------------------------------------------------------------

/// `Mod.Badge`: its text key, outline and fill (`ModBadgeRenderer`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Badge {
    Library,
    Client,
    Deprecated,
    Forge,
    Modpack,
    Minecraft,
}

impl Badge {
    pub fn text(self, ru: bool) -> &'static str {
        match (self, ru) {
            (Badge::Library, false) => "Library",
            (Badge::Library, true) => "Библиотека",
            (Badge::Client, false) => "Client",
            (Badge::Client, true) => "Клиент",
            (Badge::Deprecated, false) => "Deprecated",
            (Badge::Deprecated, true) => "Устарел",
            (Badge::Forge, _) => "Forge",
            (Badge::Modpack, false) => "Modpack",
            (Badge::Modpack, true) => "Сборка модов",
            (Badge::Minecraft, _) => "Minecraft",
        }
    }
    /// (outline, fill), 0xAARRGGBB.
    pub fn colours(self) -> (u32, u32) {
        match self {
            Badge::Library => (0xff10_7454, 0xff09_3929),
            Badge::Client => (0xff2b_4b7c, 0xff0e_2a55),
            Badge::Deprecated => (0xff84_1426, 0xff53_0c17),
            Badge::Forge => (0xff1f_2d42, 0xff10_1721),
            Badge::Modpack => (0xff7a_2b7c, 0xff51_0d54),
            Badge::Minecraft => (0xff6f_6c6a, 0xff31_302f),
        }
    }
}

/// Where a mod's icon comes from.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Icon {
    Minecraft,
    Horizon,
    ModMenu,
    Unknown,
    /// The catalog's icon (an https URL), fetched and kept.
    Url(String),
    /// The mod's own icon (`icon.png` in its data folder), as Java's comes
    /// from the mod's jar.
    File(String),
}

/// What Configure opens.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Configure {
    None,
    /// A native mod's page on Options > Mods (its id).
    Native(String),
    /// A native mod's own config screen over its page (`MODMENU_CONFIGURE`).
    Screen(String),
    /// Mod Menu's own options.
    ModMenu,
    /// The game's Options screen (Minecraft).
    Options,
    /// Horizon's settings page.
    Horizon,
}

/// An update the catalog has (`UpdateInfo`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Update {
    pub version: String,
    pub url: String,
}

#[derive(Clone, Debug)]
pub struct Mod {
    pub id: String,
    pub name: String,
    /// As the mod gives it ("1.0.1"); shown through `version_text`.
    pub version: String,
    /// The list's second line.
    pub summary: String,
    pub description: String,
    pub authors: Vec<String>,
    pub contributors: Vec<String>,
    pub maintainers: Vec<String>,
    pub licenses: Vec<String>,
    pub website: Option<String>,
    pub issues: Option<String>,
    pub source: Option<String>,
    pub badges: Vec<Badge>,
    pub icon: Icon,
    pub configure: Configure,
    pub update: Option<Update>,
}

impl Mod {
    pub fn has(&self, badge: Badge) -> bool {
        self.badges.contains(&badge)
    }
    pub fn configurable(&self) -> bool {
        self.configure != Configure::None
    }
}

/// `VersionUtil.getPrefixedVersion`: "1.2" → "v1.2", "version3" → "v3".
pub fn version_text(version: &str) -> String {
    let v = version.trim();
    let v = ["version", "ver", "v"].iter().find_map(|p| v.strip_prefix(p)).unwrap_or(v);
    format!("v{v}")
}

/// Whether version `a` is newer than `b`: their numbers compared in turn.
pub fn newer(a: &str, b: &str) -> bool {
    let nums = |s: &str| -> Vec<u64> { s.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).filter_map(|p| p.parse().ok()).collect() };
    nums(a) > nums(b)
}

/// A catalog description (Markdown) as plain text: no emphasis marks, links
/// as their text, headings as lines.
pub fn plain_markdown(md: &str) -> String {
    let mut out = Vec::new();
    for line in md.lines() {
        let line = line.trim_end();
        let line = line.trim_start_matches('#').trim_start();
        let mut s = String::new();
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '[' {
                // [text](url) → text
                if let Some(close) = chars[i + 1..].iter().position(|&x| x == ']') {
                    let end = i + 1 + close;
                    if chars.get(end + 1) == Some(&'(')
                        && let Some(paren) = chars[end + 2..].iter().position(|&x| x == ')')
                    {
                        s.extend(&chars[i + 1..end]);
                        i = end + 2 + paren + 1;
                        continue;
                    }
                }
            }
            if c == '*' || c == '`' || (c == '_' && chars.get(i + 1) == Some(&'_')) {
                i += if c == '_' { 2 } else { 1 };
                continue;
            }
            s.push(c);
            i += 1;
        }
        out.push(s);
    }
    // Paragraphs join; an empty line stays one.
    let text = out.join("\n");
    text.trim().to_owned()
}

// ---- search, sorting, the library filter --------------------------------------------

/// `ModSearch.calculateSearchScore`: 2 a name or id match (three letters or
/// more), 1 a shorter one or a description, summary, author or search-term
/// match, 0 none.
pub fn score(m: &Mod, query: &str, ru: bool) -> u8 {
    let q = query.to_lowercase();
    if q.is_empty() {
        return 1;
    }
    let name = m.name.to_lowercase();
    let id = m.id.to_lowercase();
    if name.contains(&q) || id.contains(&q) {
        return if q.chars().count() >= 3 { 2 } else { 1 };
    }
    if m.description.to_lowercase().contains(&q) || m.summary.to_lowercase().contains(&q) || m.authors.iter().any(|a| a.to_lowercase().contains(&q)) {
        return 1;
    }
    let term = |key: &str| tr(ru, key).contains(q.as_str());
    let hit = (term("terms_library") && m.has(Badge::Library))
        || (term("terms_patchwork") && m.has(Badge::Forge))
        || (term("terms_modpack") && m.has(Badge::Modpack))
        || (term("terms_deprecated") && m.has(Badge::Deprecated))
        || (term("terms_clientside") && m.has(Badge::Client))
        || (term("terms_configurable") && m.configurable())
        || (term("terms_has_update") && m.update.is_some());
    u8::from(hit)
}

/// The library filter hides `m` (`ModMenuConfig.SHOW_LIBRARIES`).
pub fn library_hidden(m: &Mod, libraries: Libraries) -> bool {
    m.has(Badge::Library)
        && match libraries {
            Libraries::Hidden => true,
            Libraries::WithConfig => !m.configurable(),
            Libraries::Shown => false,
        }
}

/// The mods in the list's order (`ModMenuConfig.Sorting`).
pub fn sorted(mods: &[Mod], sorting: Sorting) -> Vec<usize> {
    let mut order: Vec<usize> = (0..mods.len()).collect();
    let name = |i: usize| mods[i].name.to_lowercase();
    match sorting {
        Sorting::Ascending => order.sort_by_key(|&i| name(i)),
        Sorting::Descending => {
            order.sort_by_key(|&i| name(i));
            order.reverse();
        }
        Sorting::HasUpdate => order.sort_by_key(|&i| (mods[i].update.is_none(), name(i))),
    }
    order
}

/// The list's rows (`ModListWidget.filter`): the sorted mods that the search
/// matches and the filters keep, best match first.
pub fn rows(mods: &[Mod], order: &[usize], query: &str, config: &Config, ru: bool) -> Vec<usize> {
    let mut scored: Vec<(u8, usize)> = order
        .iter()
        .copied()
        .filter(|&i| !config.hidden_mods.contains(&mods[i].id) && !library_hidden(&mods[i], config.show_libraries))
        .map(|i| (score(&mods[i], query, ru), i))
        .filter(|(s, _)| *s > 0)
        .collect();
    if !query.is_empty() {
        // Stable: equal scores keep the sorted order.
        scored.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
    }
    scored.into_iter().map(|(_, i)| i).collect()
}

// ---- counts ---------------------------------------------------------------------------

/// `TranslationUtil.translateNumeric`'s key part for a count: ".1" for one,
/// ".n" otherwise (a "12/25" fraction is ".n").
fn plural(visible: usize, total: usize) -> &'static str {
    if visible == total && total == 1 { "1" } else { "n" }
}

fn count_arg(visible: usize, total: usize) -> String {
    if visible == total { total.to_string() } else { format!("{visible}/{total}") }
}

/// "Showing 25 mods", "Showing 12/25 mods and 3 libraries"
/// (`ModsScreen.computeModCountText`, `computeLibraryCountText`).
pub fn showing(ru: bool, mods: (usize, usize), libs: (usize, usize)) -> String {
    let m = count_arg(mods.0, mods.1);
    if libs.1 > 0 {
        let l = count_arg(libs.0, libs.1);
        let key = format!("showing_ml_{}{}", plural(mods.0, mods.1), plural(libs.0, libs.1));
        return tr(ru, &key).replacen("%s", &m, 1).replacen("%s", &l, 1);
    }
    tr(ru, &format!("showing_mods_{}", plural(mods.0, mods.1))).replace("%s", &m)
}

pub fn showing_libraries(ru: bool, libs: (usize, usize)) -> String {
    tr(ru, &format!("showing_libs_{}", plural(libs.0, libs.1))).replace("%s", &count_arg(libs.0, libs.1))
}

/// The mod count Mod Menu shows (`ModMenu.getDisplayedModCount`).
pub fn displayed_count(mods: &[Mod], config: &Config) -> usize {
    mods.iter().filter(|m| (config.count_libraries || !m.has(Badge::Library)) && (config.count_hidden_mods || !config.hidden_mods.contains(&m.id))).count()
}

/// " (25 Mods)" after the title screen's version (`menu.modded` replaced).
pub fn title_count(ru: bool, count: usize, easter_eggs: bool) -> String {
    let key = if easter_eggs && (count == 69 || count == 420) {
        format!("mods_{count}")
    } else if count == 1 {
        "mods_1".to_owned()
    } else {
        "mods_n".to_owned()
    };
    tr(ru, &key).replace("%s", &count.to_string())
}

/// The Mods button's text (`ModMenu.createModsButtonText`): "Mods", "Mods
/// (25 Loaded)", "Mods (25)" beside Realms.
pub fn button_text(ru: bool, count: usize, config: &Config) -> String {
    let base = tr(ru, "title").to_owned();
    if !config.count_at.button() || config.title_style == 3 {
        return base;
    }
    if config.title_style == 2 {
        return format!("{base} {}", tr(ru, "loaded_short").replace("%s", &count.to_string()));
    }
    let key = if config.easter_eggs && (count == 69 || count == 420) { format!("loaded_{count}") } else { "loaded".to_owned() };
    format!("{base} {}", tr(ru, &key).replace("%s", &count.to_string()))
}

// ---- labels ---------------------------------------------------------------------------

/// A label in English or Russian (Mod Menu's `en_us` and `ru_ru`, English
/// where it has no Russian).
pub fn tr(ru: bool, key: &str) -> &'static str {
    let (en, ru_text) = match key {
        "title" => ("Mods", "Моды"),
        "modmenu_name" => ("Mod Menu", "Mod Menu"),
        "modmenu_description" => ("Adds a mod menu to view the list of mods you have installed.", "Удобное меню для просмотра списка установленных модов."),
        "loaded" => ("(%s Loaded)", "(загружено: %s)"),
        "loaded_short" => ("(%s)", "(%s)"),
        "loaded_69" => ("(%s Loaded...nice)", "(загружено: %s... идеально)"),
        "loaded_420" => ("(%s Loaded...blaze it)", "(загружено: %s... огоньку?)"),
        "mods_n" => (" (%s Mods)", " (%s модов)"),
        "mods_1" => (" (%s Mod)", " (%s мод)"),
        "mods_69" => (" (%s Mods...nice)", "(%s модов... идеально)"),
        "mods_420" => (" (%s Mods...blaze it)", " (%s модов... огоньку?)"),
        "toggle_filters" => ("Toggle Filter Options", "Фильтры"),
        "showing_mods_n" => ("Showing %s mods", "Показано %s модов"),
        "showing_mods_1" => ("Showing %s mod", "Показан %s мод"),
        "showing_libs_n" => ("Showing %s libraries", "Показано %s библиотек"),
        "showing_libs_1" => ("Showing %s library", "Показана %s библиотека"),
        "showing_ml_nn" => ("Showing %s mods and %s libraries", "Показано %s модов и %s библиотек"),
        "showing_ml_n1" => ("Showing %s mods and %s library", "Показано %s модов и %s библиотека"),
        "showing_ml_1n" => ("Showing %s mod and %s libraries", "Показан %s мод и %s библиотек"),
        "showing_ml_11" => ("Showing %s mod and %s library", "Показан %s мод и %s библиотека"),
        "mod_id" => ("Mod ID: %s", "ID мода: %s"),
        "by" => ("By %s", "Авторы: %s"),
        "configure" => ("Configure...", "Настройки..."),
        "website" => ("Website", "Сайт"),
        "issues" => ("Issues", "Проблемы"),
        "credits" => ("Credits:", "Создатели:"),
        "view_credits" => ("View Credits", "Показать авторов"),
        "license" => ("License:", "Лицензия:"),
        "links" => ("Links:", "Ссылки:"),
        "source" => ("Source", "Исходный код"),
        "has_update" => ("An update is available:", "Доступно обновление:"),
        "experimental" => ("(Mod Menu update checker is experimental!)", "(проверка обновлений ModMenu экспериментальна)"),
        "update_text" => ("v%s on %s", "v%s на %s"),
        "catalog" => ("Horizon", "Horizon"),
        "role_authors" => ("Authors:", "Авторы:"),
        "role_contributors" => ("Contributors:", "Участники:"),
        "role_maintainers" => ("Maintainers:", "Основные разработчики:"),
        "mods_folder" => ("Open Mods Folder", "Папка с модами"),
        "minecraft_description" => ("The base game.", "Основная игра."),
        "horizon_description" => ("Minecraft: Java Edition rewritten in Rust.", "Minecraft: Java Edition, переписанный на Rust."),
        "options_title" => ("Mod Menu Options", "Настройки списка модов"),
        "key_open" => ("Open Mod Menu", "Открыть список модов"),
        "description" => ("Mod Menu: a list of your mods from the title screen and the pause menu.", "Mod Menu: список модов из главного меню и меню паузы."),
        "terms_library" => ("api library", "апи библиотека"),
        "terms_patchwork" => ("patchwork forge fml", "patchwork forge fml фордж"),
        "terms_modpack" => ("modpack pack", "набор сборка модпак"),
        "terms_deprecated" => ("deprecated outdated old", "устаревший доисторический старый"),
        "terms_clientside" => ("clientside gameside", "клиент клиентский клиентские"),
        "terms_configurable" => ("configurations configs configures configurable options settings", "настройки настраиваемый настраиваемые опции"),
        "terms_has_update" => ("updates version", "обновления версия"),
        _ => ("", ""),
    };
    if ru { ru_text } else { en }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, name: &str) -> Mod {
        Mod {
            id: id.into(),
            name: name.into(),
            version: "1.0.0".into(),
            summary: String::new(),
            description: String::new(),
            authors: Vec::new(),
            contributors: Vec::new(),
            maintainers: Vec::new(),
            licenses: Vec::new(),
            website: None,
            issues: None,
            source: None,
            badges: Vec::new(),
            icon: Icon::Unknown,
            configure: Configure::None,
            update: None,
        }
    }

    #[test]
    fn config_defaults_are_mod_menus() {
        let c = Config::default();
        assert_eq!(c.sorting, Sorting::Ascending);
        assert_eq!((c.title_style, c.game_menu_style), (3, 1));
        assert_eq!(c.count_at, CountAt::TitleScreen);
        assert_eq!(c.show_libraries, Libraries::WithConfig);
        assert!(c.count_libraries && c.easter_eggs && c.update_checker && c.quick_configure && !c.compact_list);
        let c = Config::read(|k| (k == "sorting").then(|| "descending".to_owned()));
        assert_eq!(c.sorting, Sorting::Descending);
        assert_eq!(cycle("sorting", "has_update", false), "ascending");
        assert_eq!(cycle("show_libraries", "false", true), "true");
    }

    #[test]
    fn versions_counts_and_texts() {
        assert_eq!(version_text("1.2"), "v1.2");
        assert_eq!(version_text("v1.2"), "v1.2");
        assert_eq!(version_text("version3"), "v3");
        assert!(newer("1.0.10", "1.0.9") && !newer("1.0.0", "1.0.0"));
        assert_eq!(showing(false, (25, 25), (0, 0)), "Showing 25 mods");
        assert_eq!(showing(false, (12, 25), (3, 3)), "Showing 12/25 mods and 3 libraries");
        assert_eq!(showing(true, (1, 1), (1, 1)), "Показан 1 мод и 1 библиотека");
        assert_eq!(title_count(false, 25, true), " (25 Mods)");
        assert_eq!(title_count(false, 1, true), " (1 Mod)");
        assert_eq!(title_count(true, 69, true), "(69 модов... идеально)");
        let mut c = Config::default();
        assert_eq!(button_text(false, 25, &c), "Mods");
        c.count_at = CountAt::ModsButton;
        c.title_style = 0;
        assert_eq!(button_text(false, 25, &c), "Mods (25 Loaded)");
        c.title_style = 2;
        assert_eq!(button_text(false, 25, &c), "Mods (25)");
    }

    #[test]
    fn search_sorting_and_libraries() {
        let mut lib = m("horizon", "Horizon");
        lib.badges.push(Badge::Library);
        let mut rei = m("roughlyenoughitems", "Roughly Enough Items");
        rei.summary = "recipes".into();
        let mods = vec![m("zoomify", "Zoomify"), rei, m("clientsort", "ClientSort"), lib];
        let order = sorted(&mods, Sorting::Ascending);
        assert_eq!(order, vec![2, 3, 1, 0]);
        let c = Config::default();
        // The library has no config screen: hidden With Config.
        assert_eq!(rows(&mods, &order, "", &c, false), vec![2, 1, 0]);
        assert_eq!(rows(&mods, &order, "zoo", &c, false), vec![0]);
        assert_eq!(rows(&mods, &order, "recipe", &c, false), vec![1]);
        let mut shown = c.clone();
        shown.show_libraries = Libraries::Shown;
        assert_eq!(rows(&mods, &order, "lib", &shown, false), vec![3]);
        assert_eq!(displayed_count(&mods, &c), 4);
    }

    #[test]
    fn markdown_reads_as_text() {
        assert_eq!(plain_markdown("A port of [REI](https://x) by **shedaniel**.\n\n- `@mod`"), "A port of REI by shedaniel.\n\n- @mod");
    }

    #[test]
    fn pause_buttons_make_room() {
        PAUSE_STYLE.store(1, Ordering::Relaxed);
        let icons = pause_icons([(194.0, 124.0, "a"), (218.0, 124.0, "b"), (242.0, 124.0, "c"), (266.0, 124.0, "d")], 480.0, 270.0, 0);
        assert_eq!(icons[0].0, 182.0);
        let b = pause_buttons(Vec::new(), 480.0, 270.0, 0);
        assert_eq!((b[0].0, b[0].1), (278.0, 124.0));
    }
}
