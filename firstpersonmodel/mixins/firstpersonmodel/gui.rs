// First-person Model for Horizon: its config screen as the original builds
// it (`ConfigScreenProvider.CustomConfigScreen`) with tr7zw's TRender (a
// LibGui fork, MIT) in its default VANILLA_MODERN style, redrawn pixel for
// pixel: the 358x246 root panel centred on the screen, a WTabPanel of four
// tabs ("Settings", "Auto Vanilla Hands", "Disable Items", "Show Layers";
// only three fit, the ribbon's `>` reaches the fourth), Done and Reset.
// Clicks act on release, as TRender's do. Spliced into game.rs as
// `firstpersonmodel_gui`.
use super::firstpersonmodel::{self as fpm, Settings};
use super::firstpersonmodel_glue as glue;
use super::firstpersonmodel_sprites::{self as sprites, Sprite};
use super::Game;
use horizon_client::hud::{rgba, HudAtlas, HudBatch};
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use horizon_client::ui;
use std::collections::HashMap;
use std::time::Instant;

/// What the screen asks of `frame`, which has the sound and the clipboard.
pub(super) enum Action {
    Click,
    Paste,
    Copy(String),
}

// ---- texts -----------------------------------------------------------------

/// The original's texts (`assets/firstperson/lang`): English, and Russian
/// where its `ru_ru.json` has the key (the tabs fall back to English there).
fn tr(key: &str, ru: bool) -> &'static str {
    match (key, ru) {
        ("title", false) => "FirstPerson Settings",
        ("title", true) => "Настройки FirstPerson",
        ("tab.settings", _) => "Settings",
        ("tab.autovanillahands", _) => "Auto Vanilla Hands",
        ("tab.disableitems", _) => "Disable Items",
        ("tab.layers", _) => "Show Layers",
        ("xOffset", false) => "Standing x Offset",
        ("xOffset", true) => "Смещение по X стоя",
        ("xOffset.tooltip", false) => "Moves the body relative to the camera back and forth",
        ("xOffset.tooltip", true) => "Смещает тело относительно камеры вперёд/назад",
        ("sneakXOffset", false) => "Sneaking x Offset",
        ("sneakXOffset", true) => "Смещение по X крадучись",
        ("sneakXOffset.tooltip", false) => "Moves the body relative to the camera back and forth while sneaking",
        ("sneakXOffset.tooltip", true) => "Смещает тело относительно камеры вперёд/назад при подкрадывании",
        ("sitXOffset", false) => "Sit X Offset",
        ("sitXOffset", true) => "Смещение по X сидя",
        ("sitXOffset.tooltip", false) => "Moves the body relative to the camera back and forth while sitting",
        ("sitXOffset.tooltip", true) => "Смещает тело относительно камеры вперёд/назад при сидении",
        ("renderStuckFeatures", false) => "Render Arrows & Stings",
        ("renderStuckFeatures", true) => "Показывать стрелы и жала",
        ("renderStuckFeatures.tooltip", false) => "Shows arrows, bee stings or other modded features in first person",
        ("renderStuckFeatures.tooltip", true) => {
            "Отображает стрелы, пчелиные жала и другие объекты в режиме от первого лица"
        }
        ("vanillaHandMode", false) => "Vanilla Hands Mode",
        ("vanillaHandMode", true) => "Режим рук",
        ("vanillaHandMode.tooltip", false) => "Shows the vanilla first person hands",
        ("vanillaHandMode.tooltip", true) => "Показывает стандартные руки из первого лица",
        ("vanillaHandMode.OFF", false) => "Off",
        ("vanillaHandMode.OFF", true) => "Выключены",
        ("vanillaHandMode.ALL", false) => "Always",
        ("vanillaHandMode.ALL", true) => "Всегда",
        ("vanillaHandMode.ALL_DOUBLE", false) => "Always Both Hands",
        ("vanillaHandMode.ALL_DOUBLE", true) => "Всегда две руки",
        ("vanillaHandMode.ITEMS", false) => "Item In Hand",
        ("vanillaHandMode.ITEMS", true) => "Только с предметом",
        ("dynamicMode", false) => "Dynamic Mode",
        ("dynamicMode", true) => "Динамический режим",
        ("dynamicMode.tooltip", false) => "Changes the Vanilla Hands view based on the current view",
        ("dynamicMode.tooltip", true) => "Автоматически меняет вид рук в зависимости от ситуации",
        ("vanillaHandsSkipSwimming", false) => "Disable Vanilla Hands while Swimming",
        ("vanillaHandsSkipSwimming", true) => "Отключить руки при плавании",
        ("vanillaHandsSkipSwimming.tooltip", false) => {
            "Disables the Vanilla Hands while swimming(also ignoring the dynamic mode)"
        }
        ("vanillaHandsSkipSwimming.tooltip", true) => {
            "Не показывать стандартные руки во время плавания (игнорирует динамический режим)"
        }
        _ => "",
    }
}

/// A vanilla text in the game's language (`gui.done`, `controls.reset`).
fn vanilla(key: &str) -> String {
    Text::translate(key, &[]).resolved().plain()
}

// ---- drawing helpers ----------------------------------------------------------------

/// An ARGB colour (as the Java code writes them) as the game packs colours.
fn c(argb: u32) -> u32 {
    rgba((argb >> 16) as u8, (argb >> 8) as u8, argb as u8, (argb >> 24) as u8)
}

fn fill(batch: &mut HudBatch, atlas: &HudAtlas, x: i32, y: i32, w: i32, h: i32, argb: u32) {
    if w > 0 && h > 0 {
        batch.fill(atlas, x as f32, y as f32, w as f32, h as f32, c(argb));
    }
}

fn width(atlas: &HudAtlas, s: &str) -> i32 {
    atlas.font.width(s) as i32
}

fn text(batch: &mut HudBatch, atlas: &HudAtlas, s: &str, x: i32, y: i32, argb: u32, shadow: bool) {
    batch.text(atlas, s, x as f32, y as f32, c(argb), shadow);
}

/// A destination pixel's source pixel in a nine-slice whose edges and centre
/// tile from their start (`blitNineSlicedSprite`).
fn slice(d: i32, dst: i32, src: i32, a: i32, b: i32) -> i32 {
    let (a, b) = (a.min(dst / 2), b.min(dst / 2));
    if d < a {
        d
    } else if d >= dst - b {
        src - (dst - d)
    } else {
        a + (d - a) % (src - a - b).max(1)
    }
}

