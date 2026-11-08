// Pick Up Notifier for Horizon: a port of Fuzs's Pick Up Notifier (MPL-2.0,
// github.com/Fuzss/pickupnotifier, branch 26.3.x). Items, experience and
// arrows the player picks up show in a list in a corner of the screen; old
// entries move out and fade, and picking up more of an item revives its entry.
//
// This file is both the mod's Cargo library (its tests) and the module the
// bundle adds to the game (`game::pickupnotifier`). It holds no game types:
// `glue.rs` (`game::pickupnotifier_glue`) reads the game and draws.
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

/// The mod's id (its settings folder), name, version and command.
pub const ID: &str = "pickupnotifier";
pub const NAME: &str = "Pick Up Notifier";
pub const VERSION: &str = "1.1.0";
pub const COMMAND: &str = "pickupnotifier";
pub const DESCRIPTION: &str = "Items, experience and arrows you pick up, listed in a corner of the screen. /pickupnotifier opens the settings.";

/// `DisplayEntry.ELEMENT_HEIGHT` and `TEXT_ITEM_MARGIN`.
pub const ELEMENT_HEIGHT: i32 = 18;
pub const TEXT_ITEM_MARGIN: i32 = 4;
/// `ItemDisplayEntry.ITEM_STACK_POP_TIME`.
pub const POP_TIME: i32 = 5;
/// `Font.lineHeight`.
pub const LINE_HEIGHT: i32 = 9;

// ---------------------------------------------------------------------------
// Options (`ClientConfig`)
// ---------------------------------------------------------------------------

/// `CombineEntries`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Combine {
    Always,
    Never,
    ExcludeNamed,
}

/// `MovementDirection`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Movement {
    None,
    Horizontal,
    Vertical,
    All,
}

/// `ScreenAxis`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Movement {
    /// `MovementDirection.move`: whether entries move out along `axis` from `anchor`.
    pub fn moves(self, axis: Axis, anchor: Anchor) -> bool {
        match self {
            Movement::None => false,
            Movement::Horizontal => axis == Axis::Horizontal && !anchor.is_horizontal_center(),
            Movement::Vertical => axis == Axis::Vertical && !anchor.is_vertical_center(),
            Movement::All => anchor != Anchor::Center,
        }
    }
}

/// `DisplayAmount`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Amount {
    Off,
    Sprite,
    Text,
    Both,
}

impl Amount {
    pub fn is_sprite(self) -> bool {
        matches!(self, Amount::Sprite | Amount::Both)
    }
    pub fn is_text(self) -> bool {
        matches!(self, Amount::Text | Amount::Both)
    }
}

/// `EntryBackground`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Background {
    None,
    Chat,
    Tooltip,
}

/// Puzzles Lib's `AnchorPoint`: where on the screen the list sits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Anchor {
    pub const ALL: [Anchor; 9] = [
        Anchor::TopLeft,
        Anchor::TopCenter,
        Anchor::TopRight,
        Anchor::CenterLeft,
        Anchor::Center,
        Anchor::CenterRight,
        Anchor::BottomLeft,
        Anchor::BottomCenter,
        Anchor::BottomRight,
    ];
    /// `AnchorPoint.normal`: -1 left/top, 0 centre, 1 right/bottom.
    pub fn normal(self) -> (i32, i32) {
        let i = self as i32;
        (i % 3 - 1, i / 3 - 1)
    }
    pub fn is_right(self) -> bool {
        self.normal().0 == 1
    }
    pub fn is_top(self) -> bool {
        self.normal().1 == -1
    }
    pub fn is_horizontal_center(self) -> bool {
        self.normal().0 == 0
    }
    pub fn is_vertical_center(self) -> bool {
        self.normal().1 == 0
    }
}

/// `AnchorPoint.PositionerImpl`: an element of `element` size in an area of
/// `area` size, `pos` in from the anchor's edges.
#[derive(Clone, Copy, Debug)]
pub struct Positioner {
    pub anchor: Anchor,
    pub area: (i32, i32),
    pub element: (i32, i32),
}

/// `Math.round(float)`.
fn round(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

impl Positioner {
    pub fn x(&self, pos: i32) -> i32 {
        let n = self.anchor.normal().0;
        let half = self.area.0 as f32 / 2.0;
        round(half + n as f32 * (half - pos as f32) - ((n + 1) * self.element.0) as f32 / 2.0)
    }
    pub fn y(&self, pos: i32) -> i32 {
        let n = self.anchor.normal().1;
        let half = self.area.1 as f32 / 2.0;
        round(half + n as f32 * (half - pos as f32) - ((n + 1) * self.element.1) as f32 / 2.0)
    }
}

/// `Rarity`, from `item_tooltip::rarity`'s names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
}

impl Rarity {
    pub fn parse(name: &str) -> Rarity {
        match name {
            "uncommon" => Rarity::Uncommon,
            "rare" => Rarity::Rare,
            "epic" => Rarity::Epic,
            _ => Rarity::Common,
        }
    }
    /// The rarity's colour (`ItemHelper.getRarityStyle`); none for common.
    pub fn color(self) -> Option<u32> {
        match self {
            Rarity::Common => None,
            Rarity::Uncommon => Some(0xFFFF55),
            Rarity::Rare => Some(0x55FFFF),
            Rarity::Epic => Some(0xFF55FF),
        }
    }
}

/// The `ChatFormatting` colours `default_text_color` may take.
pub const TEXT_COLORS: [(&str, u32); 16] = [
    ("black", 0x000000),
    ("dark_blue", 0x0000AA),
    ("dark_green", 0x00AA00),
    ("dark_aqua", 0x00AAAA),
    ("dark_red", 0xAA0000),
    ("dark_purple", 0xAA00AA),
    ("gold", 0xFFAA00),
    ("gray", 0xAAAAAA),
    ("dark_gray", 0x555555),
    ("blue", 0x5555FF),
    ("green", 0x55FF55),
    ("aqua", 0x55FFFF),
    ("red", 0xFF5555),
    ("light_purple", 0xFF55FF),
    ("yellow", 0xFFFF55),
    ("white", 0xFFFFFF),
];
const RU_COLORS: [&str; 16] = [
    "Чёрный",
    "Тёмно-синий",
    "Тёмно-зелёный",
    "Тёмно-бирюзовый",
    "Тёмно-красный",
    "Тёмно-фиолетовый",
    "Золотой",
    "Серый",
    "Тёмно-серый",
    "Синий",
    "Зелёный",
    "Бирюзовый",
    "Красный",
    "Светло-фиолетовый",
    "Жёлтый",
    "Белый",
];
const WHITE: usize = 15;

/// The player's options, as the original's client config has them.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub include_items: bool,
    pub include_experience: bool,
    pub include_arrows: bool,
    pub experience_value: bool,
    pub disable_in_creative: bool,
    pub hidden_items: Vec<String>,
    pub combine: Combine,
    pub display_time: i32,
    pub move_out: Movement,
    pub move_time: i32,
    pub fade_out: bool,
    pub draw_sprite: bool,
    pub text_color: usize,
    pub ignore_rarity: bool,
    pub corner: Anchor,
    pub offset_x: i32,
    pub offset_y: i32,
    pub max_height: f64,
    pub display_scale: i32,
    pub amount: Amount,
    pub inventory_count: bool,
    pub single_count: bool,
    pub background: Background,
    pub item_name: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            include_items: true,
            include_experience: true,
            include_arrows: true,
            experience_value: true,
            disable_in_creative: false,
            hidden_items: Vec::new(),
            combine: Combine::ExcludeNamed,
            display_time: 80,
            move_out: Movement::Vertical,
            move_time: 20,
            fade_out: true,
            draw_sprite: true,
            text_color: WHITE,
            ignore_rarity: false,
            corner: Anchor::BottomRight,
            offset_x: 8,
            offset_y: 4,
            max_height: 0.5,
            display_scale: 4,
            amount: Amount::Text,
            inventory_count: false,
            single_count: true,
            background: Background::Chat,
            item_name: true,
        }
    }
}

/// The settings page's buttons, in order; each has its key in the mod's
/// settings folder (the original's config names).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Setting {
    IncludeItems,
    IncludeExperience,
    IncludeArrows,
    ExperienceValue,
    DisableInCreative,
    Combine,
    DisplayTime,
    MoveOut,
    MoveTime,
    FadeOut,
    DrawSprite,
    ItemName,
    TextColor,
    IgnoreRarity,
    Corner,
    OffsetX,
    OffsetY,
    MaxHeight,
    DisplayScale,
    Amount,
    InventoryCount,
    SingleCount,
    Background,
    HiddenItems,
    Reset,
}

pub const PAGE: [Setting; 25] = [
    Setting::IncludeItems,
    Setting::IncludeExperience,
    Setting::IncludeArrows,
    Setting::ExperienceValue,
    Setting::DisableInCreative,
    Setting::Combine,
    Setting::DisplayTime,
    Setting::MoveOut,
    Setting::MoveTime,
    Setting::FadeOut,
    Setting::DrawSprite,
    Setting::ItemName,
    Setting::TextColor,
    Setting::IgnoreRarity,
    Setting::Corner,
    Setting::OffsetX,
    Setting::OffsetY,
    Setting::MaxHeight,
    Setting::DisplayScale,
    Setting::Amount,
    Setting::InventoryCount,
    Setting::SingleCount,
    Setting::Background,
    Setting::HiddenItems,
    Setting::Reset,
];

/// The values a click steps through for the number options.
const DISPLAY_TIMES: [i32; 13] = [0, 20, 40, 60, 80, 100, 120, 160, 200, 300, 400, 600, 1200];
const MOVE_TIMES: [i32; 8] = [0, 5, 10, 15, 20, 30, 40, 60];
const OFFSETS: [i32; 15] = [0, 2, 4, 6, 8, 10, 12, 16, 20, 24, 32, 48, 64, 96, 128];
const HEIGHTS: [f64; 10] = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
const SCALES: [i32; 24] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
];

/// The next value of `values` after `current` (the one before it going
/// `back`), wrapping round; a value the list lacks steps to its neighbour.
fn step<T: Copy + PartialOrd>(values: &[T], current: T, back: bool) -> T {
    if back {
        values
            .iter()
            .rev()
            .find(|v| **v < current)
            .copied()
            .unwrap_or(values[values.len() - 1])
    } else {
        values
            .iter()
            .find(|v| **v > current)
            .copied()
            .unwrap_or(values[0])
    }
}

/// The next of `n` choices after `i`, wrapping round.
fn cycle(i: usize, n: usize, back: bool) -> usize {
    if back { (i + n - 1) % n } else { (i + 1) % n }
}

fn parse_bool(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "true" | "on" | "1" => Some(true),
        "false" | "off" | "0" => Some(false),
        _ => None,
    }
}

/// Seconds for `ticks`: `4`, `0.25`.
fn seconds(ticks: i32) -> String {
    let s = format!("{:.2}", ticks as f64 / 20.0);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

const COMBINES: [Combine; 3] = [Combine::Always, Combine::Never, Combine::ExcludeNamed];
const MOVES: [Movement; 4] = [
    Movement::None,
    Movement::Horizontal,
    Movement::Vertical,
    Movement::All,
];
const AMOUNTS: [Amount; 4] = [Amount::Off, Amount::Sprite, Amount::Text, Amount::Both];
const BACKGROUNDS: [Background; 3] = [Background::None, Background::Chat, Background::Tooltip];

fn index_of<T: PartialEq>(all: &[T], v: &T) -> usize {
    all.iter().position(|x| x == v).unwrap_or(0)
}

/// The stored names of the choices (the original's, in lower case).
fn combine_name(v: Combine) -> &'static str {
    ["always", "never", "exclude_named"][index_of(&COMBINES, &v)]
}
fn move_name(v: Movement) -> &'static str {
    ["none", "horizontal", "vertical", "all"][index_of(&MOVES, &v)]
}
fn amount_name(v: Amount) -> &'static str {
    ["off", "sprite", "text", "both"][index_of(&AMOUNTS, &v)]
}
fn background_name(v: Background) -> &'static str {
    ["none", "chat", "tooltip"][index_of(&BACKGROUNDS, &v)]
}
fn anchor_name(v: Anchor) -> &'static str {
    [
        "top_left",
        "top_center",
        "top_right",
        "center_left",
        "center",
        "center_right",
        "bottom_left",
        "bottom_center",
        "bottom_right",
    ][v as usize]
}

/// `v` among `all` by stored name, any case.
fn by_name<T: Copy>(all: &[T], name: impl Fn(T) -> &'static str, v: &str) -> Option<T> {
    let v = v.trim().to_ascii_lowercase();
    all.iter().copied().find(|x| name(*x) == v)
}

/// Java's `Double.toString` for the plain numbers here: `0.5`, `1.0`.
pub fn java_double(v: f64) -> String {
    let s = format!("{v}");
    if s.contains(['.', 'e', 'i', 'N']) { s } else { format!("{s}.0") }
}

impl Setting {
    /// An enum option's values as stored (the original's constants in lower
    /// case), in the original's order; none for the others.
    pub fn choices(self) -> &'static [&'static str] {
        match self {
            Setting::Combine => &["always", "never", "exclude_named"],
            Setting::MoveOut => &["none", "horizontal", "vertical", "all"],
            Setting::Amount => &["off", "sprite", "text", "both"],
            Setting::Background => &["none", "chat", "tooltip"],
            Setting::Corner => &[
                "top_left",
                "top_center",
                "top_right",
                "center_left",
                "center",
                "center_right",
                "bottom_left",
                "bottom_center",
                "bottom_right",
            ],
            Setting::TextColor => &[
                "black",
                "dark_blue",
                "dark_green",
                "dark_aqua",
                "dark_red",
                "dark_purple",
                "gold",
                "gray",
                "dark_gray",
                "blue",
                "green",
                "aqua",
                "red",
                "light_purple",
                "yellow",
                "white",
            ],
            _ => &[],
        }
    }

    /// Its file in the mod's settings folder; none for the page's actions.
    pub fn key(self) -> Option<&'static str> {
        Some(match self {
            Setting::IncludeItems => "include_items",
            Setting::IncludeExperience => "include_experience",
            Setting::IncludeArrows => "include_arrows",
            Setting::ExperienceValue => "experience_value",
            Setting::DisableInCreative => "disable_in_creative",
            Setting::Combine => "combine_entries",
            Setting::DisplayTime => "display_time",
            Setting::MoveOut => "move_out",
            Setting::MoveTime => "move_time",
            Setting::FadeOut => "fade_out",
            Setting::DrawSprite => "draw_sprite",
            Setting::ItemName => "display_item_name",
            Setting::TextColor => "default_text_color",
            Setting::IgnoreRarity => "ignore_rarity",
            Setting::Corner => "screen_corner",
            Setting::OffsetX => "offset_x",
            Setting::OffsetY => "offset_y",
            Setting::MaxHeight => "max_height",
            Setting::DisplayScale => "display_scale",
            Setting::Amount => "display_amount",
            Setting::InventoryCount => "inventory_count",
            Setting::SingleCount => "display_single_count",
            Setting::Background => "entry_background",
            Setting::HiddenItems => "hidden_items",
            Setting::Reset => return None,
        })
    }

    fn caption(self, ru: bool) -> &'static str {
        let (en, rus) = match self {
            Setting::IncludeItems => ("Include Items", "Предметы"),
            Setting::IncludeExperience => ("Include Experience", "Опыт"),
            Setting::IncludeArrows => ("Include Arrows", "Стрелы и трезубцы"),
            Setting::ExperienceValue => ("Experience Count", "Опыт считать"),
            Setting::DisableInCreative => ("In Creative", "В творческом режиме"),
            Setting::Combine => ("Combine Entries", "Объединять записи"),
            Setting::DisplayTime => ("Display Time", "Время показа"),
            Setting::MoveOut => ("Move Out", "Уход с экрана"),
            Setting::MoveTime => ("Move Time", "Время ухода"),
            Setting::FadeOut => ("Fade Out", "Затухание"),
            Setting::DrawSprite => ("Draw Sprite", "Иконка"),
            Setting::ItemName => ("Display Item Name", "Название"),
            Setting::TextColor => ("Default Text Color", "Цвет текста"),
            Setting::IgnoreRarity => ("Ignore Rarity", "Без цвета редкости"),
            Setting::Corner => ("Screen Corner", "Место на экране"),
            Setting::OffsetX => ("Offset X", "Отступ по X"),
            Setting::OffsetY => ("Offset Y", "Отступ по Y"),
            Setting::MaxHeight => ("Max Height", "Макс. высота"),
            Setting::DisplayScale => ("Display Scale", "Масштаб"),
            Setting::Amount => ("Display Amount", "Количество"),
            Setting::InventoryCount => ("Inventory Count", "Сколько в инвентаре"),
            Setting::SingleCount => ("Display Single Count", "Показывать x1"),
            Setting::Background => ("Entry Background", "Фон"),
            Setting::HiddenItems => ("Hidden Items", "Скрытые предметы"),
            Setting::Reset => ("Reset to Defaults", "Сбросить настройки"),
        };
        if ru { rus } else { en }
    }
}