/// A TRender texture over `w` x `h`, nine-sliced with borders
/// (left, top, right, bottom), in runs of one colour.
#[allow(clippy::too_many_arguments)]
fn sprite(batch: &mut HudBatch, atlas: &HudAtlas, s: &Sprite, border: [i32; 4], x: i32, y: i32, w: i32, h: i32) {
    let (sw, sh) = (i32::from(s.w), i32::from(s.h));
    for dy in 0..h {
        let sy = slice(dy, h, sh, border[1], border[3]);
        let at = |dx: i32| s.px[(sy * sw + slice(dx, w, sw, border[0], border[2])) as usize];
        let mut dx = 0;
        while dx < w {
            let color = at(dx);
            let mut len = 1;
            while dx + len < w && at(dx + len) == color {
                len += 1;
            }
            if color >> 24 != 0 {
                batch.fill(atlas, (x + dx) as f32, (y + dy) as f32, len as f32, 1.0, color);
            }
            dx += len;
        }
    }
}

/// The 18x18 checkboxes (already squeezed from 20x20).
fn checkbox(batch: &mut HudBatch, atlas: &HudAtlas, s: &Sprite, x: i32, y: i32) {
    sprite(batch, atlas, s, [0; 4], x, y, 18, 18);
}

/// A 1-px bevel nine-slice (TRender's scroll bar textures): top and left
/// edge, bottom and right edge, the top-right and bottom-left corner pixel,
/// the inside.
#[allow(clippy::too_many_arguments)]
fn bevel(batch: &mut HudBatch, atlas: &HudAtlas, x: i32, y: i32, w: i32, h: i32, colors: [u32; 4]) {
    let [edge, bottom, corner, inside] = colors;
    fill(batch, atlas, x, y, w - 1, 1, edge);
    fill(batch, atlas, x + w - 1, y, 1, 1, corner);
    fill(batch, atlas, x, y + 1, 1, h - 2, edge);
    fill(batch, atlas, x + 1, y + 1, w - 2, h - 2, inside);
    fill(batch, atlas, x + w - 1, y + 1, 1, h - 2, bottom);
    fill(batch, atlas, x, y + h - 1, 1, 1, corner);
    fill(batch, atlas, x + 1, y + h - 1, w - 1, 1, bottom);
}

const SCROLL_BACKGROUND: [u32; 4] = [0xFF28_2828, 0xFF5D_5D5D, 0xFF2F_2F2F, 0xFF2F_2F2F];
const SCROLL_THUMB: [u32; 4] = [0xFF6C_6C6C, 0xFF21_2121, 0xFF41_4141, 0xFF41_4141];
const SCROLL_THUMB_HOVERED: [u32; 4] = [0xFF5F_6A9D, 0xFF0B_204A, 0xFF32_3F6E, 0xFF32_3F6E];
const SCROLL_THUMB_PRESSED: [u32; 4] = [0xFF6C_6C6C, 0xFF21_2121, 0xFF2F_2F2F, 0xFF2F_2F2F];

/// `BackgroundPainter.VANILLA` (`panel_vanilla_modern`): a black frame
/// around a `0x7A000000` fill.
fn panel(batch: &mut HudBatch, atlas: &HudAtlas, x: i32, y: i32, w: i32, h: i32) {
    fill(batch, atlas, x, y, w, 1, 0xFF00_0000);
    fill(batch, atlas, x, y + h - 1, w, 1, 0xFF00_0000);
    fill(batch, atlas, x, y + 1, 1, h - 2, 0xFF00_0000);
    fill(batch, atlas, x + w - 1, y + 1, 1, h - 2, 0xFF00_0000);
    fill(batch, atlas, x + 1, y + 1, w - 2, h - 2, 0x7A00_0000);
}

/// A tab's background (`selected/unselected_vanilla_modern`, 4-px corners),
/// open at the bottom.
fn tab_background(batch: &mut HudBatch, atlas: &HudAtlas, x: i32, y: i32, w: i32, selected: bool) {
    let a = |selected_alpha: u32, unselected_alpha: u32| {
        if selected { selected_alpha << 24 } else { unselected_alpha << 24 }
    };
    let black = 0xFF00_0000;
    // Row 0.
    fill(batch, atlas, x + 2, y, w - 4, 1, black);
    // Row 1.
    fill(batch, atlas, x + 1, y + 1, 1, 1, black);
    fill(batch, atlas, x + 2, y + 1, w - 4, 1, a(0xAB, 0x6D));
    fill(batch, atlas, x + w - 2, y + 1, 1, 1, black);
    // Rows 2..19: the frame's sides, the inner column, the fill.
    fill(batch, atlas, x, y + 2, 1, 18, black);
    fill(batch, atlas, x + w - 1, y + 2, 1, 18, black);
    for (row, inner, middle) in [
        (2, a(0xBD, 0x7D), a(0xBA, 0x7A)),
        (19, a(0xBD, 0x7D), a(0xBB, 0x7B)),
    ] {
        fill(batch, atlas, x + 1, y + row, 1, 1, inner);
        fill(batch, atlas, x + 2, y + row, w - 4, 1, middle);
        fill(batch, atlas, x + w - 2, y + row, 1, 1, inner);
    }
    fill(batch, atlas, x + 1, y + 3, 1, 16, a(0xBC, 0x7C));
    fill(batch, atlas, x + 2, y + 3, w - 4, 16, a(0xBA, 0x7A));
    fill(batch, atlas, x + w - 2, y + 3, 1, 16, a(0xBC, 0x7C));
}

/// `widget/tab/focus.png`: a rounded 1-px `0xFFFFFFA0` frame, `h` high.
fn focus_frame(batch: &mut HudBatch, atlas: &HudAtlas, x: i32, y: i32, w: i32, h: i32) {
    let col = 0xFFFF_FFA0;
    fill(batch, atlas, x + 2, y, w - 4, 1, col);
    fill(batch, atlas, x + 1, y + 1, 1, 1, col);
    fill(batch, atlas, x + w - 2, y + 1, 1, 1, col);
    fill(batch, atlas, x, y + 2, 1, h - 4, col);
    fill(batch, atlas, x + w - 1, y + 2, 1, h - 4, col);
    fill(batch, atlas, x + 1, y + h - 2, 1, 1, col);
    fill(batch, atlas, x + w - 2, y + h - 2, 1, 1, col);
    fill(batch, atlas, x + 2, y + h - 1, w - 4, 1, col);
}

/// `WButton` (dark textures): the sprite, highlighted when hovered or
/// focused, and its label centred in `0xFFE0E0E0` with a shadow.
#[allow(clippy::too_many_arguments)]
fn button(batch: &mut HudBatch, atlas: &HudAtlas, x: i32, y: i32, w: i32, h: i32, label: &str, lit: bool) {
    let s = if lit { &sprites::BUTTON_HIGHLIGHTED } else { &sprites::BUTTON };
    sprite(batch, atlas, s, [3; 4], x, y, w, h);
    text(batch, atlas, label, x + w / 2 - width(atlas, label) / 2, y + (h - 8) / 2, 0xFFE0_E0E0, true);
}