impl Config {
    /// `DisplayConfig.getDisplayScale`.
    pub fn scale(&self) -> f32 {
        self.display_scale as f32 / 6.0
    }
    /// `BehaviorConfig.getMoveTime`.
    pub fn effective_move_time(&self) -> i32 {
        self.move_time.min(self.display_time)
    }
    /// `default_text_color`'s colour.
    pub fn text_rgb(&self) -> u32 {
        TEXT_COLORS[self.text_color.min(WHITE)].1
    }
    /// The colour of an entry's text: its rarity's, else the text colour.
    pub fn name_rgb(&self, rarity: Rarity) -> u32 {
        match rarity.color() {
            Some(c) if !self.ignore_rarity => c,
            _ => self.text_rgb(),
        }
    }

    /// The options as stored, read through `get` (one file per key); a
    /// missing or unreadable value keeps its default.
    pub fn load(mut get: impl FnMut(&str) -> Option<String>) -> Config {
        let mut c = Config::default();
        for s in PAGE {
            if let Some(key) = s.key()
                && let Some(v) = get(key)
            {
                c.set(s, &v);
            }
        }
        c
    }

    /// Reads one stored value into the option; false when it doesn't parse.
    pub fn set(&mut self, s: Setting, v: &str) -> bool {
        fn num<T: std::str::FromStr>(v: &str) -> Option<T> {
            v.trim().parse().ok()
        }
        let ok = match s {
            Setting::IncludeItems => parse_bool(v).map(|b| self.include_items = b),
            Setting::IncludeExperience => parse_bool(v).map(|b| self.include_experience = b),
            Setting::IncludeArrows => parse_bool(v).map(|b| self.include_arrows = b),
            Setting::ExperienceValue => parse_bool(v).map(|b| self.experience_value = b),
            Setting::DisableInCreative => parse_bool(v).map(|b| self.disable_in_creative = b),
            Setting::Combine => by_name(&COMBINES, combine_name, v).map(|x| self.combine = x),
            Setting::DisplayTime => num::<i32>(v)
                .filter(|n| *n >= 0)
                .map(|n| self.display_time = n),
            Setting::MoveOut => by_name(&MOVES, move_name, v).map(|x| self.move_out = x),
            Setting::MoveTime => num::<i32>(v)
                .filter(|n| *n >= 0)
                .map(|n| self.move_time = n),
            Setting::FadeOut => parse_bool(v).map(|b| self.fade_out = b),
            Setting::DrawSprite => parse_bool(v).map(|b| self.draw_sprite = b),
            Setting::ItemName => parse_bool(v).map(|b| self.item_name = b),
            Setting::TextColor => {
                let v = v.trim().to_ascii_lowercase();
                TEXT_COLORS
                    .iter()
                    .position(|(n, _)| *n == v)
                    .map(|i| self.text_color = i)
            }
            Setting::IgnoreRarity => parse_bool(v).map(|b| self.ignore_rarity = b),
            Setting::Corner => by_name(&Anchor::ALL, anchor_name, v).map(|x| self.corner = x),
            Setting::OffsetX => num::<i32>(v).filter(|n| *n >= 0).map(|n| self.offset_x = n),
            Setting::OffsetY => num::<i32>(v).filter(|n| *n >= 0).map(|n| self.offset_y = n),
            Setting::MaxHeight => num::<f64>(v)
                .filter(|n| (0.0..=1.0).contains(n))
                .map(|n| self.max_height = n),
            Setting::DisplayScale => num::<i32>(v)
                .filter(|n| (1..=24).contains(n))
                .map(|n| self.display_scale = n),
            Setting::Amount => by_name(&AMOUNTS, amount_name, v).map(|x| self.amount = x),
            Setting::InventoryCount => parse_bool(v).map(|b| self.inventory_count = b),
            Setting::SingleCount => parse_bool(v).map(|b| self.single_count = b),
            Setting::Background => {
                by_name(&BACKGROUNDS, background_name, v).map(|x| self.background = x)
            }
            Setting::HiddenItems => {
                self.hidden_items = split_items(v);
                Some(())
            }
            Setting::Reset => None,
        };
        ok.is_some()
    }

    /// The option's value as stored.
    pub fn value(&self, s: Setting) -> String {
        let b = |v: bool| if v { "true" } else { "false" }.to_owned();
        match s {
            Setting::IncludeItems => b(self.include_items),
            Setting::IncludeExperience => b(self.include_experience),
            Setting::IncludeArrows => b(self.include_arrows),
            Setting::ExperienceValue => b(self.experience_value),
            Setting::DisableInCreative => b(self.disable_in_creative),
            Setting::Combine => combine_name(self.combine).to_owned(),
            Setting::DisplayTime => self.display_time.to_string(),
            Setting::MoveOut => move_name(self.move_out).to_owned(),
            Setting::MoveTime => self.move_time.to_string(),
            Setting::FadeOut => b(self.fade_out),
            Setting::DrawSprite => b(self.draw_sprite),
            Setting::ItemName => b(self.item_name),
            Setting::TextColor => TEXT_COLORS[self.text_color.min(WHITE)].0.to_owned(),
            Setting::IgnoreRarity => b(self.ignore_rarity),
            Setting::Corner => anchor_name(self.corner).to_owned(),
            Setting::OffsetX => self.offset_x.to_string(),
            Setting::OffsetY => self.offset_y.to_string(),
            Setting::MaxHeight => java_double(self.max_height),
            Setting::DisplayScale => self.display_scale.to_string(),
            Setting::Amount => amount_name(self.amount).to_owned(),
            Setting::InventoryCount => b(self.inventory_count),
            Setting::SingleCount => b(self.single_count),
            Setting::Background => background_name(self.background).to_owned(),
            Setting::HiddenItems => self.hidden_items.join(","),
            Setting::Reset => String::new(),
        }
    }

    /// The button's label: `Caption: value`.
    pub fn label(&self, s: Setting, ru: bool) -> String {
        let on = |v: bool| -> String {
            match (v, ru) {
                (true, false) => "ON",
                (false, false) => "OFF",
                (true, true) => "Вкл",
                (false, true) => "Выкл",
            }
            .to_owned()
        };
        let pick = |en: &str, rus: &str| if ru { rus.to_owned() } else { en.to_owned() };
        let time = |ticks: i32| {
            if ru {
                format!("{} с", seconds(ticks).replace('.', ","))
            } else {
                format!("{} s", seconds(ticks))
            }
        };
        let value = match s {
            Setting::IncludeItems => on(self.include_items),
            Setting::IncludeExperience => on(self.include_experience),
            Setting::IncludeArrows => on(self.include_arrows),
            Setting::ExperienceValue => {
                if self.experience_value {
                    pick("Points", "Очками")
                } else {
                    pick("Orbs", "Сферами")
                }
            }
            Setting::DisableInCreative => {
                if self.disable_in_creative {
                    pick("Hidden", "Не показывать")
                } else {
                    pick("Shown", "Показывать")
                }
            }
            Setting::Combine => match self.combine {
                Combine::Always => pick("Always", "Всегда"),
                Combine::Never => pick("Never", "Никогда"),
                Combine::ExcludeNamed => pick("Exclude Named", "Кроме переименованных"),
            },
            Setting::DisplayTime if self.display_time == 0 => {
                pick("Until Pushed Out", "Пока не вытеснят")
            }
            Setting::DisplayTime => time(self.display_time),
            Setting::MoveOut => match self.move_out {
                Movement::None => pick("None", "Нет"),
                Movement::Horizontal => pick("Horizontal", "Вбок"),
                Movement::Vertical => pick("Vertical", "Вверх или вниз"),
                Movement::All => pick("All", "Во все стороны"),
            },
            Setting::MoveTime => time(self.move_time),
            Setting::FadeOut => on(self.fade_out),
            Setting::DrawSprite => on(self.draw_sprite),
            Setting::ItemName => on(self.item_name),
            Setting::TextColor => {
                let i = self.text_color.min(WHITE);
                if ru {
                    RU_COLORS[i].to_owned()
                } else {
                    title_case(TEXT_COLORS[i].0)
                }
            }
            Setting::IgnoreRarity => on(self.ignore_rarity),
            Setting::Corner => {
                if ru {
                    [
                        "Слева сверху",
                        "Сверху по центру",
                        "Справа сверху",
                        "Слева по центру",
                        "По центру",
                        "Справа по центру",
                        "Слева снизу",
                        "Снизу по центру",
                        "Справа снизу",
                    ][self.corner as usize]
                        .to_owned()
                } else {
                    title_case(anchor_name(self.corner))
                }
            }
            Setting::OffsetX => self.offset_x.to_string(),
            Setting::OffsetY => self.offset_y.to_string(),
            Setting::MaxHeight => format!("{}%", (self.max_height * 100.0).round() as i32),
            Setting::DisplayScale => format!(
                "{} ({}%)",
                self.display_scale,
                (self.scale() * 100.0).round() as i32
            ),
            Setting::Amount => match self.amount {
                Amount::Off => pick("Off", "Не показывать"),
                Amount::Sprite => pick("Sprite", "На иконке"),
                Amount::Text => pick("Text", "Текстом"),
                Amount::Both => pick("Both", "И там, и там"),
            },
            Setting::InventoryCount => on(self.inventory_count),
            Setting::SingleCount => on(self.single_count),
            Setting::Background => match self.background {
                Background::None => pick("None", "Нет"),
                Background::Chat => pick("Chat", "Как у чата"),
                Background::Tooltip => pick("Tooltip", "Как у подсказки"),
            },
            Setting::HiddenItems => format!("{} (/{COMMAND} hide)", self.hidden_items.len()),
            Setting::Reset => return s.caption(ru).to_owned(),
        };
        format!("{}: {}", s.caption(ru), value)
    }

    /// A click on the option's button: its next value (the previous one
    /// `back`, with Shift held). The settings it changed, to keep.
    pub fn click(&mut self, s: Setting, back: bool) -> Vec<Setting> {
        match s {
            Setting::IncludeItems => self.include_items ^= true,
            Setting::IncludeExperience => self.include_experience ^= true,
            Setting::IncludeArrows => self.include_arrows ^= true,
            Setting::ExperienceValue => self.experience_value ^= true,
            Setting::DisableInCreative => self.disable_in_creative ^= true,
            Setting::Combine => {
                self.combine = COMBINES[cycle(index_of(&COMBINES, &self.combine), 3, back)]
            }
            Setting::DisplayTime => {
                self.display_time = step(&DISPLAY_TIMES, self.display_time, back)
            }
            Setting::MoveOut => {
                self.move_out = MOVES[cycle(index_of(&MOVES, &self.move_out), 4, back)]
            }
            Setting::MoveTime => self.move_time = step(&MOVE_TIMES, self.move_time, back),
            Setting::FadeOut => self.fade_out ^= true,
            Setting::DrawSprite => self.draw_sprite ^= true,
            Setting::ItemName => self.item_name ^= true,
            Setting::TextColor => self.text_color = cycle(self.text_color.min(WHITE), 16, back),
            Setting::IgnoreRarity => self.ignore_rarity ^= true,
            Setting::Corner => self.corner = Anchor::ALL[cycle(self.corner as usize, 9, back)],
            Setting::OffsetX => self.offset_x = step(&OFFSETS, self.offset_x, back),
            Setting::OffsetY => self.offset_y = step(&OFFSETS, self.offset_y, back),
            Setting::MaxHeight => {
                // Compared with some slack: 0.1 steps don't add up exactly.
                let nudged = self.max_height + if back { -1e-6 } else { 1e-6 };
                self.max_height = step(&HEIGHTS, nudged, back);
            }
            Setting::DisplayScale => self.display_scale = step(&SCALES, self.display_scale, back),
            Setting::Amount => {
                self.amount = AMOUNTS[cycle(index_of(&AMOUNTS, &self.amount), 4, back)]
            }
            Setting::InventoryCount => self.inventory_count ^= true,
            Setting::SingleCount => self.single_count ^= true,
            Setting::Background => {
                self.background =
                    BACKGROUNDS[cycle(index_of(&BACKGROUNDS, &self.background), 3, back)]
            }
            // The list is changed with the command; the button only says how.
            Setting::HiddenItems => return Vec::new(),
            Setting::Reset => {
                let hidden = std::mem::take(&mut self.hidden_items);
                *self = Config {
                    hidden_items: hidden,
                    ..Config::default()
                };
                return PAGE
                    .iter()
                    .copied()
                    .filter(|s| s.key().is_some() && *s != Setting::HiddenItems)
                    .collect();
            }
        }
        vec![s]
    }
}