/// `Math.round(float)`.
fn round(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

// ---- the screen's parts ---------------------------------------------------------------

/// The root panel's fixed size.
const ROOT: (i32, i32) = (358, 246);
/// Root-relative rectangles.
const RIBBON: (i32, i32) = (7, 23);
const CARD: [i32; 4] = [7, 43, 340, 180];
const DONE: [i32; 4] = [7, 223, 48, 16];
const RESET: [i32; 4] = [303, 223, 48, 16];
/// The ribbon's width; a tab is `TAB_WIDTH` plus its title.
const RIBBON_WIDTH: i32 = 340;
const TAB_WIDTH: i32 = 28;
/// The tabs' titles and icons.
const TABS: [(&str, &str); 4] = [
    ("tab.settings", "minecraft:comparator"),
    ("tab.autovanillahands", "minecraft:filled_map"),
    ("tab.disableitems", "minecraft:barrier"),
    ("tab.layers", "minecraft:netherite_helmet"),
];
/// The three int sliders' keys and range.
const SLIDERS: [&str; 3] = ["xOffset", "sneakXOffset", "sitXOffset"];
const SLIDER_MIN: i32 = -40;
const SLIDER_MAX: i32 = 40;
const SLIDER_W: i32 = 180;
/// The options in their rows: three sliders, a toggle, the enum, two toggles.
const TOGGLES: [(&str, usize); 3] = [("renderStuckFeatures", 3), ("dynamicMode", 5), ("vanillaHandsSkipSwimming", 6)];
const ENUM_ROW: usize = 4;
/// The item and layer lists: rows visible, and the search field's limit.
const LIST_ROWS: usize = 7;
const OPTION_ROWS: usize = 9;
const MAX_LENGTH: usize = 16;

fn slider_value(s: &Settings, i: usize) -> i32 {
    match i {
        0 => s.x_offset,
        1 => s.sneak_x_offset,
        _ => s.sit_x_offset,
    }
}

fn set_slider_value(s: &mut Settings, i: usize, v: i32) {
    match i {
        0 => s.x_offset = v,
        1 => s.sneak_x_offset = v,
        _ => s.sit_x_offset = v,
    }
}

fn toggle_value(s: &Settings, i: usize) -> bool {
    match i {
        0 => s.render_stuck_features,
        1 => s.dynamic_mode,
        _ => s.vanilla_hands_skip_swimming,
    }
}

fn flip_toggle(s: &mut Settings, i: usize) {
    match i {
        0 => s.render_stuck_features = !s.render_stuck_features,
        1 => s.dynamic_mode = !s.dynamic_mode,
        _ => s.vanilla_hands_skip_swimming = !s.vanilla_hands_skip_swimming,
    }
}

/// `WAbstractIntSlider`'s thumb x for a value (f32, as TRender computes it).
fn thumb_x(value: i32) -> i32 {
    let value_to_coord = (SLIDER_MAX - SLIDER_MIN) as f32 / (SLIDER_W - 8) as f32;
    let coord_to_value = 1.0 / value_to_coord;
    round(coord_to_value * (value - SLIDER_MIN) as f32)
}

/// `moveSlider`: the value under the pointer at `x` in the slider.
fn slider_at(x: i32) -> i32 {
    let value_to_coord = (SLIDER_MAX - SLIDER_MIN) as f32 / (SLIDER_W - 8) as f32;
    (SLIDER_MIN + round(value_to_coord * (x - 4) as f32)).clamp(SLIDER_MIN, SLIDER_MAX)
}

/// One entry of an item or layer list.
struct Entry {
    /// The item id or the layer's class name (what the settings keep).
    key: String,
    /// The row's label: the item's name, the class's simple name.
    label: String,
    /// What the search matches.
    search: String,
    item: Option<ItemStack>,
}

/// `WTextField`: the search field under a list.
#[derive(Default)]
struct Field {
    text: String,
    /// Cursor and selection anchor, in characters.
    cursor: usize,
    select: usize,
    /// The first character shown.
    scroll: usize,
}

impl Field {
    fn chars(&self) -> Vec<char> {
        self.text.chars().collect()
    }
    fn len(&self) -> usize {
        self.text.chars().count()
    }
    fn slice(&self, from: usize, to: usize) -> String {
        self.text.chars().skip(from).take(to.saturating_sub(from)).collect()
    }
    fn selection(&self) -> (usize, usize) {
        (self.cursor.min(self.select), self.cursor.max(self.select))
    }
    /// Inserts at the cursor, over the selection, within the 16 characters.
    fn insert(&mut self, s: &str) {
        let (a, b) = self.selection();
        let room = MAX_LENGTH.saturating_sub(self.len() - (b - a));
        let s: String = s.chars().filter(|c| !c.is_control()).take(room).collect();
        if s.is_empty() && a == b {
            return;
        }
        let chars = self.chars();
        let mut out: String = chars[..a].iter().collect();
        out.push_str(&s);
        out.extend(&chars[b..]);
        self.text = out;
        self.cursor = a + s.chars().count();
        self.select = self.cursor;
    }
    fn delete(&mut self, forward: bool, word: bool) {
        let (a, b) = self.selection();
        if a != b {
            self.insert("");
            return;
        }
        let target = if word {
            self.word(if forward { 1 } else { -1 })
        } else if forward {
            (self.cursor + 1).min(self.len())
        } else {
            self.cursor.saturating_sub(1)
        };
        let (from, to) = (self.cursor.min(target), self.cursor.max(target));
        if from == to {
            return;
        }
        let chars = self.chars();
        self.text = chars[..from].iter().chain(&chars[to..]).collect();
        self.cursor = from;
        self.select = from;
    }
    /// The next whitespace boundary (`WTextField`'s Ctrl skips).
    fn word(&self, dir: isize) -> usize {
        let chars = self.chars();
        let mut i = self.cursor;
        if dir > 0 {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            while i < chars.len() && !chars[i].is_whitespace() {
                i += 1;
            }
        } else {
            while i > 0 && chars[i - 1].is_whitespace() {
                i -= 1;
            }
            while i > 0 && !chars[i - 1].is_whitespace() {
                i -= 1;
            }
        }
        i
    }
    fn move_to(&mut self, pos: usize, select: bool) {
        self.cursor = pos.min(self.len());
        if !select {
            self.select = self.cursor;
        }
    }
    /// `scrollCursorIntoView`: the cursor stays within the 332 visible pixels.
    fn follow(&mut self, atlas: &HudAtlas) {
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        }
        while self.scroll < self.cursor && width(atlas, &self.slice(self.scroll, self.cursor)) > CARD[2] - 8 {
            self.scroll += 1;
        }
        self.scroll = self.scroll.min(self.len());
    }
}

/// An item list or the layer list, with its search field.
struct List {
    entries: Vec<Entry>,
    /// The filtered entries (indices into `entries`).
    shown: Vec<usize>,
    /// `WScrollBar.value`: the first row.
    scroll: i32,
    /// The toggles as each row was configured (TRender sets a row's state
    /// once, when it is first shown; Reset doesn't redo it).
    toggles: HashMap<usize, bool>,
    field: Field,
}