fn title_case(name: &str) -> String {
    name.split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_ascii_uppercase().to_string() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Hidden items (`hidden_items`, Puzzles Lib's `ConfigDataSet`)
// ---------------------------------------------------------------------------

/// The stored list: entries split at commas (or new lines), trimmed.
pub fn split_items(v: &str) -> Vec<String> {
    v.split([',', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// An id with its namespace (`minecraft` when it has none).
pub fn full_id(id: &str) -> String {
    let id = id.trim().to_ascii_lowercase();
    if id.contains(':') {
        id
    } else {
        format!("minecraft:{id}")
    }
}

/// `pattern` with `*` wildcards matching all of `text`.
fn glob(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !text.starts_with(first) || text.len() < first.len() + last.len() || !text.ends_with(last) {
        return false;
    }
    let mut rest = &text[first.len()..text.len() - last.len()];
    for middle in &parts[1..parts.len() - 1] {
        match rest.find(middle) {
            Some(i) => rest = &rest[i + middle.len()..],
            None => return false,
        }
    }
    true
}

/// Whether one entry matches `item`: an id (`minecraft:stone`, `stone`), a
/// pattern (`minecraft:*_shulker_box`, `somemod:*`) or a tag (`#minecraft:logs`,
/// asked through `in_tag(tag, item)`).
fn entry_matches(entry: &str, item: &str, in_tag: &dyn Fn(&str, &str) -> bool) -> bool {
    match entry.strip_prefix('#') {
        Some(tag) => in_tag(&full_id(tag), item),
        None => glob(&full_id(entry), item),
    }
}

/// `ConfigDataSet.contains`: matched by an entry and by none starting with `!`.
pub fn hidden(entries: &[String], item: &str, in_tag: impl Fn(&str, &str) -> bool) -> bool {
    let mut found = false;
    for e in entries {
        if let Some(excluded) = e.strip_prefix('!') {
            if entry_matches(excluded, item, &in_tag) {
                return false;
            }
        } else if !found && entry_matches(e, item, &in_tag) {
            found = true;
        }
    }
    found
}

// ---------------------------------------------------------------------------
// The list (`DrawEntriesHandler.collector`, `AddEntriesHandler`)
// ---------------------------------------------------------------------------

/// What makes two entries the same (`DisplayEntry.equals`).
#[derive(Clone, Debug, PartialEq)]
pub enum Key {
    /// `ItemDisplayEntry`: same item, rarity and enchanted-ness, and the same
    /// hover name unless entries combine always.
    Item {
        id: String,
        rarity: Rarity,
        enchanted: bool,
        name: String,
    },
    /// `ExperienceDisplayEntry`: every one is the same.
    Experience,
}

impl Key {
    pub fn same(&self, other: &Key, combine: Combine) -> bool {
        if combine == Combine::Never {
            return false;
        }
        match (self, other) {
            (Key::Experience, Key::Experience) => true,
            (
                Key::Item {
                    id,
                    rarity,
                    enchanted,
                    name,
                },
                Key::Item {
                    id: id2,
                    rarity: rarity2,
                    enchanted: enchanted2,
                    name: name2,
                },
            ) => {
                id == id2
                    && rarity == rarity2
                    && enchanted == enchanted2
                    && (combine == Combine::Always || name == name2)
            }
            _ => false,
        }
    }
    /// The rarity the entry's text takes its colour from (experience is
    /// uncommon, as the original gives it).
    pub fn rarity(&self) -> Rarity {
        match self {
            Key::Item { rarity, .. } => *rarity,
            Key::Experience => Rarity::Uncommon,
        }
    }
}

/// One line of the list. `payload` is what the game draws (the stack, the name).
#[derive(Clone, Debug)]
pub struct Entry<P> {
    pub key: Key,
    pub amount: u32,
    /// `remainingTicks`.
    pub remaining: i32,
    /// The stack's pop time (items).
    pub pop: i32,
    /// The orb's `tickCount` (experience), for its colour.
    pub ticks: u32,
    pub payload: P,
}

impl<P> Entry<P> {
    pub fn item(key: Key, amount: u32, cfg: &Config, payload: P) -> Self {
        Entry {
            key,
            amount,
            remaining: cfg.display_time,
            pop: POP_TIME,
            ticks: 0,
            payload,
        }
    }
    pub fn experience(amount: u32, ticks: u32, cfg: &Config, payload: P) -> Self {
        Entry {
            key: Key::Experience,
            amount,
            remaining: cfg.display_time,
            pop: 0,
            ticks,
            payload,
        }
    }
    pub fn is_item(&self) -> bool {
        matches!(self.key, Key::Item { .. })
    }
    /// `getRelativeRemainingTicks`: 1 while it shows, down to 0 as it goes
    /// out. With a display time of 0 entries stay until pushed out, fully
    /// shown (the original divides by zero there).
    pub fn relative(&self, partial: f32, cfg: &Config) -> f32 {
        if cfg.display_time == 0 {
            return 1.0;
        }
        let left = self.remaining as f32 - partial;
        let time = cfg.effective_move_time();
        if time <= 0 {
            return if left > 0.0 { 1.0 } else { 0.0 };
        }
        (left / time as f32).clamp(0.0, 1.0)
    }
}

/// The entries, oldest first.
#[derive(Clone, Debug)]
pub struct Log<P> {
    pub entries: Vec<Entry<P>>,
}

impl<P> Default for Log<P> {
    fn default() -> Self {
        Log {
            entries: Vec::new(),
        }
    }
}

/// How many entries fit (`addEntry`'s `maxSize`) on a GUI `gui_height` tall.
pub fn max_size(gui_height: f32, cfg: &Config) -> i32 {
    let scaled = (gui_height / cfg.scale()) as i32;
    (scaled as f64 * cfg.max_height / ELEMENT_HEIGHT as f64) as i32 - 1
}

impl<P> Log<P> {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// `AddEntriesHandler.addEntry`: the same entry merges into the new one
    /// (amounts add up, the time and pop start over); when the list is full
    /// the oldest goes; the new one comes last.
    pub fn add(&mut self, mut entry: Entry<P>, combine: Combine, max_size: i32) {
        if let Some(i) = self
            .entries
            .iter()
            .position(|e| entry.key.same(&e.key, combine))
        {
            let old = self.entries.remove(i);
            entry.amount = entry.amount.saturating_add(old.amount);
        }
        if !self.entries.is_empty() && self.entries.len() as i32 >= max_size {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }
    /// `DrawEntriesHandler.onClientTick`, unpaused: time runs down; spent
    /// entries go unless the display time is 0.
    pub fn tick(&mut self, display_time: i32) {
        for e in &mut self.entries {
            if e.remaining > 0 {
                e.remaining -= 1;
            }
            if e.pop > 0 {
                e.pop -= 1;
            }
            e.ticks = e.ticks.wrapping_add(1);
        }
        if display_time != 0 {
            self.entries.retain(|e| e.remaining > 0);
        }
    }
    /// The entries in drawing order, top to bottom: oldest first at the top
    /// of the screen, newest first elsewhere (the oldest sits at the edge).
    pub fn drawing_order(&self, anchor: Anchor) -> Vec<usize> {
        let n = self.entries.len();
        if anchor.is_top() {
            (0..n).collect()
        } else {
            (0..n).rev().collect()
        }
    }
}

// ---------------------------------------------------------------------------
// Text and layout (`DisplayEntry`, `DrawEntriesHandler.renderPickUpEntries`)
// ---------------------------------------------------------------------------

/// A piece of an entry's text, joined with spaces.
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    /// The item's (or orb's) name.
    Name,
    Literal(String),
}

/// `createTextComponent`: `(64) Stone x5` on the left, `5x Stone (64)` on
/// the right, as the original orders them.
pub fn parts(cfg: &Config, is_item: bool, amount: u32, inventory: u32) -> Vec<Part> {
    let reverse = cfg.corner.is_right();
    let mut out = Vec::new();
    if is_item && cfg.inventory_count {
        out.push(Part::Literal(format!("({inventory})")));
    }
    if cfg.item_name {
        out.push(Part::Name);
    }
    if cfg.amount.is_text() && (amount > 1 || amount == 1 && cfg.single_count) {
        out.push(Part::Literal(if reverse {
            format!("{amount}x")
        } else {
            format!("x{amount}")
        }));
    }
    if reverse {
        out.reverse();
    }
    out
}

/// `getEntryWidth`: the text and, with a sprite, the margin and the 16 pixel icon.
pub fn entry_width(cfg: &Config, text_width: i32) -> i32 {
    if cfg.draw_sprite {
        text_width + if text_width == 0 { 0 } else { TEXT_ITEM_MARGIN } + 16
    } else {
        text_width
    }
}

/// The x of an entry's text, and of its sprite, from the entry's x.
pub fn text_and_sprite_x(cfg: &Config, x: i32, text_width: i32) -> (i32, i32) {
    let mirror = cfg.corner.is_right();
    let text = if mirror || !cfg.draw_sprite {
        x
    } else {
        x + 16 + TEXT_ITEM_MARGIN
    };
    let sprite = if mirror {
        x + text_width + if text_width == 0 { 0 } else { TEXT_ITEM_MARGIN }
    } else {
        x
    };
    (text, sprite)
}

/// Where an entry draws (in the list's scaled pixels) and how opaque.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub alpha: f32,
}

/// `ScreenRectangle` with `intersection`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Rect {
    fn area_in(&self, other: &Rect) -> i32 {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = (self.x + self.w).min(other.x + other.w);
        let bottom = (self.y + self.h).min(other.y + other.h);
        if left < right && top < bottom {
            (right - left) * (bottom - top)
        } else {
            0
        }
    }
}

/// `renderPickUpEntries`: each entry's place, in drawing order. `widths` and
/// `relative` are the entries' widths and relative remaining ticks in that
/// order; `gui` is the GUI's size. Coordinates are in pixels of the list's
/// scale (`Config::scale`), which the drawing applies around (0, 0).
pub fn layout(cfg: &Config, widths: &[i32], relative: &[f32], gui: (f32, f32)) -> Vec<Placement> {
    let scale = cfg.scale();
    let screen = ((gui.0 / scale) as i32, (gui.1 / scale) as i32);
    let anchor = cfg.corner;
    let n = widths.len() as i32;
    let max_width = widths.iter().copied().max().unwrap_or(0);
    let positioner = Positioner {
        anchor,
        area: screen,
        element: (max_width, ELEMENT_HEIGHT * n),
    };
    let offset_x = (cfg.offset_x as f32 / scale) as i32;
    let offset_y = (cfg.offset_y as f32 / scale) as i32;
    let original = Rect {
        x: positioner.x(offset_x),
        y: positioner.y(offset_y),
        w: max_width,
        h: ELEMENT_HEIGHT * n,
    };
    let move_offset = if cfg.move_out.moves(Axis::Vertical, anchor) {
        let gone: f64 = relative.iter().map(|r| 1.0 - f64::from(*r)).sum();
        (gone * ELEMENT_HEIGHT as f64) as i32
    } else {
        0
    };
    let x = positioner.x(offset_x);
    let mut y = positioner.y(offset_y - move_offset);
    let horizontal = cfg.move_out.moves(Axis::Horizontal, anchor);
    let vertical = cfg.move_out.moves(Axis::Vertical, anchor);
    let mut out = Vec::with_capacity(widths.len());
    for (i, &width) in widths.iter().enumerate() {
        let rel = relative.get(i).copied().unwrap_or(1.0);
        let inner = Positioner {
            anchor,
            area: (max_width, -1),
            element: (width, -1),
        };
        let mut ex = x + inner.x(0);
        if horizontal {
            ex = (ex as f64 + max_width as f64 * (1.0 - f64::from(rel)) * anchor.normal().0 as f64)
                as i32;
        }
        let alpha = if !cfg.fade_out {
            1.0
        } else if horizontal || vertical {
            // The part of the entry still inside where the list sits.
            let element = Rect {
                x: ex,
                y,
                w: width,
                h: ELEMENT_HEIGHT,
            };
            let area = element.w * element.h;
            if area <= 0 {
                0.0
            } else {
                (element.area_in(&original) as f32 / area as f32).clamp(0.0, 1.0)
            }
        } else {
            rel
        };
        out.push(Placement { x: ex, y, alpha });
        y += ELEMENT_HEIGHT;
    }
    out
}

/// `ARGB.as8BitChannel`.
pub fn channel(v: f32) -> u8 {
    (v * 255.0).floor().clamp(0.0, 255.0) as u8
}

/// `ExperienceOrb.getIcon` for the entry's amount: the frame of the 4x4 sheet.
pub fn xp_icon(amount: u32) -> u32 {
    const STEPS: [u32; 10] = [3, 7, 17, 37, 73, 149, 307, 617, 1237, 2477];
    STEPS.iter().filter(|s| amount >= **s).count() as u32
}

/// `getExperienceOrbColor`: the orb's pulsing tint, (r, g, b).
pub fn xp_color(ticks: u32) -> (u8, u8, u8) {
    let age = ticks as f32 / 2.0;
    let red = channel((age.sin() + 1.0) * 0.5);
    let blue = channel(((age + std::f32::consts::PI * 4.0 / 3.0).sin() + 1.0) * 0.1);
    (red, 255, blue)
}

/// `DisplayEntryRenderHelper.shortenValue`: 5000 is `5K`.
pub fn shorten(value: u32) -> String {
    match value {
        1_000_000_000.. => format!("{}B", value / 1_000_000_000),
        1_000_000.. => format!("{}M", value / 1_000_000),
        1_000.. => format!("{}K", value / 1_000),
        _ => value.to_string(),
    }
}

/// Whether the amount shows on the sprite (`renderGuiItemDecorations`).
pub fn sprite_count_shown(cfg: &Config, amount: u32) -> bool {
    cfg.amount.is_sprite() && !(amount <= 1 && !cfg.single_count)
}

/// The item's pop when picked up: the scale around (x + 8, y + 12), or none.
pub fn pop_scale(pop: i32, partial: f32) -> Option<(f32, f32)> {
    if pop <= 0 {
        return None;
    }
    let s = 1.0 + (pop as f32 - partial) / POP_TIME as f32;
    Some((1.0 / s, (s + 1.0) / 2.0))
}

// ---------------------------------------------------------------------------
// The command
// ---------------------------------------------------------------------------

/// `/pickupnotifier` and what follows it.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Opens the settings page.
    Open,
    /// Hides an item (the held one when none is named).
    Hide(Option<String>),
    /// Shows it again.
    Show(Option<String>),
    /// Lists the hidden items.
    List,
    /// Empties the list on the screen.
    Clear,
    Help,
}

/// The command, when `line` (without its slash) is this mod's.
pub fn parse_command(line: &str) -> Option<Command> {
    let mut words = line.split_whitespace();
    if words.next()? != COMMAND {
        return None;
    }
    let verb = words.next();
    let arg = words.next().map(str::to_owned);
    Some(match verb {
        None => Command::Open,
        Some("hide") => Command::Hide(arg),
        Some("show") | Some("unhide") => Command::Show(arg),
        Some("hidden") | Some("list") => Command::List,
        Some("clear") => Command::Clear,
        _ => Command::Help,
    })
}

/// Adds `entry` to the list; false when it is there already.
pub fn hide(cfg: &mut Config, entry: &str) -> bool {
    let entry = entry.trim().to_owned();
    if entry.is_empty() || cfg.hidden_items.iter().any(|e| same_entry(e, &entry)) {
        return false;
    }
    cfg.hidden_items.push(entry);
    true
}

/// Takes `entry` off the list; false when it wasn't there.
pub fn show(cfg: &mut Config, entry: &str) -> bool {
    let before = cfg.hidden_items.len();
    cfg.hidden_items.retain(|e| !same_entry(e, entry));
    cfg.hidden_items.len() != before
}

fn same_entry(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        let s = s.trim();
        let (bang, s) = s.strip_prefix('!').map_or(("", s), |r| ("!", r));
        let (hash, s) = s.strip_prefix('#').map_or(("", s), |r| ("#", r));
        format!("{bang}{hash}{}", full_id(s))
    };
    norm(a) == norm(b)
}

pub fn help(ru: bool) -> &'static str {
    if ru {
        "/pickupnotifier: настройки; hide [id]: скрыть предмет (без id: тот, что в руке); show [id]: снова показывать; hidden: список скрытых; clear: очистить список на экране"
    } else {
        "/pickupnotifier: settings; hide [id]: hide an item (the held one without an id); show [id]: show it again; hidden: list hidden items; clear: empty the list on screen"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, name: &str) -> Key {
        Key::Item {
            id: id.into(),
            rarity: Rarity::Common,
            enchanted: false,
            name: name.into(),
        }
    }

    #[test]
    fn defaults_are_the_originals() {
        let c = Config::default();
        assert_eq!(c.display_time, 80);
        assert_eq!(c.effective_move_time(), 20);
        assert_eq!(c.corner, Anchor::BottomRight);
        assert!((c.scale() - 4.0 / 6.0).abs() < 1e-6);
        assert_eq!(c.combine, Combine::ExcludeNamed);
        assert_eq!(c.amount, Amount::Text);
        assert_eq!(c.background, Background::Chat);
        assert_eq!(c.text_rgb(), 0xFFFFFF);
    }

    #[test]
    fn settings_round_trip_through_their_files() {
        let mut c = Config::default();
        c.corner = Anchor::TopLeft;
        c.display_time = 160;
        c.max_height = 0.3;
        c.text_color = 6;
        c.hidden_items = vec!["minecraft:dirt".into(), "#minecraft:logs".into()];
        c.amount = Amount::Both;
        let stored: Vec<(String, String)> = PAGE
            .iter()
            .filter_map(|s| s.key().map(|k| (k.to_owned(), c.value(*s))))
            .collect();
        let back = Config::load(|k| {
            stored
                .iter()
                .find(|(kk, _)| kk == k)
                .map(|(_, v)| v.clone())
        });
        assert_eq!(back, c);
        // Unreadable values keep their defaults; the original's upper case reads.
        let odd = Config::load(|k| match k {
            "display_scale" => Some("99".into()),
            "screen_corner" => Some("TOP_RIGHT".into()),
            "combine_entries" => Some("always".into()),
            _ => None,
        });
        assert_eq!(odd.display_scale, 4);
        assert_eq!(odd.corner, Anchor::TopRight);
        assert_eq!(odd.combine, Combine::Always);
    }

    #[test]
    fn clicks_step_through_values_and_back() {
        let mut c = Config::default();
        assert_eq!(
            c.click(Setting::DisplayTime, false),
            vec![Setting::DisplayTime]
        );
        assert_eq!(c.display_time, 100);
        c.click(Setting::DisplayTime, true);
        c.click(Setting::DisplayTime, true);
        assert_eq!(c.display_time, 60);
        c.display_time = 1200;
        c.click(Setting::DisplayTime, false);
        assert_eq!(c.display_time, 0);
        c.display_time = 70;
        c.click(Setting::DisplayTime, false);
        assert_eq!(c.display_time, 80);
        c.click(Setting::Corner, false);
        assert_eq!(c.corner, Anchor::TopLeft);
        c.click(Setting::Corner, true);
        assert_eq!(c.corner, Anchor::BottomRight);
        c.click(Setting::MaxHeight, false);
        assert!((c.max_height - 0.6).abs() < 1e-9);
        c.max_height = 1.0;
        c.click(Setting::MaxHeight, false);
        assert!((c.max_height - 0.1).abs() < 1e-9);
        c.click(Setting::MaxHeight, true);
        assert!((c.max_height - 1.0).abs() < 1e-9);
        c.click(Setting::DisplayScale, true);
        c.click(Setting::DisplayScale, true);
        c.click(Setting::DisplayScale, true);
        c.click(Setting::DisplayScale, true);
        assert_eq!(c.display_scale, 24);
        c.click(Setting::TextColor, false);
        assert_eq!(c.text_color, 0);
        c.hidden_items.push("minecraft:dirt".into());
        let reset = c.click(Setting::Reset, false);
        assert!(reset.contains(&Setting::DisplayTime) && !reset.contains(&Setting::HiddenItems));
        assert_eq!(c.hidden_items, vec!["minecraft:dirt".to_owned()]);
        assert_eq!(c.display_scale, 4);
        assert!(c.click(Setting::HiddenItems, false).is_empty());
    }

    #[test]
    fn labels_read_well_in_both_languages() {
        let c = Config::default();
        assert_eq!(c.label(Setting::IncludeItems, false), "Include Items: ON");
        assert_eq!(c.label(Setting::DisplayTime, false), "Display Time: 4 s");
        assert_eq!(c.label(Setting::DisplayTime, true), "Время показа: 4 с");
        assert_eq!(c.label(Setting::MoveTime, true), "Время ухода: 1 с");
        assert_eq!(
            c.label(Setting::Corner, false),
            "Screen Corner: Bottom Right"
        );
        assert_eq!(
            c.label(Setting::Corner, true),
            "Место на экране: Справа снизу"
        );
        assert_eq!(
            c.label(Setting::DisplayScale, false),
            "Display Scale: 4 (67%)"
        );
        assert_eq!(c.label(Setting::MaxHeight, true), "Макс. высота: 50%");
        assert_eq!(
            c.label(Setting::TextColor, false),
            "Default Text Color: White"
        );
        assert_eq!(c.label(Setting::Reset, true), "Сбросить настройки");
        let mut c = c;
        c.display_time = 0;
        c.move_time = 5;
        assert_eq!(
            c.label(Setting::DisplayTime, false),
            "Display Time: Until Pushed Out"
        );
        assert_eq!(c.label(Setting::MoveTime, true), "Время ухода: 0,25 с");
    }

    #[test]
    fn hidden_items_match_ids_patterns_tags_and_exclusions() {
        let tags = |tag: &str, item: &str| tag == "minecraft:logs" && item.ends_with("_log");
        let list =
            split_items("dirt, minecraft:*_shulker_box,#logs, !minecraft:oak_log, othermod:*");
        assert!(hidden(&list, "minecraft:dirt", tags));
        assert!(hidden(&list, "minecraft:red_shulker_box", tags));
        assert!(!hidden(&list, "minecraft:shulker_box", tags));
        assert!(hidden(&list, "minecraft:birch_log", tags));
        assert!(!hidden(&list, "minecraft:oak_log", tags));
        assert!(hidden(&list, "othermod:thing", tags));
        assert!(!hidden(&list, "minecraft:stone", tags));
        assert!(glob("a*b*c", "axxbyyc"));
        assert!(!glob("a*b*c", "axxc"));
        assert!(glob("*", "anything"));
    }

    #[test]
    fn hide_and_show_keep_one_entry_each() {
        let mut c = Config::default();
        assert!(hide(&mut c, "stone"));
        assert!(!hide(&mut c, "minecraft:stone"));
        assert!(hide(&mut c, "#minecraft:logs"));
        assert!(show(&mut c, "minecraft:stone"));
        assert!(!show(&mut c, "minecraft:stone"));
        assert!(show(&mut c, "#logs"));
        assert!(c.hidden_items.is_empty());
    }

    #[test]
    fn same_items_merge_and_time_starts_over() {
        let cfg = Config::default();
        let mut log: Log<()> = Log::default();
        log.add(
            Entry::item(item("minecraft:stone", "Stone"), 3, &cfg, ()),
            cfg.combine,
            9,
        );
        log.add(
            Entry::item(item("minecraft:dirt", "Dirt"), 1, &cfg, ()),
            cfg.combine,
            9,
        );
        for _ in 0..30 {
            log.tick(cfg.display_time);
        }
        log.add(
            Entry::item(item("minecraft:stone", "Stone"), 2, &cfg, ()),
            cfg.combine,
            9,
        );
        assert_eq!(log.entries.len(), 2);
        // The merged one moved to the end, with the sum and a fresh time and pop.
        assert_eq!(log.entries[1].amount, 5);
        assert_eq!(log.entries[1].remaining, 80);
        assert_eq!(log.entries[1].pop, POP_TIME);
        assert_eq!(log.entries[0].remaining, 50);
        // A renamed stack stays apart unless entries combine always.
        let named = item("minecraft:stone", "Rock");
        log.add(
            Entry::item(named.clone(), 1, &cfg, ()),
            Combine::ExcludeNamed,
            9,
        );
        assert_eq!(log.entries.len(), 3);
        // Always: the first same item merges in, whatever its name.
        log.add(Entry::item(named, 1, &cfg, ()), Combine::Always, 9);
        assert_eq!(log.entries.len(), 3);
        assert_eq!(log.entries[2].amount, 6);
        // Never: every pickup its own line.
        log.add(Entry::experience(4, 0, &cfg, ()), Combine::Never, 9);
        log.add(Entry::experience(4, 0, &cfg, ()), Combine::Never, 9);
        assert_eq!(log.entries.len(), 5);
    }

    #[test]
    fn a_full_list_drops_its_oldest_and_spent_entries_go() {
        let cfg = Config::default();
        let mut log: Log<u32> = Log::default();
        for i in 0..5 {
            log.add(
                Entry::item(item(&format!("m:{i}"), ""), 1, &cfg, i),
                cfg.combine,
                3,
            );
        }
        assert_eq!(
            log.entries.iter().map(|e| e.payload).collect::<Vec<_>>(),
            vec![2, 3, 4]
        );
        // A max size of 0 or less still keeps the newest.
        log.add(Entry::item(item("m:9", ""), 1, &cfg, 9), cfg.combine, 0);
        assert_eq!(log.entries.len(), 3);
        for _ in 0..80 {
            log.tick(80);
        }
        assert!(log.is_empty());
        // Display time 0: they stay until pushed out.
        let mut forever = Config::default();
        forever.display_time = 0;
        log.add(
            Entry::item(item("m:1", ""), 1, &forever, 1),
            forever.combine,
            3,
        );
        for _ in 0..1000 {
            log.tick(0);
        }
        assert_eq!(log.entries.len(), 1);
        assert_eq!(log.entries[0].relative(0.5, &forever), 1.0);
    }

    #[test]
    fn max_size_follows_the_screen_scale_and_height() {
        let cfg = Config::default();
        // 240 / (4/6) = 360; 360 * 0.5 / 18 = 10; minus 1.
        assert_eq!(max_size(240.0, &cfg), 9);
        let mut big = cfg.clone();
        big.display_scale = 12;
        big.max_height = 1.0;
        assert_eq!(max_size(240.0, &big), 5);
    }

    #[test]
    fn relative_time_runs_out_over_the_move_time() {
        let cfg = Config::default();
        let mut e = Entry::item(item("m:a", ""), 1, &cfg, ());
        assert_eq!(e.relative(0.0, &cfg), 1.0);
        e.remaining = 10;
        assert!((e.relative(0.0, &cfg) - 0.5).abs() < 1e-6);
        assert!((e.relative(0.5, &cfg) - 0.475).abs() < 1e-6);
        e.remaining = 0;
        assert_eq!(e.relative(0.0, &cfg), 0.0);
        let mut instant = cfg.clone();
        instant.move_time = 0;
        e.remaining = 1;
        assert_eq!(e.relative(0.5, &instant), 1.0);
    }

    #[test]
    fn text_parts_follow_the_side() {
        let mut c = Config::default();
        assert_eq!(
            parts(&c, true, 5, 64),
            vec![Part::Literal("5x".into()), Part::Name]
        );
        c.corner = Anchor::BottomLeft;
        c.inventory_count = true;
        assert_eq!(
            parts(&c, true, 5, 64),
            vec![
                Part::Literal("(64)".into()),
                Part::Name,
                Part::Literal("x5".into())
            ]
        );
        // Experience has no inventory count; a single one may hide its x1.
        c.single_count = false;
        assert_eq!(parts(&c, false, 1, 0), vec![Part::Name]);
        c.amount = Amount::Sprite;
        assert_eq!(parts(&c, false, 7, 0), vec![Part::Name]);
    }

    #[test]
    fn widths_and_sprite_side() {
        let mut c = Config::default();
        assert_eq!(entry_width(&c, 30), 50);
        assert_eq!(entry_width(&c, 0), 16);
        // Right side: text first, sprite after it.
        assert_eq!(text_and_sprite_x(&c, 100, 30), (100, 134));
        c.corner = Anchor::TopLeft;
        assert_eq!(text_and_sprite_x(&c, 100, 30), (120, 100));
        c.draw_sprite = false;
        assert_eq!(entry_width(&c, 30), 30);
        assert_eq!(text_and_sprite_x(&c, 100, 30).0, 100);
    }

    #[test]
    fn positioner_places_like_puzzles_lib() {
        let p = Positioner {
            anchor: Anchor::BottomRight,
            area: (480, 360),
            element: (50, 36),
        };
        assert_eq!((p.x(12), p.y(6)), (418, 318));
        let p = Positioner {
            anchor: Anchor::TopLeft,
            ..p
        };
        assert_eq!((p.x(12), p.y(6)), (12, 6));
        let p = Positioner {
            anchor: Anchor::Center,
            ..p
        };
        assert_eq!((p.x(12), p.y(6)), (215, 162));
        // Right-aligned inside the widest entry.
        let inner = Positioner {
            anchor: Anchor::BottomRight,
            area: (50, -1),
            element: (30, -1),
        };
        assert_eq!(inner.x(0), 20);
    }

    #[test]
    fn layout_stacks_newest_on_top_at_the_bottom_right() {
        let cfg = Config::default();
        // GUI 320x240 at scale 4/6: 480x360; offsets 8/4 become 12/6.
        let places = layout(&cfg, &[50, 30], &[1.0, 1.0], (320.0, 240.0));
        assert_eq!(
            places,
            vec![
                Placement {
                    x: 418,
                    y: 318,
                    alpha: 1.0
                },
                Placement {
                    x: 438,
                    y: 336,
                    alpha: 1.0
                },
            ]
        );
        // The bottom (oldest) entry half gone: everything moves down 9, and
        // that entry is half outside the list's place.
        let places = layout(&cfg, &[50, 30], &[1.0, 0.5], (320.0, 240.0));
        assert_eq!(places[0].y, 327);
        assert_eq!(places[1].y, 345);
        assert!((places[1].alpha - 0.5).abs() < 1e-6);
        assert_eq!(places[0].alpha, 1.0);
    }

    #[test]
    fn horizontal_moves_slide_and_fade_by_area() {
        let mut cfg = Config::default();
        cfg.move_out = Movement::Horizontal;
        cfg.corner = Anchor::TopLeft;
        let places = layout(&cfg, &[40], &[0.25], (320.0, 240.0));
        // 40 * 0.75 to the left of 12.
        assert_eq!(places[0].x, -18);
        assert!((places[0].alpha - 0.25).abs() < 1e-6);
        // No movement: the alpha is the relative time.
        cfg.move_out = Movement::None;
        let places = layout(&cfg, &[40], &[0.25], (320.0, 240.0));
        assert_eq!(places[0].x, 12);
        assert!((places[0].alpha - 0.25).abs() < 1e-6);
        cfg.fade_out = false;
        assert_eq!(layout(&cfg, &[40], &[0.25], (320.0, 240.0))[0].alpha, 1.0);
        // A centred list doesn't move along its centred axis.
        assert!(!Movement::Vertical.moves(Axis::Vertical, Anchor::CenterLeft));
        assert!(Movement::All.moves(Axis::Horizontal, Anchor::TopCenter));
        assert!(!Movement::All.moves(Axis::Vertical, Anchor::Center));
    }

    #[test]
    fn experience_icon_colour_and_counts() {
        assert_eq!(xp_icon(1), 0);
        assert_eq!(xp_icon(3), 1);
        assert_eq!(xp_icon(36), 3);
        assert_eq!(xp_icon(37), 4);
        assert_eq!(xp_icon(5000), 10);
        assert_eq!(xp_color(0), (127, 255, 3));
        assert_eq!(shorten(999), "999");
        assert_eq!(shorten(1500), "1K");
        assert_eq!(shorten(2_500_000), "2M");
        assert_eq!(shorten(3_000_000_000), "3B");
        assert_eq!(channel(0.5), 127);
        assert_eq!(channel(1.0), 255);
        let mut c = Config::default();
        c.amount = Amount::Sprite;
        assert!(sprite_count_shown(&c, 1));
        c.single_count = false;
        assert!(!sprite_count_shown(&c, 1));
        assert!(sprite_count_shown(&c, 2));
        let (sx, sy) = pop_scale(5, 0.0).unwrap();
        assert!((sx - 0.5).abs() < 1e-6 && (sy - 1.5).abs() < 1e-6);
        assert_eq!(pop_scale(0, 0.3), None);
    }

    #[test]
    fn the_command_and_its_words() {
        assert_eq!(parse_command("pickupnotifier"), Some(Command::Open));
        assert_eq!(
            parse_command("pickupnotifier hide minecraft:dirt"),
            Some(Command::Hide(Some("minecraft:dirt".into())))
        );
        assert_eq!(
            parse_command("pickupnotifier hide"),
            Some(Command::Hide(None))
        );
        assert_eq!(
            parse_command("pickupnotifier show #logs"),
            Some(Command::Show(Some("#logs".into())))
        );
        assert_eq!(parse_command("pickupnotifier hidden"), Some(Command::List));
        assert_eq!(parse_command("pickupnotifier clear"), Some(Command::Clear));
        assert_eq!(parse_command("pickupnotifier what"), Some(Command::Help));
        assert_eq!(parse_command("pickup"), None);
        assert_eq!(parse_command("tp 0 0 0"), None);
    }
}