impl List {
    fn new(entries: Vec<Entry>) -> Self {
        let shown = (0..entries.len()).collect();
        Self { entries, shown, scroll: 0, toggles: HashMap::new(), field: Field::default() }
    }
    fn max_scroll(&self) -> i32 {
        (self.shown.len() as i32 - LIST_ROWS as i32).max(0)
    }
    fn set_scroll(&mut self, v: i32) {
        self.scroll = v.clamp(0, self.max_scroll());
    }
    /// The search's filter (`setFilter` then `layout`, the scroll kept within range).
    fn filter(&mut self) {
        let query = self.field.text.to_lowercase();
        self.shown = (0..self.entries.len())
            .filter(|&i| self.entries[i].search.contains(&query))
            .collect();
        self.set_scroll(self.scroll);
    }
}

/// What a point on the screen is (`root.hit`, deepest widget first).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hit {
    Tab(usize),
    Back,
    Forward,
    /// The options' scroll bar, or a list's (tab 1..3).
    ScrollBar(usize),
    Slider(usize),
    Toggle(usize),
    EnumButton,
    /// An option row away from its widget.
    OptionRow,
    /// An item or layer row: its entry.
    Row(usize, usize),
    Field(usize),
    Done,
    Reset,
    Panel,
}

/// The widget with the keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Tab(usize),
    Back,
    Forward,
    ScrollBar(usize),
    Slider(usize),
    Toggle(usize),
    EnumButton,
    Row(usize, usize),
    Field(usize),
    Done,
    Reset,
}

/// A scroll bar being dragged: the press's y and the value then.
#[derive(Clone, Copy)]
struct Slide {
    anchor: i32,
    value: i32,
}

pub(super) struct Gui {
    pub(super) actions: Vec<Action>,
    pub(super) closed: bool,
    ru: bool,
    in_world: bool,
    tab: usize,
    /// The ribbon's first tab (`WScrollBox.scrollIndex`).
    ribbon: usize,
    lists: [List; 3],
    focus: Option<Focus>,
    /// The widget the pressed button started on (`lastResponder`).
    pressed: Option<Hit>,
    dragging_slider: bool,
    slide: Option<Slide>,
    /// The root panel's corner, as last drawn.
    origin: (i32, i32),
    opened: Instant,
    labels: (String, String),
}

/// The tabs' titles.
fn tab_title(i: usize, ru: bool) -> &'static str {
    tr(TABS[i].0, ru)
}

fn stack(id: &str) -> ItemStack {
    ItemStack { item: id.into(), count: 1, damage: 0, extra: None }
}

impl Gui {
    pub(super) fn new(game: &Game<'_>) -> Self {
        let ru = horizon_client::lang::code().starts_with("ru");
        // `BuiltInRegistries.ITEM`, air included, sorted by id.
        let mut ids: Vec<String> = game.res.items().items.keys().map(|k| k.to_string()).collect();
        if !ids.iter().any(|i| i == "minecraft:air") {
            ids.push("minecraft:air".into());
        }
        ids.sort();
        let names = &game.pack.names;
        let items: Vec<Entry> = ids
            .iter()
            .map(|id| {
                let s = stack(id);
                let label = horizon_client::item_tooltip::name_line(&s, &|k| names.display_get(k));
                Entry {
                    key: id.clone(),
                    search: format!("{id} {label}").to_lowercase(),
                    label,
                    item: Some(s),
                }
            })
            .collect();
        let copy = |entries: &Vec<Entry>| {
            entries
                .iter()
                .map(|e| Entry { key: e.key.clone(), label: e.label.clone(), search: e.search.clone(), item: e.item.clone() })
                .collect::<Vec<_>>()
        };
        let layers = glue::LAYERS
            .iter()
            .map(|class| Entry {
                key: class.to_string(),
                label: class.rsplit('.').next().unwrap_or(class).to_string(),
                search: class.to_lowercase(),
                item: None,
            })
            .collect();
        Self {
            actions: Vec::new(),
            closed: false,
            ru,
            in_world: game.title.is_none() && game.player.is_some(),
            tab: 0,
            ribbon: 0,
            lists: [List::new(copy(&items)), List::new(items), List::new(layers)],
            focus: None,
            pressed: None,
            dragging_slider: false,
            slide: None,
            origin: (0, 0),
            opened: Instant::now(),
            labels: (vanilla("gui.done"), vanilla("controls.reset")),
        }
    }

    // ---- layout -------------------------------------------------------------

    /// The ribbon's children (`WScrollBox.layout`): `<`, the tabs that fit, `>`.
    fn ribbon(&self, atlas: &HudAtlas) -> Vec<(Hit, i32, i32, i32)> {
        let mut out = Vec::new();
        let mut used = 0;
        if self.ribbon > 0 {
            out.push((Hit::Back, used, 18, 18));
            used += 19;
        }
        for i in self.ribbon..TABS.len() {
            let w = TAB_WIDTH + width(atlas, tab_title(i, self.ru));
            if used + w + 1 < RIBBON_WIDTH {
                out.push((Hit::Tab(i), used, w, 20));
                used += w + 1;
            } else {
                out.push((Hit::Forward, used, 18, 18));
                break;
            }
        }
        out
    }

    /// The enum button's x in its row: `(int)(font.width(label) / 4.7)` cells of 5.
    fn enum_x(&self, atlas: &HudAtlas) -> i32 {
        let n = (f64::from(width(atlas, tr("vanillaHandMode", self.ru))) / 4.7) as i32;
        n * 5 + 2
    }

    /// Every widget's root-relative rectangle, in paint order.
    fn widgets(&self, atlas: &HudAtlas) -> Vec<(Hit, [i32; 4])> {
        let mut out = Vec::new();
        for (hit, x, w, h) in self.ribbon(atlas) {
            out.push((hit, [RIBBON.0 + x, RIBBON.1, w, h]));
        }
        let [cx, cy, cw, ch] = CARD;
        if self.tab == 0 {
            out.push((Hit::ScrollBar(0), [cx + cw - 8, cy, 8, ch]));
            for row in 0..7 {
                let (rx, ry) = (cx + 4, cy + 4 + 19 * row as i32);
                out.push((Hit::OptionRow, [rx, ry, 324, if row == ENUM_ROW { 23 } else { 20 }]));
                if row < 3 {
                    out.push((Hit::Slider(row), [rx + 2, ry, SLIDER_W, 18]));
                } else if row == ENUM_ROW {
                    out.push((Hit::EnumButton, [rx + self.enum_x(atlas), ry, 103, 18]));
                } else if let Some(t) = TOGGLES.iter().position(|(_, r)| *r == row) {
                    out.push((Hit::Toggle(t), [rx + 2, ry, SLIDER_W, 18]));
                }
            }
        } else {
            let list = &self.lists[self.tab - 1];
            out.push((Hit::ScrollBar(self.tab), [cx + cw - 8, cy, 8, 140]));
            for row in 0..LIST_ROWS {
                let Some(&entry) = list.shown.get(list.scroll as usize + row) else {
                    break;
                };
                out.push((Hit::Row(self.tab, entry), [cx + 4, cy + 2 + 19 * row as i32, 324, 20]));
            }
            out.push((Hit::Field(self.tab), [cx, cy + 140, cw, 20]));
        }
        out.push((Hit::Done, DONE));
        out.push((Hit::Reset, RESET));
        out
    }

    /// `root.hit`: the last widget (painted on top) under the point.
    fn hit(&self, atlas: &HudAtlas, p: (i32, i32)) -> Hit {
        let inside = |r: [i32; 4]| p.0 >= r[0] && p.0 < r[0] + r[2] && p.1 >= r[1] && p.1 < r[1] + r[3];
        self.widgets(atlas)
            .into_iter()
            .rev()
            .find(|(_, r)| inside(*r))
            .map_or(Hit::Panel, |(h, _)| h)
    }

    fn rect(&self, atlas: &HudAtlas, hit: Hit) -> Option<[i32; 4]> {
        self.widgets(atlas).into_iter().find(|(h, _)| *h == hit).map(|(_, r)| r)
    }

    /// The screen point as root-relative whole pixels.
    fn local(&self, p: (f32, f32)) -> (i32, i32) {
        (p.0.floor() as i32 - self.origin.0, p.1.floor() as i32 - self.origin.1)
    }

    // ---- drawing ------------------------------------------------------------

    pub(super) fn draw(&mut self, game: &Game<'_>, batch: &mut HudBatch) {
        let atlas = &game.gui.atlas;
        let (w, h) = (batch.width.ceil() as i32, batch.height.ceil() as i32);
        // CottonClientScreen.reposition: centred, never scaled.
        self.origin = ((w - ROOT.0) / 2, (h - ROOT.1) / 2);
        let (ox, oy) = self.origin;
        let mouse = self.local(game.cursor);
        // Screen.extractBackground: the blur over the world or the panorama,
        // then the menu background.
        game.native_apply_screen_blur();
        ui::menu_background(batch, atlas, 0.0, 0.0, w as f32, h as f32, self.in_world);
        panel(batch, atlas, ox, oy, ROOT.0, ROOT.1);
        let settings = fpm::settings();
        // The tab ribbon.
        for (hit, x, tw, th) in self.ribbon(atlas) {
            let (x, y) = (ox + RIBBON.0 + x, oy + RIBBON.1);
            match hit {
                Hit::Tab(i) => {
                    let selected = i == self.tab;
                    tab_background(batch, atlas, x, y, tw, selected);
                    if self.focus == Some(Focus::Tab(i)) {
                        focus_frame(batch, atlas, x, y, tw, if selected { 22 } else { 20 });
                    }
                    let color = if selected { 0xFFEE_EEEE } else { 0xFF77_7777 };
                    text(batch, atlas, tab_title(i, self.ru), x + 23, y + 8, color, true);
                    game.draw_item_model(batch, &stack(TABS[i].1), (x + 6) as f32, (y + 3) as f32);
                }
                Hit::Back | Hit::Forward => {
                    let lit = within(mouse, [x - ox, y - oy, tw, th])
                        || self.focus == Some(if hit == Hit::Back { Focus::Back } else { Focus::Forward });
                    button(batch, atlas, x, y, tw, th, if hit == Hit::Back { "<" } else { ">" }, lit);
                }
                _ => {}
            }
        }
        // The card.
        let [cx, cy, cw, ch] = CARD;
        panel(batch, atlas, ox + cx, oy + cy, cw, ch);
        if self.tab == 0 {
            self.draw_options(game, batch, &settings, mouse);
        } else {
            self.draw_list(game, batch, mouse);
        }
        // Done and Reset.
        for (hit, r, label) in [(Hit::Done, DONE, &self.labels.0), (Hit::Reset, RESET, &self.labels.1)] {
            let focused = matches!((hit, self.focus), (Hit::Done, Some(Focus::Done)) | (Hit::Reset, Some(Focus::Reset)));
            button(batch, atlas, ox + r[0], oy + r[1], r[2], r[3], label, within(mouse, r) || focused);
        }
        // The title, over the panel.
        text(batch, atlas, tr("title", self.ru), ox + 8, oy + 6, 0xFFBC_BCBC, true);
        // The tooltip of the widget under the pointer.
        if let Some(tip) = self.tooltip(self.hit(atlas, mouse)) {
            let lines = atlas.font.split(&Text::literal(&tip), 170.0);
            ui::tooltip(batch, atlas, &lines, game.cursor);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_scroll_bar(&self, batch: &mut HudBatch, atlas: &HudAtlas, which: usize, rect: [i32; 4], mouse: (i32, i32), value: i32, count: i32, window: i32) {
        let [x, y, w, h] = rect;
        let (ox, oy) = self.origin;
        bevel(batch, atlas, ox + x, oy + y, w, h, SCROLL_BACKGROUND);
        if count <= 0 {
            return;
        }
        let bar = h - 2;
        let ratio = if window >= count { 1.0 } else { window as f32 / count as f32 };
        let size = ((ratio * bar as f32) as i32).max(6);
        let movable = bar - size;
        let pos = (value as f32 / (count - window).max(1) as f32 * movable as f32) as i32;
        let colors = if self.slide.is_some() && self.focus == Some(Focus::ScrollBar(which)) {
            SCROLL_THUMB_PRESSED
        } else if within(mouse, rect) {
            SCROLL_THUMB_HOVERED
        } else {
            SCROLL_THUMB
        };
        bevel(batch, atlas, ox + x + 1, oy + y + 1 + pos, 6, size, colors);
        if self.focus == Some(Focus::ScrollBar(which)) {
            bevel(batch, atlas, ox + x + 1, oy + y + 1 + pos, 6, size, SCROLL_THUMB_HOVERED);
        }
    }

    fn draw_options(&self, game: &Game<'_>, batch: &mut HudBatch, settings: &Settings, mouse: (i32, i32)) {
        let atlas = &game.gui.atlas;
        let (ox, oy) = self.origin;
        let [cx, cy, cw, ch] = CARD;
        self.draw_scroll_bar(batch, atlas, 0, [cx + cw - 8, cy, 8, ch], mouse, 0, 7, OPTION_ROWS as i32);
        for (hit, r) in self.widgets(atlas) {
            let (x, y) = (ox + r[0], oy + r[1]);
            match hit {
                Hit::Slider(i) => {
                    let value = slider_value(settings, i);
                    let focused = self.focus == Some(Focus::Slider(i));
                    let track = if focused { "gui/sprites/widget/slider_highlighted" } else { "gui/sprites/widget/slider" };
                    ui::nine_slice(batch, atlas, track, x as f32, y as f32, SLIDER_W as f32, 18.0, u32::MAX);
                    let tx = thumb_x(value);
                    let (mx, my) = (mouse.0 - r[0], mouse.1 - r[1]);
                    let over_thumb = mx >= tx && mx <= tx + 8 && (0..=18).contains(&my);
                    let handle = if (self.dragging_slider && focused) || over_thumb {
                        &sprites::SLIDER_HANDLE_HIGHLIGHTED
                    } else {
                        &sprites::SLIDER_HANDLE
                    };
                    sprite(batch, atlas, handle, [2, 2, 2, 3], x + tx, y, 8, 18);
                    let label = format!("{}: {}", tr(SLIDERS[i], self.ru), value);
                    let hover = (0..=SLIDER_W).contains(&mx) && (0..=18).contains(&my);
                    let color = if hover { 0xFFFF_FFA0 } else { 0xFFE0_E0E0 };
                    text(batch, atlas, &label, x + 2 + (176 / 2 - width(atlas, &label) / 2), y + 5, color, true);
                }
                Hit::Toggle(t) => {
                    let on = toggle_value(settings, t);
                    checkbox(batch, atlas, if on { &sprites::CHECKBOX_SELECTED } else { &sprites::CHECKBOX }, x, y);
                    if self.focus == Some(Focus::Toggle(t)) {
                        checkbox(batch, atlas, &sprites::CHECKBOX_HIGHLIGHTED, x, y);
                    }
                    text(batch, atlas, tr(TOGGLES[t].0, self.ru), x + 22, y + 6, 0xFFBC_BCBC, false);
                }
                Hit::EnumButton => {
                    // The label first (row (2, 5)), then the button over it.
                    let row_x = ox + cx + 4;
                    text(batch, atlas, tr("vanillaHandMode", self.ru), row_x + 2, y + 5, 0xFFBC_BCBC, true);
                    let key = format!("vanillaHandMode.{}", settings.vanilla_hands_mode.name());
                    let lit = within(mouse, r) || self.focus == Some(Focus::EnumButton);
                    button(batch, atlas, x, y, r[2], r[3], tr(&key, self.ru), lit);
                }
                _ => {}
            }
        }
    }

    fn draw_list(&self, game: &Game<'_>, batch: &mut HudBatch, mouse: (i32, i32)) {
        let atlas = &game.gui.atlas;
        let (ox, oy) = self.origin;
        let list = &self.lists[self.tab - 1];
        let [cx, cy, cw, _] = CARD;
        self.draw_scroll_bar(
            batch,
            atlas,
            self.tab,
            [cx + cw - 8, cy, 8, 140],
            mouse,
            list.scroll,
            list.shown.len() as i32,
            LIST_ROWS as i32,
        );
        let settings = fpm::settings();
        for (hit, r) in self.widgets(atlas) {
            let (x, y) = (ox + r[0], oy + r[1]);
            match hit {
                Hit::Row(tab, entry) => {
                    let e = &list.entries[entry];
                    let on = list.toggles.get(&entry).copied().unwrap_or_else(|| initial(&settings, tab, &e.key));
                    checkbox(batch, atlas, if on { &sprites::CHECKBOX_SELECTED } else { &sprites::CHECKBOX }, x, y);
                    if self.focus == Some(Focus::Row(tab, entry)) {
                        checkbox(batch, atlas, &sprites::CHECKBOX_HIGHLIGHTED, x, y);
                    }
                    let label_x = match &e.item {
                        Some(item) => {
                            game.draw_item_model(batch, item, (x + 22) as f32, (y + 1) as f32);
                            x + 40
                        }
                        None => x + 22,
                    };
                    text(batch, atlas, &e.label, label_x, y + 6, 0xFFBC_BCBC, false);
                }
                Hit::Field(_) => self.draw_field(batch, atlas, &list.field, x, y, r[2], r[3]),
                _ => {}
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_field(&self, batch: &mut HudBatch, atlas: &HudAtlas, f: &Field, x: i32, y: i32, w: i32, h: i32) {
        let focused = self.focus == Some(Focus::Field(self.tab));
        fill(batch, atlas, x - 1, y - 1, w + 2, h + 2, if focused { 0xFFFF_FFA0 } else { 0xFFA0_A0A0 });
        fill(batch, atlas, x, y, w, h, 0xFF00_0000);
        // plainSubstrByWidth(text.substring(scrollOffset), w - 8).
        let mut visible = String::new();
        for ch in f.text.chars().skip(f.scroll) {
            visible.push(ch);
            if width(atlas, &visible) > w - 8 {
                visible.pop();
                break;
            }
        }
        text(batch, atlas, &visible, x + 4, y + 6, 0xFFE0_E0E0, true);
        let shown = visible.chars().count();
        let at = |i: usize| width(atlas, &visible.chars().take(i.saturating_sub(f.scroll).min(shown)).collect::<String>());
        if focused {
            let ticks = self.opened.elapsed().as_millis() / 50;
            if (ticks / 6) % 2 == 1 {
                fill(batch, atlas, x + 4 + at(f.cursor), y + 4, 1, 12, 0xFFD0_D0D0);
            }
            let (a, b) = f.selection();
            if a != b {
                let (l, r) = (at(a), at(b));
                batch.text_highlight(atlas, ((x + 4 + l) as f32, (y + 4) as f32, (r - l) as f32, 12.0), false);
            }
        }
    }

    /// The tooltip of the widget under the pointer (`font.split(…, 170)`).
    fn tooltip(&self, hit: Hit) -> Option<String> {
        let key = match hit {
            Hit::Slider(i) => format!("{}.tooltip", SLIDERS[i]),
            Hit::Toggle(t) => format!("{}.tooltip", TOGGLES[t].0),
            Hit::EnumButton => "vanillaHandMode.tooltip".into(),
            Hit::Row(tab, entry) => return Some(self.lists[tab - 1].entries[entry].key.clone()),
            _ => return None,
        };
        Some(tr(&key, self.ru).to_string()).filter(|t| !t.is_empty())
    }

    // ---- input --------------------------------------------------------------

    fn click(&mut self) {
        self.actions.push(Action::Click);
    }

    /// `checkFocus`, then `onMouseDown` on the widget under the pointer.
    pub(super) fn press(&mut self, game: &Game<'_>, p: (f32, f32)) {
        let atlas = &game.gui.atlas;
        let p = self.local(p);
        if let Some(f) = self.focus {
            let rect = self.rect(atlas, focus_hit(f));
            if rect.is_none_or(|r| !within(p, r)) {
                self.focus = None;
            }
        }
        let hit = self.hit(atlas, p);
        self.pressed = Some(hit);
        match hit {
            Hit::Slider(i) => self.focus = Some(Focus::Slider(i)),
            Hit::ScrollBar(which) => {
                self.focus = Some(Focus::ScrollBar(which));
                let value = if which == 0 { 0 } else { self.lists[which - 1].scroll };
                self.slide = Some(Slide { anchor: p.1, value });
            }
            _ => {}
        }
    }

    /// `onMouseDrag` on the pressed widget.
    pub(super) fn drag(&mut self, game: &Game<'_>, p: (f32, f32)) {
        let atlas = &game.gui.atlas;
        let p = self.local(p);
        match self.pressed {
            Some(Hit::Slider(i)) if self.focus == Some(Focus::Slider(i)) => {
                self.dragging_slider = true;
                self.move_slider(atlas, i, p);
            }
            Some(Hit::ScrollBar(which)) if which > 0 => {
                if let (Some(s), Some(r)) = (self.slide, self.rect(atlas, Hit::ScrollBar(which))) {
                    let list = &mut self.lists[which - 1];
                    let count = list.shown.len() as i32;
                    let bar = r[3] - 2;
                    let ratio = if LIST_ROWS as i32 >= count { 1.0 } else { LIST_ROWS as f32 / count as f32 };
                    let movable = bar - ((ratio * bar as f32) as i32).max(6);
                    let delta = ((p.1 - s.anchor) as f32 / movable as f32 * (count - LIST_ROWS as i32) as f32) as i32;
                    list.set_scroll(s.value + delta);
                }
            }
            _ => {}
        }
    }

    fn move_slider(&mut self, atlas: &HudAtlas, i: usize, p: (i32, i32)) {
        if let Some(r) = self.rect(atlas, Hit::Slider(i)) {
            let value = slider_at(p.0 - r[0]);
            fpm::edit(|s| set_slider_value(s, i, value));
        }
    }

    /// `onMouseUp`, then `onClick` on the pressed widget: TRender's clicks.
    pub(super) fn release(&mut self, game: &Game<'_>, p: (f32, f32)) {
        let atlas = &game.gui.atlas;
        let p = self.local(p);
        let Some(hit) = self.pressed.take() else {
            return;
        };
        self.dragging_slider = false;
        self.slide = None;
        match hit {
            // Tabs and toggles don't look where the button went up.
            Hit::Tab(i) => self.select_tab(i),
            Hit::Toggle(t) => self.flip(t),
            Hit::Row(tab, entry) => self.flip_row(tab, entry),
            Hit::Slider(i) => self.move_slider(atlas, i, p),
            Hit::Field(tab) => {
                self.focus = Some(Focus::Field(tab));
                let place = self.rect(atlas, hit).map(|r| p.0 - r[0] - 4);
                let f = &mut self.lists[tab - 1].field;
                if let Some(x) = place {
                    // Before the character whose left half the click is in.
                    let chars: Vec<char> = f.text.chars().skip(f.scroll).collect();
                    let mut pos = f.scroll + chars.len();
                    let mut run = 0;
                    for (k, ch) in chars.iter().enumerate() {
                        let cw = width(atlas, &ch.to_string());
                        if x < run + cw / 2 {
                            pos = f.scroll + k;
                            break;
                        }
                        run += cw;
                    }
                    f.move_to(pos, false);
                }
            }
            // Buttons click only when the button went up over them.
            Hit::Back | Hit::Forward | Hit::EnumButton | Hit::Done | Hit::Reset => {
                if self.rect(atlas, hit).is_some_and(|r| within(p, r)) {
                    self.activate(hit);
                }
            }
            _ => {}
        }
    }

    fn select_tab(&mut self, i: usize) {
        self.click();
        if self.tab != i {
            self.tab = i;
            // Hidden widgets let go of the focus.
            if !matches!(self.focus, Some(Focus::Tab(_) | Focus::Back | Focus::Forward | Focus::Done | Focus::Reset)) {
                self.focus = None;
            }
        }
    }

    fn flip(&mut self, t: usize) {
        self.click();
        fpm::edit(|s| flip_toggle(s, t));
    }

    /// An item or layer row's toggle: the list changes and is saved at once.
    fn flip_row(&mut self, tab: usize, entry: usize) {
        self.click();
        let settings = fpm::settings();
        let list = &mut self.lists[tab - 1];
        let key = list.entries[entry].key.clone();
        let on = !list.toggles.get(&entry).copied().unwrap_or_else(|| initial(&settings, tab, &key));
        list.toggles.insert(entry, on);
        fpm::edit(|s| match tab {
            1 if on => fpm::add(&mut s.auto_vanilla_hands, &key),
            1 => fpm::remove(&mut s.auto_vanilla_hands, &key),
            2 if on => fpm::add(&mut s.auto_toggle_mod_items, &key),
            2 => fpm::remove(&mut s.auto_toggle_mod_items, &key),
            // Shown when on.
            _ if on => fpm::remove(&mut s.hidden_layers, &key),
            _ => fpm::add(&mut s.hidden_layers, &key),
        });
        fpm::save();
    }

    /// A button's click.
    fn activate(&mut self, hit: Hit) {
        self.click();
        match hit {
            Hit::Back => self.ribbon = self.ribbon.saturating_sub(1),
            Hit::Forward => self.ribbon = (self.ribbon + 1).min(TABS.len() - 1),
            Hit::EnumButton => fpm::edit(|s| s.vanilla_hands_mode = s.vanilla_hands_mode.next()),
            Hit::Done => self.done(),
            Hit::Reset => {
                fpm::reset();
                fpm::save();
            }
            _ => {}
        }
    }

    /// Done and Escape: `save()`, then back.
    fn done(&mut self) {
        fpm::save();
        self.closed = true;
    }

    /// The wheel: up from the widget under the pointer to its list.
    pub(super) fn scroll(&mut self, game: &Game<'_>, p: (f32, f32), scroll: [f64; 2]) {
        let atlas = &game.gui.atlas;
        let hit = self.hit(atlas, self.local(p));
        let tab = match hit {
            Hit::Row(tab, _) => tab,
            Hit::ScrollBar(tab) if tab > 0 => tab,
            _ => return,
        };
        let list = &mut self.lists[tab - 1];
        list.set_scroll(list.scroll + ((scroll[0] - scroll[1]) as i32) * 4);
    }

    /// The widgets Tab walks through, in tree order.
    fn focus_order(&self, atlas: &HudAtlas) -> Vec<Focus> {
        let mut out = Vec::new();
        for (hit, ..) in self.ribbon(atlas) {
            out.push(match hit {
                Hit::Tab(i) => Focus::Tab(i),
                Hit::Back => Focus::Back,
                _ => Focus::Forward,
            });
        }
        for (hit, _) in self.widgets(atlas) {
            match hit {
                Hit::ScrollBar(w) => out.push(Focus::ScrollBar(w)),
                Hit::Slider(i) => out.push(Focus::Slider(i)),
                Hit::Toggle(t) => out.push(Focus::Toggle(t)),
                Hit::EnumButton => out.push(Focus::EnumButton),
                Hit::Row(t, e) => out.push(Focus::Row(t, e)),
                Hit::Field(t) => out.push(Focus::Field(t)),
                _ => {}
            }
        }
        out.push(Focus::Done);
        out.push(Focus::Reset);
        out
    }

    pub(super) fn key(&mut self, game: &Game<'_>, key: &str, modifiers: u32) {
        let atlas = &game.gui.atlas;
        let shift = modifiers & 1 != 0;
        if key == "key.keyboard.escape" {
            self.done();
            return;
        }
        // The focused widget first.
        if let Some(f) = self.focus
            && self.focused_key(atlas, f, key, modifiers)
        {
            return;
        }
        if key == "key.keyboard.tab" {
            let order = self.focus_order(atlas);
            if order.is_empty() {
                return;
            }
            let at = self.focus.and_then(|f| order.iter().position(|o| *o == f));
            let next = match (at, shift) {
                (None, false) => 0,
                (None, true) => order.len() - 1,
                (Some(i), false) => (i + 1) % order.len(),
                (Some(i), true) => (i + order.len() - 1) % order.len(),
            };
            self.focus = Some(order[next]);
        }
    }

    /// A key for the focused widget: true when it took it.
    fn focused_key(&mut self, atlas: &HudAtlas, f: Focus, key: &str, modifiers: u32) -> bool {
        let shift = modifiers & 1 != 0;
        let ctrl = modifiers & 2 != 0;
        let enter = matches!(key, "key.keyboard.enter" | "key.keyboard.keypad.enter" | "key.keyboard.space");
        match f {
            Focus::Tab(i) if enter => self.select_tab(i),
            Focus::Toggle(t) if enter => self.flip(t),
            Focus::Row(tab, e) if enter => self.flip_row(tab, e),
            Focus::Back if enter => self.activate(Hit::Back),
            Focus::Forward if enter => self.activate(Hit::Forward),
            Focus::EnumButton if enter => self.activate(Hit::EnumButton),
            Focus::Done if enter => self.activate(Hit::Done),
            Focus::Reset if enter => self.activate(Hit::Reset),
            Focus::Slider(i) => {
                let value = slider_value(&fpm::settings(), i);
                let new = match (key, modifiers) {
                    ("key.keyboard.left" | "key.keyboard.down", 0) => value - 1,
                    ("key.keyboard.right" | "key.keyboard.up", 0) => value + 1,
                    ("key.keyboard.left" | "key.keyboard.down", 2) => SLIDER_MIN,
                    ("key.keyboard.right" | "key.keyboard.up", 2) => SLIDER_MAX,
                    _ => return false,
                }
                .clamp(SLIDER_MIN, SLIDER_MAX);
                if new == value {
                    return false;
                }
                fpm::edit(|s| set_slider_value(s, i, new));
            }
            Focus::ScrollBar(which) => {
                let step = match key {
                    "key.keyboard.down" | "key.keyboard.left" => 1,
                    "key.keyboard.up" | "key.keyboard.right" => -1,
                    _ => return false,
                };
                if which > 0 {
                    let list = &mut self.lists[which - 1];
                    list.set_scroll(list.scroll + step);
                }
            }
            Focus::Field(tab) => {
                let f = &mut self.lists[tab - 1].field;
                let before = f.text.clone();
                match key {
                    "key.keyboard.a" if ctrl && !shift => {
                        f.select = 0;
                        f.cursor = f.len();
                    }
                    "key.keyboard.c" if ctrl && !shift => {
                        let (a, b) = f.selection();
                        let copied = f.slice(a, b);
                        self.actions.push(Action::Copy(copied));
                    }
                    "key.keyboard.v" if ctrl && !shift => self.actions.push(Action::Paste),
                    "key.keyboard.backspace" => f.delete(false, ctrl),
                    "key.keyboard.delete" => f.delete(true, ctrl),
                    "key.keyboard.left" | "key.keyboard.right" => {
                        let right = key == "key.keyboard.right";
                        let (a, b) = f.selection();
                        if a != b && !shift {
                            f.move_to(if right { b } else { a }, false);
                        } else {
                            let to = if ctrl {
                                f.word(if right { 1 } else { -1 })
                            } else if right {
                                f.cursor + 1
                            } else {
                                f.cursor.saturating_sub(1)
                            };
                            f.move_to(to, shift);
                        }
                    }
                    "key.keyboard.home" | "key.keyboard.up" => f.move_to(0, shift),
                    "key.keyboard.end" | "key.keyboard.down" => {
                        let end = f.len();
                        f.move_to(end, shift);
                    }
                    _ => return false,
                }
                let changed = f.text != before;
                f.follow(atlas);
                if changed {
                    self.lists[tab - 1].filter();
                }
            }
            _ => return false,
        }
        true
    }

    pub(super) fn char_typed(&mut self, game: &Game<'_>, ch: char) {
        if let Some(Focus::Field(tab)) = self.focus {
            let list = &mut self.lists[tab - 1];
            list.field.insert(&ch.to_string());
            list.field.follow(&game.gui.atlas);
            list.filter();
        }
    }

    /// Ctrl+V's text, from `frame`.
    pub(super) fn paste(&mut self, text: &str) {
        if let Some(Focus::Field(tab)) = self.focus {
            let list = &mut self.lists[tab - 1];
            list.field.insert(text);
            list.filter();
        }
    }
}

/// The widget a focus stands for, for `checkFocus`.
fn focus_hit(f: Focus) -> Hit {
    match f {
        Focus::Tab(i) => Hit::Tab(i),
        Focus::Back => Hit::Back,
        Focus::Forward => Hit::Forward,
        Focus::ScrollBar(w) => Hit::ScrollBar(w),
        Focus::Slider(i) => Hit::Slider(i),
        Focus::Toggle(t) => Hit::Toggle(t),
        Focus::EnumButton => Hit::EnumButton,
        Focus::Row(t, e) => Hit::Row(t, e),
        Focus::Field(t) => Hit::Field(t),
        Focus::Done => Hit::Done,
        Focus::Reset => Hit::Reset,
    }
}

/// `isWithinBounds`: the point in the rectangle (right and bottom excluded).
fn within(p: (i32, i32), r: [i32; 4]) -> bool {
    p.0 >= r[0] && p.0 < r[0] + r[2] && p.1 >= r[1] && p.1 < r[1] + r[3]
}

/// A row's toggle as it is first configured: in the tab's list, or for
/// "Show Layers", not hidden.
fn initial(s: &Settings, tab: usize, key: &str) -> bool {
    match tab {
        1 => s.auto_vanilla_hands.iter().any(|k| k == key),
        2 => s.auto_toggle_mod_items.iter().any(|k| k == key),
        _ => !s.layer_hidden(key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thumb_follows_trenders_float_maths() {
        assert_eq!(thumb_x(-40), 0);
        assert_eq!(thumb_x(-30), 21);
        assert_eq!(thumb_x(0), 86);
        assert_eq!(thumb_x(10), 107);
        assert_eq!(thumb_x(40), 172);
        assert_eq!(slider_at(90), 0);
        assert_eq!(slider_at(0), -40);
        assert_eq!(slider_at(180), 40);
    }

    #[test]
    fn the_enum_cycles() {
        assert_eq!(fpm::VanillaHands::Items.next(), fpm::VanillaHands::Off);
    }
}
