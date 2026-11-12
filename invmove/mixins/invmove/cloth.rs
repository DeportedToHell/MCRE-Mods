// Cloth Config for Horizon: the config screen Cloth Config 26.2 builds
// (`ClothConfigScreen`): the title, the tab row with its arrows, the list
// with its search field and the options' rows (`BooleanListEntry`,
// `IntegerListEntry`, `FloatListEntry`, `StringListEntry`,
// `SelectionListEntry`, `KeyCodeEntry`, `TextListEntry`,
// `StringListListEntry`, `IntegerSliderEntry`, `SubCategoryListEntry`), the
// error line, Cancel and Save & Quit, and the "Changes Not Saved"
// confirmation, laid out with Cloth Config's own numbers in the game's
// widgets and Cloth Config's texture. Rewritten from Cloth Config by
// shedaniel (LGPL-3.0, https://github.com/shedaniel/cloth-config). Falling
// Leaves' copy (ClientSort's, with the slider and the sub-category), with what
// InvMove's screen adds: a toggle's own Yes/No texts (`setYesNoTextSupplier`)
// and an entry that can't be changed (`setRequirement(() -> false)`).
//
// Any of our mods can build one: `Screen::new(title, categories)` of
// `Entry`s, then `draw` from `screen_render_post`, the input functions from
// the screen hooks, and `take_requests` from `frame` (what needs `&mut Game`:
// the click sound, the clipboard, closing, saving).
use super::Game;
use super::invmove_cloth_sprites::{self as sprites, Sprite};
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::text::Text;
use horizon_client::ui::{self, EditBox};
use std::time::Instant;

type Rect = [f32; 4];

/// GLFW's modifier bits, as the screen hooks pass them.
pub(super) const SHIFT: u32 = 1;
pub(super) const CONTROL: u32 = 2;
pub(super) const ALT: u32 = 4;
pub(super) const UNBOUND: &str = "key.keyboard.unknown";

// ---- texts ---------------------------------------------------------------------------------

/// Cloth Config's `en_us.json` and `ru_ru.json`.
fn tr(ru: bool, key: &str) -> &'static str {
    match (key, ru) {
        ("reset", false) => "Reset",
        ("reset", true) => "Сброс",
        ("save", false) => "Save & Quit",
        ("save", true) => "Сохранить и выйти",
        ("discard", false) => "Discard Changes",
        ("discard", true) => "Отменить изменения",
        ("cannot_save", false) => "Error!",
        ("cannot_save", true) => "Ошибка!",
        ("multi_error", false) => "Multiple Issues!",
        ("multi_error", true) => "Несколько проблем!",
        ("quit_config", false) => "Changes Not Saved",
        ("quit_config", true) => "Изменения не сохранены",
        ("quit_config_sure", false) => "Are you sure you want to quit editing the config? Changes will not be saved!",
        ("quit_config_sure", true) => "Вы уверены, что хотите просто выйти из настроек? Изменения не сохранятся!",
        ("quit_discard", false) => "Quit & Discard Changes",
        ("quit_discard", true) => "Выйти и отменить изменения",
        ("yes", false) => "§aYes",
        ("yes", true) => "§aДа",
        ("no", false) => "§cNo",
        ("no", true) => "§cНет",
        ("not_int", false) => "Not a valid number! (Integer)",
        ("not_int", true) => "Недопустимое число! (Целое)",
        ("not_float", false) => "Not a valid number! (Float)",
        ("not_float", true) => "Недопустимое число! (С плавающей запятой)",
        ("add", false) => "Insert New",
        ("add", true) => "Вставить новый",
        ("remove", false) => "Delete Selected",
        ("remove", true) => "Удалить выбранное",
        _ => "",
    }
}

fn russian() -> bool {
    horizon_client::lang::code().starts_with("ru")
}

/// `String.split(" ")`: Java drops the empty strings at the end.
fn java_split(s: &str) -> Vec<String> {
    let mut parts: Vec<String> = s.split(' ').map(str::to_owned).collect();
    while parts.len() > 1 && parts.last().is_some_and(String::is_empty) {
        parts.pop();
    }
    if parts.len() == 1 && parts[0].is_empty() && !s.is_empty() {
        parts.clear();
    }
    parts
}

// ---- values and entries ---------------------------------------------------------------------

/// An entry's value as the screen edits it.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Value {
    Bool(bool),
    /// An int, float or string field's text.
    Text(String),
    /// A slider's number.
    Int(i64),
    /// A selector's choice, by index.
    Choice(usize),
    /// A key (`key.keyboard.k`, `key.mouse.middle`) and the modifiers held
    /// with it (`SHIFT`, `CONTROL`, `ALT`), as `ModifierKeyCode` keeps them.
    Key(String, u32),
    List(Vec<String>),
    None,
}

impl Value {
    pub(super) fn as_bool(&self) -> bool {
        matches!(self, Value::Bool(true))
    }
    pub(super) fn as_text(&self) -> &str {
        match self {
            Value::Text(s) => s,
            _ => "",
        }
    }
    /// A slider's number, or `IntegerListEntry.getValue`: the text's number,
    /// 0 when it is none.
    pub(super) fn as_int(&self) -> i64 {
        match self {
            Value::Int(n) => *n,
            _ => self.as_text().parse::<i32>().map_or(0, i64::from),
        }
    }
    /// `FloatListEntry.getValue`.
    #[allow(dead_code)] // InvMove's screen has no number field.
    pub(super) fn as_float(&self) -> f32 {
        parse_float(self.as_text()).unwrap_or(0.0)
    }
    pub(super) fn as_choice(&self) -> usize {
        match self {
            Value::Choice(i) => *i,
            _ => 0,
        }
    }
    #[allow(dead_code)]
    pub(super) fn as_key(&self) -> &str {
        match self {
            Value::Key(k, _) => k,
            _ => UNBOUND,
        }
    }
    pub(super) fn as_list(&self) -> Vec<String> {
        match self {
            Value::List(l) => l.clone(),
            _ => Vec::new(),
        }
    }
}

/// `Float.parseFloat` for the characters a number field takes.
fn parse_float(s: &str) -> Option<f32> {
    s.parse::<f32>().ok().filter(|f| f.is_finite())
}

/// What a slider shows for its number (`setTextGetter`).
pub(super) type TextGetter = Box<dyn Fn(i64) -> String>;

/// What kind of row an entry is.
#[allow(dead_code)] // InvMove's screen has no number field, list or slider.
pub(super) enum Kind {
    Bool,
    Int,
    Float,
    Str,
    /// The choices' names, in the order a click cycles them.
    Select(Vec<String>),
    Key,
    /// A text description (`TextListEntry`): its text and colour (0xRRGGBB).
    Text(String, u32),
    /// A list of strings (`StringListListEntry`): new items go first.
    List { insert_front: bool },
    /// An int slider (`IntegerSliderEntry`) from `min` to `max`.
    Slider { min: i64, max: i64, text: TextGetter },
    /// A sub-category (`SubCategoryListEntry`): its entries are `children`.
    Sub,
}

/// An error supplier: the entry's value and every entry's, to compare with.
pub(super) type Check = Box<dyn Fn(&Value, &Values<'_>) -> Option<String>>;

/// The screen's entries, by id, for an error supplier.
pub(super) struct Values<'a>(&'a [Category]);

impl Values<'_> {
    #[allow(dead_code)]
    pub(super) fn get(&self, id: &str) -> Option<Value> {
        self.0.iter().flat_map(|c| &c.entries).flat_map(Entry::with_children).find(|e| e.id == id).map(Entry::current)
    }
}

/// One row of a category (`AbstractConfigListEntry`).
pub(super) struct Entry {
    pub(super) id: String,
    name: String,
    kind: Kind,
    tooltip: Vec<String>,
    /// A selector's tooltip for each choice (`setTooltipSupplier(val -> ...)`).
    choice_tooltips: Vec<Vec<String>>,
    value: Value,
    original: Value,
    default: Option<Value>,
    check: Option<Check>,
    /// The int, float or string field.
    field: EditBox,
    /// A list's cells, open or not.
    cells: Vec<EditBox>,
    /// A list or a sub-category open.
    expanded: bool,
    /// A sub-category's entries.
    children: Vec<Entry>,
    /// A slider's handle, 0 to 1 (`AbstractSliderButton.value`), and where
    /// it was drawn last.
    position: f64,
    slider: Rect,
    /// A toggle's texts for true and false (`setYesNoTextSupplier`), with
    /// their colour codes; Cloth Config's Yes and No when none.
    yes_no: Option<(String, String)>,
    /// The entry's requirement holds (`isEnabled`): off, its buttons are
    /// inactive and it keeps its value.
    enabled: bool,
}

fn field_box(text: &str, bordered: bool) -> EditBox {
    let mut b = EditBox::new(0.0, 0.0, 100.0, 18.0);
    b.max_length = if bordered { 999_999 } else { usize::MAX };
    b.bordered = bordered;
    b.focused = false;
    b.set_value(text);
    if !bordered {
        b.set_cursor(0, false);
    }
    b
}

impl Entry {
    fn new(id: &str, name: &str, kind: Kind, value: Value, default: Option<Value>) -> Entry {
        let text = value.as_text().to_owned();
        let cells = value.as_list().iter().map(|s| field_box(s, false)).collect();
        Entry {
            id: id.to_owned(),
            name: name.to_owned(),
            kind,
            tooltip: Vec::new(),
            choice_tooltips: Vec::new(),
            original: value.clone(),
            value,
            default,
            check: None,
            field: field_box(&text, true),
            cells,
            expanded: false,
            children: Vec::new(),
            position: 0.0,
            slider: [0.0; 4],
            yes_no: None,
            enabled: true,
        }
    }
    pub(super) fn boolean(id: &str, name: &str, value: bool, default: bool) -> Entry {
        Entry::new(id, name, Kind::Bool, Value::Bool(value), Some(Value::Bool(default)))
    }
    #[allow(dead_code)]
    pub(super) fn int(id: &str, name: &str, value: i64, default: i64) -> Entry {
        Entry::new(id, name, Kind::Int, Value::Text(value.to_string()), Some(Value::Text(default.to_string())))
    }
    /// `value` and `default` as `String.valueOf(float)` writes them.
    #[allow(dead_code)]
    pub(super) fn float(id: &str, name: &str, value: &str, default: &str) -> Entry {
        Entry::new(id, name, Kind::Float, Value::Text(value.into()), Some(Value::Text(default.into())))
    }
    #[allow(dead_code)]
    pub(super) fn string(id: &str, name: &str, value: &str, default: &str) -> Entry {
        Entry::new(id, name, Kind::Str, Value::Text(value.into()), Some(Value::Text(default.into())))
    }
    pub(super) fn select(id: &str, name: &str, names: Vec<String>, value: usize, default: usize) -> Entry {
        Entry::new(id, name, Kind::Select(names), Value::Choice(value), Some(Value::Choice(default)))
    }
    #[allow(dead_code)]
    pub(super) fn key(id: &str, name: &str, value: &str, default: &str) -> Entry {
        Entry::new(id, name, Kind::Key, Value::Key(value.into(), 0), Some(Value::Key(default.into(), 0)))
    }
    /// A text description (`startTextDescription`), white unless `color`.
    #[allow(dead_code)]
    pub(super) fn text(text: &str, color: u32) -> Entry {
        Entry::new("", "", Kind::Text(text.into(), color), Value::None, None)
    }
    #[allow(dead_code)]
    pub(super) fn list(id: &str, name: &str, value: Vec<String>, default: Vec<String>, expanded: bool, insert_front: bool) -> Entry {
        let mut e = Entry::new(id, name, Kind::List { insert_front }, Value::List(value), Some(Value::List(default)));
        e.expanded = expanded;
        e
    }
    /// An int slider (`startIntSlider`), its text "Value: 5" until
    /// `slider_text` says otherwise.
    #[allow(dead_code)]
    pub(super) fn slider(id: &str, name: &str, value: i64, default: i64, min: i64, max: i64) -> Entry {
        let text: TextGetter = Box::new(|v| format!("Value: {v}"));
        let mut e = Entry::new(id, name, Kind::Slider { min, max, text }, Value::Int(value), Some(Value::Int(default)));
        // The handle where the constructor puts it: not clamped.
        e.position = (value - min) as f64 / (max - min).abs().max(1) as f64;
        e
    }
    #[allow(dead_code)]
    pub(super) fn slider_text(mut self, f: impl Fn(i64) -> String + 'static) -> Entry {
        if let Kind::Slider { text, .. } = &mut self.kind {
            *text = Box::new(f);
        }
        self
    }
    /// A sub-category (`SubCategoryBuilder`), closed unless `expanded`.
    pub(super) fn sub(name: &str, children: Vec<Entry>, expanded: bool) -> Entry {
        let mut e = Entry::new("", name, Kind::Sub, Value::None, None);
        e.children = children;
        e.expanded = expanded;
        e
    }
    /// A toggle's texts for true and false (`setYesNoTextSupplier`), with
    /// colour codes (`§a`, `§c`).
    pub(super) fn yes_no(mut self, yes: &str, no: &str) -> Entry {
        self.yes_no = Some((yes.to_owned(), no.to_owned()));
        self
    }
    /// No default value (no `setDefaultValue`): Reset stays inactive.
    pub(super) fn no_default(mut self) -> Entry {
        self.default = None;
        self
    }
    /// A requirement that never holds (`setRequirement(() -> false)`).
    pub(super) fn disabled(mut self) -> Entry {
        self.enabled = false;
        self
    }
    pub(super) fn tooltip(mut self, lines: &[&str]) -> Entry {
        self.tooltip = lines.iter().map(|s| s.to_string()).collect();
        self
    }
    #[allow(dead_code)]
    pub(super) fn choice_tooltips(mut self, tips: Vec<Vec<String>>) -> Entry {
        self.choice_tooltips = tips;
        self
    }
    #[allow(dead_code)]
    pub(super) fn check(mut self, f: impl Fn(&Value, &Values<'_>) -> Option<String> + 'static) -> Entry {
        self.check = Some(Box::new(f));
        self
    }

    /// The entry, then a sub-category's entries.
    fn with_children(&self) -> impl Iterator<Item = &Entry> {
        std::iter::once(self).chain(self.children.iter())
    }

    /// The value now (`getValue`).
    pub(super) fn current(&self) -> Value {
        match self.kind {
            Kind::Int | Kind::Float | Kind::Str => Value::Text(self.field.value.clone()),
            Kind::List { .. } => Value::List(self.cells.iter().map(|c| c.value.clone()).collect()),
            _ => self.value.clone(),
        }
    }

    /// `getConfigError`: the error supplier's, then the entry's own; a
    /// sub-category's is its first entry's with one.
    fn error(&self, values: &Values<'_>, ru: bool) -> Option<String> {
        let value = self.current();
        if let Some(e) = self.check.as_ref().and_then(|c| c(&value, values)) {
            return Some(e);
        }
        match self.kind {
            Kind::Int if value.as_text().parse::<i32>().is_err() => Some(tr(ru, "not_int").into()),
            Kind::Float if parse_float(value.as_text()).is_none() => Some(tr(ru, "not_float").into()),
            Kind::Sub => self.children.iter().find_map(|c| c.error(values, ru)),
            _ => None,
        }
    }

    /// `isEdited`: changed from what the screen opened with (an error counts
    /// too, but not for the text fields); a sub-category when any of its
    /// entries is.
    fn edited(&self, values: &Values<'_>, ru: bool) -> bool {
        let error = || self.error(values, ru).is_some();
        match self.kind {
            Kind::Int | Kind::Float | Kind::Str => self.current() != self.original,
            Kind::Text(..) => error(),
            Kind::Sub => self.children.iter().any(|c| c.edited(values, ru)),
            _ => error() || self.current() != self.original,
        }
    }

    fn matches_default(&self) -> bool {
        self.default.as_ref().is_some_and(|d| {
            let now = self.current();
            match (&now, d) {
                // `KeyCodeEntry`: the default has no modifiers.
                (Value::Key(k, m), Value::Key(dk, _)) => k == dk && *m == 0,
                _ => &now == d,
            }
        })
    }

    fn reset(&mut self) {
        let Some(d) = self.default.clone() else {
            return;
        };
        match &self.kind {
            Kind::Int | Kind::Float | Kind::Str => {
                self.field.set_value(d.as_text());
            }
            Kind::List { .. } => {
                self.cells = d.as_list().iter().map(|s| field_box(s, false)).collect();
            }
            Kind::Slider { .. } => self.set_slider(d.as_int()),
            _ => self.value = d,
        }
    }

    /// `IntegerSliderEntry.setValue`: the number clamped, the handle moved.
    fn set_slider(&mut self, v: i64) {
        if let Kind::Slider { min, max, .. } = self.kind {
            let v = v.clamp(min, max);
            self.position = (v - min) as f64 / (max - min).abs().max(1) as f64;
            self.value = Value::Int(v);
        }
    }

    /// `AbstractSliderButton.setValue` then `applyValue`: the handle to `to`,
    /// the number from it (`(int) (minimum + range * value)`).
    fn slide(&mut self, to: f64) {
        let Kind::Slider { min, max, .. } = self.kind else {
            return;
        };
        let old = self.position;
        self.position = to.clamp(0.0, 1.0);
        if old != self.position {
            self.value = Value::Int((min as f64 + (max - min).abs() as f64 * self.position) as i64);
        }
    }

    /// `setValueFromMouse`: the handle under the pointer.
    fn slide_to_mouse(&mut self, mx: f32) {
        let [x, _, w, _] = self.slider;
        self.slide((f64::from(mx) - f64::from(x + 4.0)) / f64::from(w - 8.0));
    }

    /// `getSearchTags`: the name's words (a sub-category's, then its
    /// entries'). A text description's name is a random id in Cloth Config,
    /// so a search never finds it.
    fn tags(&self) -> Vec<String> {
        let mut tags = java_split(&self.name);
        for c in &self.children {
            tags.extend(c.tags());
        }
        tags
    }

    /// `SearchFieldEntry.matchesSearch`: each word in a tag, the tags read
    /// once from start to end for all the words (it walks one iterator).
    fn matches(&self, words: &[String]) -> bool {
        if words.is_empty() {
            return true;
        }
        if matches!(self.kind, Kind::Text(..)) {
            return false;
        }
        let tags = self.tags();
        if tags.is_empty() {
            return true;
        }
        let mut it = tags.iter().map(|t| t.to_lowercase());
        words.iter().all(|w| it.any(|t| t.contains(w.as_str())))
    }

    fn tooltip_lines(&self) -> Vec<String> {
        match (&self.kind, &self.value) {
            (Kind::Select(_), Value::Choice(i)) if !self.choice_tooltips.is_empty() => {
                self.choice_tooltips.get(*i).cloned().unwrap_or_default()
            }
            _ => self.tooltip.clone(),
        }
    }

    /// `getItemHeight` (a sub-category's own row; its open entries follow
    /// as rows of their own).
    fn height(&self, game: &Game<'_>, entry_w: f32) -> f32 {
        match &self.kind {
            Kind::Text(text, _) => {
                let n = wrap(game, text, entry_w).len();
                if n == 0 { 0.0 } else { 14.0 + 12.0 * n as f32 }
            }
            Kind::List { .. } if self.expanded => 24.0 + 20.0 * self.cells.len() as f32,
            _ => 24.0,
        }
    }

    /// `getEntryArea`'s height: a list's or a sub-category's label row, the
    /// whole row less 4 otherwise.
    fn area_height(&self, row_h: f32) -> f32 {
        match self.kind {
            Kind::List { .. } | Kind::Sub => 20.0,
            _ => row_h - 4.0,
        }
    }
}

pub(super) struct Category {
    pub(super) name: String,
    pub(super) entries: Vec<Entry>,
}

// ---- the screen -----------------------------------------------------------------------------

/// An entry of the open category: its index, and its index inside a
/// sub-category when it is one of a sub-category's entries.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct At(usize, Option<usize>);

/// What has the keyboard: the search field, an entry's field, list cell or
/// slider, or another of an entry's widgets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Focus {
    Search,
    Field(At),
    Cell(At, usize),
    Slider(At),
    Widget(At),
}

/// What the screen asks of the game, for `frame`.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Request {
    /// A button's click sound.
    Click,
    /// Ctrl+V into the focused field.
    Paste,
    Copy(String),
    /// Leave the screen: Save & Quit (true) or discarded (false).
    Close(bool),
}

/// `ValueAnimator`'s easing toward a target (`DoubleValueAnimatorImpl`).
#[derive(Clone, Copy, Debug)]
struct Anim {
    value: f32,
    target: f32,
    start: Instant,
    duration: f32,
}

impl Anim {
    fn new() -> Anim {
        Anim { value: 0.0, target: 0.0, start: Instant::now(), duration: 0.0 }
    }
    fn set_to(&mut self, target: f32, ms: f32) {
        if self.target != target {
            self.target = target;
            self.start = Instant::now();
            self.duration = ms;
            if ms <= 0.0 {
                self.value = target;
            }
        }
    }
    fn set(&mut self, value: f32) {
        self.value = value;
        self.target = value;
        self.duration = 0.0;
    }
    /// One frame `dt` ticks long.
    fn update(&mut self, dt: f32) {
        if self.duration == 0.0 || self.value == self.target {
            return;
        }
        let t = (self.start.elapsed().as_secs_f32() * 1000.0 / self.duration * dt * 3.0).min(1.0);
        let (a, b) = (self.value, self.target);
        self.value = if a < b { (a + 2.0 * (b - a) * t).min(b) } else { (a - 2.0 * (a - b) * t).max(b) };
    }
}

/// A row of the list as laid out this frame.
#[derive(Clone, Copy, Debug)]
enum RowKind {
    Gap,
    Search,
    Entry(At),
}

#[derive(Clone, Copy, Debug)]
struct Row {
    kind: RowKind,
    /// The row's top on the screen and its height (`getItemHeight`).
    y: f32,
    h: f32,
    /// The entry's left edge and width: a sub-category's entries 14 pixels
    /// in.
    x: f32,
    w: f32,
}

/// Where everything is this frame (`init`'s numbers).
struct Layout {
    w: f32,
    h: f32,
    tabs: bool,
    top: f32,
    bottom: f32,
    /// The rows' left edge and width (`getRowLeft`, `getItemWidth`).
    x: f32,
    entry_w: f32,
    rows: Vec<Row>,
    content: f32,
    max_scroll: f32,
    tab_xs: Vec<(f32, f32)>,
    tab_max: f32,
    footer_w: f32,
}

pub(super) struct Screen {
    title: String,
    pub(super) categories: Vec<Category>,
    selected: usize,
    search: EditBox,
    focus: Option<Focus>,
    /// The key field waiting for a key (`focusedBinding`) and what has been
    /// pressed since (`startedKeyCode`).
    binding: Option<At>,
    started: (String, u32),
    /// The focused slider's arrows move it (`canChangeValue`: focused by the
    /// mouse, or Enter pressed on it).
    slider_live: bool,
    /// The slider the left button went down on, until it comes up.
    sliding: Option<At>,
    scroll: Anim,
    tab_scroll: Anim,
    scrolling: Option<f32>,
    /// The hovered row's highlight (`currentBounds`), in list coordinates,
    /// and when the pointer was last over a row.
    hover: [Anim; 4],
    hover_target: Option<Rect>,
    last_touch: Instant,
    last_frame: Instant,
    /// "Changes Not Saved" is up.
    confirm: bool,
    pub(super) in_world: bool,
    requests: Vec<Request>,
}

fn argb(c: u32) -> u32 {
    let [a, r, g, b] = c.to_be_bytes();
    rgba(r, g, b, a)
}

fn fill(game: &Game<'_>, batch: &mut HudBatch, r: Rect, color: u32) {
    if r[2] > 0.0 && r[3] > 0.0 {
        batch.fill(&game.gui.atlas, r[0], r[1], r[2], r[3], argb(color));
    }
}

fn width(game: &Game<'_>, s: &str) -> f32 {
    HudBatch::text_width(&game.gui.atlas, s)
}

fn inside(p: (f32, f32), r: Rect) -> bool {
    p.0 >= r[0] && p.0 < r[0] + r[2] && p.1 >= r[1] && p.1 < r[1] + r[3]
}

fn blit(game: &Game<'_>, batch: &mut HudBatch, s: &Sprite, x: f32, y: f32) {
    let atlas = &game.gui.atlas;
    let _ = s.width;
    for &(px, py, len, c) in s.runs {
        batch.fill(atlas, x + f32::from(px), y + f32::from(py), f32::from(len), 1.0, c);
    }
}

fn wrap(game: &Game<'_>, s: &str, w: f32) -> Vec<Text> {
    if s.is_empty() {
        return Vec::new();
    }
    game.gui.atlas.font.split_lines(&Text::legacy(s), w.max(1.0)).into_iter().map(|(t, _)| t).collect()
}

fn button(game: &Game<'_>, batch: &mut HudBatch, r: Rect, label: &Text, active: bool) {
    let hover = active && inside(game.cursor, r);
    ui::button(batch, &game.gui.atlas, r[0], r[1], r[2], r[3], label, active, hover);
}

/// A key's name as `ModifierKeyCode.getLocalizedName` writes it.
fn key_text(key: &str, mods: u32) -> Text {
    let mut t = horizon_client::keybinds::display_name(key);
    if mods & SHIFT != 0 {
        t = Text::literal("Shift + ").append(t);
    }
    if mods & CONTROL != 0 {
        t = Text::literal("Ctrl + ").append(t);
    }
    if mods & ALT != 0 {
        t = Text::literal("Alt + ").append(t);
    }
    t
}

/// A key that writes text when typed (its characters come through the
/// character hook).
fn writes(key: &str) -> bool {
    let Some(name) = key.strip_prefix("key.keyboard.") else {
        return false;
    };
    name.chars().count() == 1 || name == "space" || name.starts_with("keypad.") && name.len() == 8
}

impl Screen {
    pub(super) fn new(title: &str, categories: Vec<Category>, in_world: bool) -> Screen {
        let mut search = EditBox::new(0.0, 0.0, 100.0, 18.0);
        search.focused = false;
        Screen {
            title: title.to_owned(),
            categories,
            selected: 0,
            search,
            focus: None,
            binding: None,
            started: (UNBOUND.into(), 0),
            slider_live: false,
            sliding: None,
            scroll: Anim::new(),
            tab_scroll: Anim::new(),
            scrolling: None,
            hover: [Anim::new(); 4],
            hover_target: None,
            last_touch: Instant::now(),
            last_frame: Instant::now(),
            confirm: false,
            in_world,
            requests: Vec::new(),
        }
    }

    pub(super) fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    /// `init` again (a tab chosen, back from the confirmation): a new list,
    /// so its scroll and search start over.
    fn reinit(&mut self) {
        self.scroll.set(0.0);
        self.search.set_value("");
        self.search.focused = false;
        self.focus = None;
        self.binding = None;
        self.sliding = None;
        self.scrolling = None;
        self.hover_target = None;
    }

    fn entries(&self) -> &[Entry] {
        self.categories.get(self.selected).map_or(&[], |c| &c.entries)
    }

    fn entry(&self, at: At) -> Option<&Entry> {
        let e = self.entries().get(at.0)?;
        match at.1 {
            Some(c) => e.children.get(c),
            None => Some(e),
        }
    }

    fn entry_mut(&mut self, at: At) -> Option<&mut Entry> {
        let e = self.categories.get_mut(self.selected)?.entries.get_mut(at.0)?;
        match at.1 {
            Some(c) => e.children.get_mut(c),
            None => Some(e),
        }
    }

    /// Focus on an entry's widget; a slider keeps `canChangeValue` only
    /// while it has the focus.
    fn focus_on(&mut self, f: Focus) {
        if !matches!(f, Focus::Slider(_)) {
            self.slider_live = false;
        }
        self.focus = Some(f);
    }

    /// Every entry's error, in order (`getConfigError` for all categories).
    fn errors(&self, ru: bool) -> Vec<String> {
        let values = Values(&self.categories);
        self.categories.iter().flat_map(|c| &c.entries).filter_map(|e| e.error(&values, ru)).collect()
    }

    /// `isEdited` for the whole screen.
    pub(super) fn edited(&self, ru: bool) -> bool {
        let values = Values(&self.categories);
        self.categories.iter().flat_map(|c| &c.entries).any(|e| e.edited(&values, ru))
    }

    /// Every entry's id and value (a sub-category's entries too), for the
    /// mod to save.
    pub(super) fn values(&self) -> Vec<(String, Value)> {
        self.categories
            .iter()
            .flat_map(|c| &c.entries)
            .flat_map(Entry::with_children)
            .filter(|e| !e.id.is_empty())
            .map(|e| (e.id.clone(), e.current()))
            .collect()
    }

    fn search_words(&self) -> Vec<String> {
        let v = self.search.value.to_lowercase();
        if v.is_empty() { Vec::new() } else { java_split(&v) }
    }

    fn layout(&self, game: &Game<'_>, w: f32, h: f32) -> Layout {
        let tabs = self.categories.len() > 1;
        let top = if tabs { 70.0 } else { 30.0 };
        let bottom = h - 32.0;
        let item_w = w - 80.0;
        let x = (w / 2.0).floor() - (item_w / 2.0).floor() + 2.0;
        let scroll = self.scroll.value.floor();
        let mut rows = Vec::new();
        let mut y = top + 4.0 - scroll;
        let mut push = |rows: &mut Vec<Row>, kind: RowKind, h: f32, x: f32, w: f32| {
            rows.push(Row { kind, y, h, x, w });
            y += h;
        };
        push(&mut rows, RowKind::Gap, 5.0, x, item_w);
        push(&mut rows, RowKind::Search, 24.0, x, item_w);
        push(&mut rows, RowKind::Gap, 5.0, x, item_w);
        let words = self.search_words();
        for (i, e) in self.entries().iter().enumerate() {
            if !e.matches(&words) {
                continue;
            }
            push(&mut rows, RowKind::Entry(At(i, None)), e.height(game, item_w), x, item_w);
            // `SubCategoryListEntry.filteredEntries`, 24 pixels under its
            // label and 14 in.
            if matches!(e.kind, Kind::Sub) && e.expanded {
                for (c, child) in e.children.iter().enumerate() {
                    if child.matches(&words) {
                        let h = child.height(game, item_w - 14.0);
                        push(&mut rows, RowKind::Entry(At(i, Some(c))), h, x + 14.0, item_w - 14.0);
                    }
                }
            }
        }
        let content: f32 = rows.iter().map(|r| r.h).sum();
        let max_scroll = (content - (bottom - top - 4.0)).max(0.0);
        let mut tab_xs = Vec::new();
        let mut tx = 24.0 - self.tab_scroll.value.floor();
        let mut tab_total = 0.0;
        for c in &self.categories {
            let tw = width(game, &c.name) + 8.0;
            tab_xs.push((tx, tw));
            tx += tw + 2.0;
            tab_total += tw + 2.0;
        }
        // `getTabsMaximumScrolled` against the row's 40 pixels of arrows.
        let tab_max = (tab_total + 6.0 - (w - 40.0)).max(0.0);
        Layout {
            w,
            h,
            tabs,
            top,
            bottom,
            x,
            entry_w: item_w,
            rows,
            content,
            max_scroll,
            tab_xs,
            tab_max,
            footer_w: 200.0f32.min(((w - 50.0 - 12.0) / 3.0).floor()),
        }
    }

    fn footer(l: &Layout) -> (Rect, Rect) {
        let cx = (l.w / 2.0).floor();
        (
            [cx - l.footer_w - 3.0, l.h - 26.0, l.footer_w, 20.0],
            [cx + 3.0, l.h - 26.0, l.footer_w, 20.0],
        )
    }

    // ---- drawing --------------------------------------------------------------------------

    /// The whole screen over the options (`extractRenderState`).
    pub(super) fn draw(&mut self, game: &Game<'_>, batch: &mut HudBatch) {
        let ru = russian();
        let atlas = &game.gui.atlas;
        let (w, h) = (batch.width.floor(), batch.height.floor());
        let dt = (self.last_frame.elapsed().as_secs_f32() * 20.0).min(5.0);
        self.last_frame = Instant::now();
        if !self.in_world {
            game.native_apply_screen_blur();
        }
        ui::menu_background(batch, atlas, 0.0, 0.0, w, h, self.in_world);
        if self.confirm {
            let title = Text::literal(tr(ru, "quit_config"));
            let message = Text::literal(tr(ru, "quit_config_sure"));
            let c = horizon_client::confirm_screen::Confirm::new(&atlas.font, (w, h), &title, &message);
            let labels = [Text::literal(tr(ru, "quit_discard")), Text::translate("gui.cancel", &[])];
            c.draw(batch, atlas, game.cursor, [&labels[0], &labels[1]]);
            return;
        }
        // Scrolling eases toward its target; the tab row too.
        let l = self.layout(game, w, h);
        self.scroll.target = self.scroll.target.clamp(0.0, l.max_scroll);
        self.scroll.update(dt);
        self.scroll.value = self.scroll.value.clamp(0.0, l.max_scroll);
        self.tab_scroll.target = self.tab_scroll.target.clamp(0.0, l.tab_max);
        self.tab_scroll.update(dt);
        let l = self.layout(game, w, h);
        let cursor = game.cursor;
        let mut tip: Option<Vec<Text>> = None;

        // The list (`DynamicEntryListWidget.extractRenderState`).
        fill(game, batch, [0.0, l.top, w, l.bottom - l.top], 0x4000_0000);
        let start = batch.vertices.len();
        let over_list = cursor.1 >= l.top && cursor.1 <= l.bottom;
        // The hovered row's highlight (`getEntryArea`), moving and fading as
        // Cloth Config's does.
        let scroll = self.scroll.value.floor();
        self.hover_target = None;
        for row in &l.rows {
            let area_h = match row.kind {
                RowKind::Gap => continue,
                RowKind::Search => row.h - 4.0,
                RowKind::Entry(at) => self.entry(at).map_or(row.h - 4.0, |e| e.area_height(row.h)),
            };
            let area = [0.0, row.y, w, area_h];
            if over_list && inside(cursor, area) {
                self.hover_target = Some([area[0], area[1] + scroll, area[2], area[3]]);
            }
        }
        if let Some(t) = self.hover_target {
            self.last_touch = Instant::now();
            for (a, v) in self.hover.iter_mut().zip(t) {
                a.set_to(v, 100.0);
            }
        }
        for a in &mut self.hover {
            a.update(dt);
        }
        let hr = [self.hover[0].value, self.hover[1].value, self.hover[2].value, self.hover[3].value];
        if hr[2] > 0.0 && hr[3] > 0.0 {
            let past = self.last_touch.elapsed().as_millis() as f32;
            let alpha = if past <= 200.0 { 255.0 } else { (255.0 - (past - 200.0).min(500.0) / 500.0 * 255.0).ceil() };
            let a = (alpha as u32 * 36 / 255) << 24;
            fill(game, batch, [hr[0], hr[1] - scroll, hr[2], hr[3]], 0x00ff_ffff | a);
        }
        for row in &l.rows {
            if row.y + row.h < l.top || row.y > l.bottom {
                continue;
            }
            match row.kind {
                RowKind::Gap => {}
                RowKind::Search => {
                    let sw = (l.entry_w - 10.0).clamp(0.0, 500.0);
                    self.search.x = l.x + (l.entry_w / 2.0).floor() - (sw / 2.0).floor();
                    self.search.y = row.y + ((row.h - 4.0) / 2.0).floor() - 9.0;
                    self.search.width = sw;
                    self.search.height = 18.0;
                    self.search.focused = self.focus == Some(Focus::Search);
                    self.search.suggestion = self.search.value.is_empty().then(|| "Search...".to_owned());
                    self.search.draw(batch, atlas, None);
                }
                RowKind::Entry(at) => {
                    let values = Values(&self.categories);
                    let state = self.entry(at).map(|e| (e.error(&values, ru), e.edited(&values, ru)));
                    let Some((error, edited)) = state else {
                        continue;
                    };
                    let row_tip = self.draw_entry(game, batch, &l, row, at, (error, edited), ru, over_list);
                    if row_tip.is_some() {
                        tip = row_tip;
                    }
                }
            }
        }
        batch.clip_since(start, 0.0, l.top, w, l.bottom);
        ui::separators(batch, atlas, l.top, l.bottom, false);
        if l.max_scroll > 0.0 {
            // `DynamicSmoothScrollingEntryListWidget.renderScrollBar`.
            let x = w - 36.0;
            let lh = l.bottom - l.top;
            let thumb = ((lh * lh / l.content).floor()).clamp(32.0, lh - 8.0).max(10.0);
            let y = ((self.scroll.value.floor() * (lh - thumb) / l.max_scroll).floor() + l.top).clamp(l.top, l.bottom - thumb);
            let over = inside(cursor, [x, y, 6.0, thumb]);
            let (bottom_c, top_c) = if over { (168u32, 222u32) } else { (128, 172) };
            fill(game, batch, [x, l.top, 6.0, lh], 0xff00_0000);
            fill(game, batch, [x, y, 6.0, thumb], 0xff00_0000 | bottom_c * 0x10101);
            fill(game, batch, [x, y, 5.0, thumb - 1.0], 0xff00_0000 | top_c * 0x10101);
        }

        // The title and the tabs.
        let title_y = if l.tabs { 18.0 } else { 12.0 };
        let tw = width(game, &self.title);
        batch.text(atlas, &self.title, ((w - tw) / 2.0).floor(), title_y, u32::MAX, true);
        if l.tabs {
            let inner = [20.0, 41.0, w - 40.0, 24.0];
            let start = batch.vertices.len();
            fill(game, batch, inner, 0x6800_0000);
            for (i, (c, &(x, cw))) in self.categories.iter().zip(&l.tab_xs).enumerate() {
                let r = [x, 43.0, cw, 20.0];
                let active = i != self.selected;
                let hover = active && inside(cursor, r) && cursor.0 >= 20.0 && cursor.0 < w - 20.0;
                ui::button(batch, atlas, r[0], r[1], r[2], r[3], &Text::literal(c.name.clone()), active, hover);
            }
            batch.clip_since(start, inner[0], inner[1], inner[0] + inner[2], inner[1] + inner[3]);
            let left_on = self.tab_scroll.value > 0.0;
            let right_on = self.tab_scroll.value < l.tab_max;
            let left = [4.0, 44.0, 12.0, 18.0];
            let right = [w - 16.0, 44.0, 12.0, 18.0];
            let pick = |on: bool, r: Rect, off: &'static Sprite, act: &'static Sprite, hov: &'static Sprite| {
                if !on { off } else if inside(cursor, r) { hov } else { act }
            };
            blit(game, batch, pick(left_on, left, &sprites::TAB_LEFT, &sprites::TAB_LEFT_ACTIVE, &sprites::TAB_LEFT_HOVER), left[0], left[1]);
            blit(game, batch, pick(right_on, right, &sprites::TAB_RIGHT, &sprites::TAB_RIGHT_ACTIVE, &sprites::TAB_RIGHT_HOVER), right[0], right[1]);
        }

        // The errors, top left.
        let all = self.errors(ru);
        if !all.is_empty() {
            let text = format!("§c{}", if all.len() == 1 { all[0].clone() } else { tr(ru, "multi_error").to_owned() });
            let t = Text::legacy(&text);
            let sw = atlas.font.width_text(&t);
            fill(game, batch, [8.0, 9.0, 12.0 + sw, 14.0], 0x6800_0000);
            blit(game, batch, &sprites::ERROR, 10.0, 10.0);
            batch.text_styled(atlas, &t, 18.0, 12.0, u32::MAX, true);
            if all.len() > 1 && cursor.0 >= 10.0 && cursor.1 >= 10.0 && cursor.0 <= 18.0 + sw && cursor.1 <= 23.0 {
                tip = Some(all.iter().map(|e| Text::literal(e.clone())).collect());
            }
        }

        // Cancel and Save & Quit.
        let edited = self.edited(ru);
        let (cancel, save) = Self::footer(&l);
        let cancel_label = if edited { Text::literal(tr(ru, "discard")) } else { Text::translate("gui.cancel", &[]) };
        button(game, batch, cancel, &cancel_label, true);
        let save_label = Text::literal(tr(ru, if all.is_empty() { "save" } else { "cannot_save" }));
        button(game, batch, save, &save_label, edited && all.is_empty());

        if let Some(lines) = tip {
            ui::tooltip(batch, atlas, &lines, cursor);
        }
    }

    /// One entry's row; its tooltip when the pointer is over it.
    #[allow(clippy::too_many_arguments)]
    fn draw_entry(
        &mut self,
        game: &Game<'_>,
        batch: &mut HudBatch,
        l: &Layout,
        row: &Row,
        at: At,
        (error, edited): (Option<String>, bool),
        ru: bool,
        over_list: bool,
    ) -> Option<Vec<Text>> {
        let atlas = &game.gui.atlas;
        let cursor = game.cursor;
        let (x, y, ew) = (row.x, row.y, row.w);
        let focus = self.focus;
        let binding = self.binding == Some(at);
        let slider_live = self.slider_live;
        let sliding = self.sliding == Some(at);
        let reset_label = Text::literal(tr(ru, "reset"));
        let rw = width(game, tr(ru, "reset")) + 6.0;
        let e = self.entry_mut(at)?;
        // `getDisplayedFieldName`: red with an error, italic when edited,
        // grey otherwise; `getPreferredTextColor` under it.
        let mut name = Text::literal(e.name.clone());
        if error.is_some() {
            name = name.with_color(horizon_client::text::RED);
        }
        if edited {
            name = name.italic();
        }
        if error.is_none() && !edited {
            name = name.with_color(horizon_client::text::GRAY);
        }
        let base = if error.is_some() { argb(0xffff_5555) } else { u32::MAX };
        let area = [0.0, y, l.w, e.area_height(row.h)];
        let hovered = over_list && inside(cursor, area);
        let mut tip = hovered.then(|| e.tooltip_lines()).filter(|t| !t.is_empty());
        let reset_r = [x + ew - rw, y, rw, 20.0];
        let can_reset = e.enabled && e.default.is_some() && !e.matches_default();
        match &e.kind {
            Kind::Bool | Kind::Select(_) | Kind::Key => {
                batch.text_styled(atlas, &name, x, y + 6.0, base, true);
                button(game, batch, reset_r, &reset_label, can_reset);
                let label = match (&e.kind, &e.value) {
                    (Kind::Bool, Value::Bool(b)) => match &e.yes_no {
                        Some((yes, no)) => Text::legacy(if *b { yes } else { no }),
                        None => Text::legacy(tr(ru, if *b { "yes" } else { "no" })),
                    },
                    (Kind::Select(names), Value::Choice(c)) => Text::literal(names.get(*c).cloned().unwrap_or_default()),
                    (Kind::Key, Value::Key(k, m)) => {
                        let t = key_text(k, *m);
                        if binding {
                            Text::literal("> ")
                                .with_color(horizon_client::text::WHITE)
                                .append(Text::literal(t.plain()).with_color(horizon_client::text::YELLOW))
                                .append(Text::literal(" <").with_color(horizon_client::text::WHITE))
                        } else {
                            t
                        }
                    }
                    _ => Text::empty(),
                };
                button(game, batch, [x + ew - 150.0, y, 150.0 - rw - 2.0, 20.0], &label, e.enabled);
            }
            Kind::Int | Kind::Float | Kind::Str => {
                batch.text_styled(atlas, &name, x, y + 6.0, base, true);
                button(game, batch, reset_r, &reset_label, can_reset);
                let f = &mut e.field;
                f.x = x + ew - 148.0;
                f.y = y + 1.0;
                f.width = 148.0 - rw - 4.0;
                f.height = 18.0;
                f.focused = focus == Some(Focus::Field(at));
                f.text_color = if error.is_some() { rgba(255, 85, 85, 255) } else { rgba(224, 224, 224, 255) };
                f.draw(batch, atlas, None);
            }
            Kind::Slider { text, .. } => {
                // `IntegerSliderEntry`: the name, Reset at the right, the
                // slider 150 pixels from the right edge up to Reset.
                batch.text_styled(atlas, &name, x, y + 6.0, base, true);
                button(game, batch, reset_r, &reset_label, can_reset);
                let label = Text::literal(text(e.value.as_int()));
                let r = [x + ew - 150.0, y, 150.0 - rw - 2.0, 20.0];
                e.slider = r;
                let focused = focus == Some(Focus::Slider(at));
                let hover = inside(cursor, r);
                ui::slider_ext(batch, atlas, r[0], r[1], r[2], &label, e.position as f32, hover, focused, focused && slider_live, true, sliding);
            }
            Kind::Text(text, color) => {
                let mut yy = y + 7.0;
                for line in wrap(game, text, ew) {
                    batch.text_styled(atlas, &line, x, yy, argb(0xff00_0000 | color), true);
                    yy += 12.0;
                }
            }
            Kind::List { .. } => {
                let label_r = [x - 15.0, y, ew + 15.0, 24.0];
                let insert = [x - 15.0 + 12.0, y + 3.0, 12.0, 12.0];
                let delete = [x - 15.0 + 25.0, y + 3.0, 12.0, 12.0];
                let in_label = inside(cursor, label_r);
                let in_insert = inside(cursor, insert);
                let in_delete = inside(cursor, delete);
                let focused_cell = match focus {
                    Some(Focus::Cell(fa, c)) if fa == at && e.expanded => Some(c),
                    _ => None,
                };
                let plain = in_label && !in_insert && !in_delete;
                let arrow = match (e.expanded, plain) {
                    (false, false) => &sprites::EXPAND,
                    (true, false) => &sprites::EXPAND_OPEN,
                    (false, true) => &sprites::EXPAND_HOVER,
                    (true, true) => &sprites::EXPAND_OPEN_HOVER,
                };
                blit(game, batch, arrow, x - 15.0, y + 5.0);
                blit(game, batch, if in_insert { &sprites::INSERT_HOVER } else { &sprites::INSERT }, x - 15.0 + 13.0, y + 5.0);
                let del = match (focused_cell, in_delete) {
                    (None, _) => &sprites::DELETE_OFF,
                    (Some(_), true) => &sprites::DELETE_HOVER,
                    (Some(_), false) => &sprites::DELETE,
                };
                blit(game, batch, del, x - 15.0 + 26.0, y + 5.0);
                button(game, batch, reset_r, &reset_label, can_reset);
                let on_reset = inside(cursor, reset_r);
                let color = if in_label && !on_reset && !in_delete && !in_insert { argb(0xffe6_fe16) } else { base };
                batch.text_styled(atlas, &name, x + 24.0, y + 6.0, color, true);
                if e.expanded {
                    let mut yy = y + 24.0;
                    for (c, cell) in e.cells.iter_mut().enumerate() {
                        cell.x = x + 14.0;
                        cell.y = yy + 1.0;
                        cell.width = ew - 14.0 - 12.0;
                        cell.height = 18.0;
                        cell.focused = focused_cell == Some(c);
                        cell.text_color = rgba(224, 224, 224, 255);
                        cell.draw(batch, atlas, None);
                        if cell.focused {
                            fill(game, batch, [x + 14.0, yy + 12.0, ew - 14.0 - 12.0, 1.0], 0xffe0_e0e0);
                        }
                        yy += 20.0;
                    }
                }
                if hovered && in_insert {
                    tip = Some(vec![tr(ru, "add").to_owned()]);
                } else if hovered && in_delete && focused_cell.is_some() {
                    tip = Some(vec![tr(ru, "remove").to_owned()]);
                }
            }
            Kind::Sub => {
                // `SubCategoryListEntry`: the arrow (plain, or yellow under
                // the pointer) and the name, white or yellow under its styles.
                let in_label = inside(cursor, [x - 15.0, y, ew + 15.0, 24.0]);
                let arrow = match (e.expanded, in_label) {
                    (false, false) => &sprites::SUB,
                    (true, false) => &sprites::SUB_OPEN,
                    (false, true) => &sprites::SUB_HOVER,
                    (true, true) => &sprites::SUB_OPEN_HOVER,
                };
                blit(game, batch, arrow, x - 15.0, y + 5.0);
                let color = if in_label { argb(0xffe6_fe16) } else { u32::MAX };
                batch.text_styled(atlas, &name, x, y + 6.0, color, true);
            }
        }
        // `wrapLinesToScreen`.
        tip.map(|lines| lines.iter().flat_map(|s| wrap(game, s, l.w)).collect())
    }

    // ---- input ----------------------------------------------------------------------------

    fn click(&mut self) {
        self.requests.push(Request::Click);
    }

    /// `quit`: "Changes Not Saved" when something changed.
    fn quit(&mut self) {
        if self.edited(russian()) {
            self.confirm = true;
        } else {
            self.requests.push(Request::Close(false));
        }
    }

    pub(super) fn press(&mut self, game: &Game<'_>, w: f32, h: f32, p: (f32, f32), button: i32, mods: u32) {
        let ru = russian();
        if self.confirm {
            if button == 0 {
                let atlas = &game.gui.atlas;
                let c = horizon_client::confirm_screen::Confirm::new(&atlas.font, (w, h), &Text::literal(tr(ru, "quit_config")), &Text::literal(tr(ru, "quit_config_sure")));
                match c.answer_at(p) {
                    Some(true) => {
                        self.click();
                        self.confirm = false;
                        self.requests.push(Request::Close(false));
                    }
                    Some(false) => {
                        self.click();
                        self.confirm = false;
                        self.reinit();
                    }
                    None => {}
                }
            }
            return;
        }
        // A key field waiting: the mouse button is the key (`mouseClicked`).
        if self.binding.is_some() {
            let name = mouse_name(button);
            if self.started.0 == UNBOUND {
                self.started = (name, 0);
            } else if self.started.0.starts_with("key.keyboard.") {
                let bit = if mods & CONTROL != 0 { CONTROL } else if mods & SHIFT != 0 { SHIFT } else if mods & ALT != 0 { ALT } else { 0 };
                if bit != 0 {
                    self.started = (name, self.started.1 | bit);
                }
            }
            return;
        }
        let left = button == 0;
        let l = self.layout(game, w, h);
        let (cancel, save) = Self::footer(&l);
        if left && inside(p, cancel) {
            self.click();
            self.quit();
            return;
        }
        if inside(p, save) {
            if left && self.edited(ru) && self.errors(ru).is_empty() {
                self.click();
                self.requests.push(Request::Close(true));
            }
            return;
        }
        if l.tabs {
            if left && inside(p, [4.0, 44.0, 12.0, 18.0]) {
                if self.tab_scroll.value > 0.0 {
                    self.click();
                    self.tab_scroll.set_to(0.0, 600.0);
                }
                return;
            }
            if left && inside(p, [w - 16.0, 44.0, 12.0, 18.0]) {
                if self.tab_scroll.value < l.tab_max {
                    self.click();
                    self.tab_scroll.set_to(l.tab_max, 600.0);
                }
                return;
            }
            if p.0 >= 20.0 && p.0 < w - 20.0 {
                for (i, &(x, cw)) in l.tab_xs.iter().enumerate() {
                    if inside(p, [x, 43.0, cw, 20.0]) {
                        if left && i != self.selected {
                            self.click();
                            self.selected = i;
                            self.reinit();
                        }
                        return;
                    }
                }
            }
        }
        if p.1 < l.top || p.1 > l.bottom {
            return;
        }
        // The scroll bar.
        if left && l.max_scroll > 0.0 && p.0 >= w - 36.0 && p.0 < w - 30.0 {
            self.scrolling = Some(p.1);
            return;
        }
        let atlas = &game.gui.atlas;
        let shift = mods & SHIFT != 0;
        let rw = width(game, tr(ru, "reset")) + 6.0;
        let focus = self.focus;
        for row in &l.rows {
            if row.y + row.h < l.top || row.y > l.bottom {
                continue;
            }
            match row.kind {
                RowKind::Gap => {}
                RowKind::Search => {
                    if inside(p, [self.search.x, self.search.y, self.search.width, self.search.height]) {
                        self.focus_on(Focus::Search);
                        self.search.focused = true;
                        self.search.click(atlas, p.0, shift, false);
                        self.search.reset_blink();
                        return;
                    }
                }
                RowKind::Entry(at) => {
                    let (x, y, ew) = (row.x, row.y, row.w);
                    let reset_r = [x + ew - rw, y, rw, 20.0];
                    let value_r = [x + ew - 150.0, y, 150.0 - rw - 2.0, 20.0];
                    let Some(e) = self.entry_mut(at) else {
                        continue;
                    };
                    if !e.enabled {
                        continue;
                    }
                    let can_reset = e.default.is_some() && !e.matches_default();
                    let on_reset = left && can_reset && inside(p, reset_r);
                    match e.kind {
                        Kind::Bool | Kind::Select(_) | Kind::Key => {
                            if on_reset {
                                e.reset();
                                self.binding = None;
                                self.focus_on(Focus::Widget(at));
                                self.click();
                                return;
                            }
                            if left && inside(p, value_r) {
                                let mut bind = false;
                                match (&e.kind, &mut e.value) {
                                    (Kind::Bool, Value::Bool(b)) => *b = !*b,
                                    (Kind::Select(names), Value::Choice(c)) => *c = (*c + 1) % names.len().max(1),
                                    (Kind::Key, _) => bind = true,
                                    _ => {}
                                }
                                if bind {
                                    self.binding = Some(at);
                                    self.started = (UNBOUND.into(), 0);
                                }
                                self.focus_on(Focus::Widget(at));
                                self.click();
                                return;
                            }
                        }
                        Kind::Int | Kind::Float | Kind::Str => {
                            if on_reset {
                                e.reset();
                                self.focus_on(Focus::Widget(at));
                                self.click();
                                return;
                            }
                            let f = &mut e.field;
                            if inside(p, [f.x, f.y, f.width, f.height]) {
                                f.click(atlas, p.0, shift, false);
                                f.reset_blink();
                                self.focus_on(Focus::Field(at));
                                return;
                            }
                        }
                        Kind::Slider { .. } => {
                            // `AbstractSliderButton.onClick`: the handle to the
                            // pointer; the click sounds when the button comes up.
                            if left && inside(p, e.slider) {
                                e.slide_to_mouse(p.0);
                                self.focus_on(Focus::Slider(at));
                                self.slider_live = true;
                                self.sliding = Some(at);
                                return;
                            }
                            if on_reset {
                                e.reset();
                                self.focus_on(Focus::Widget(at));
                                self.click();
                                return;
                            }
                        }
                        Kind::Text(..) => {}
                        Kind::List { insert_front } => {
                            if on_reset {
                                e.reset();
                                self.focus_on(Focus::Widget(at));
                                self.click();
                                return;
                            }
                            let label_r = [x - 15.0, y, ew + 15.0, 24.0];
                            if left && inside(p, label_r) && !inside(p, reset_r) {
                                if inside(p, [x - 15.0 + 12.0, y + 3.0, 12.0, 12.0]) {
                                    e.expanded = true;
                                    let cell = field_box("", false);
                                    if insert_front {
                                        e.cells.insert(0, cell);
                                    } else {
                                        e.cells.push(cell);
                                    }
                                    self.focus_on(Focus::Widget(at));
                                    self.click();
                                } else if inside(p, [x - 15.0 + 25.0, y + 3.0, 12.0, 12.0]) {
                                    if let Some(Focus::Cell(fa, c)) = focus
                                        && fa == at
                                        && e.expanded
                                        && c < e.cells.len()
                                    {
                                        e.cells.remove(c);
                                        self.focus_on(Focus::Widget(at));
                                        self.click();
                                    }
                                } else {
                                    e.expanded = !e.expanded;
                                    self.focus_on(Focus::Widget(at));
                                    self.click();
                                }
                                return;
                            }
                            if e.expanded {
                                for (c, cell) in e.cells.iter_mut().enumerate() {
                                    if inside(p, [cell.x, cell.y - 1.0, cell.width + 12.0, 20.0]) {
                                        cell.click(atlas, p.0, shift, false);
                                        cell.reset_blink();
                                        self.focus_on(Focus::Cell(at, c));
                                        return;
                                    }
                                }
                            }
                        }
                        Kind::Sub => {
                            // `CategoryLabelWidget.mouseClicked`: any button
                            // on the label opens or closes it.
                            if inside(p, [x - 15.0, y, ew + 15.0, 24.0]) {
                                e.expanded = !e.expanded;
                                self.focus_on(Focus::Widget(at));
                                self.click();
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn release(&mut self, button: i32) {
        self.scrolling = None;
        // `AbstractSliderButton.onRelease`: the click sound.
        if button == 0 && self.sliding.take().is_some() {
            self.click();
        }
        // `mouseReleased`: once something was pressed for a key field, it is
        // its key.
        if let Some(at) = self.binding
            && self.started.0 != UNBOUND
        {
            let started = self.started.clone();
            if let Some(e) = self.entry_mut(at) {
                e.value = Value::Key(started.0, started.1);
            }
            self.binding = None;
        }
    }

    /// The scroll bar or a slider dragged (`mouseDragged`).
    pub(super) fn dragged(&mut self, game: &Game<'_>, w: f32, h: f32, p: (f32, f32)) {
        if let Some(at) = self.sliding {
            if let Some(e) = self.entry_mut(at) {
                e.slide_to_mouse(p.0);
            }
            return;
        }
        let Some(last) = self.scrolling else {
            return;
        };
        self.scrolling = Some(p.1);
        let l = self.layout(game, w, h);
        if p.1 < l.top {
            self.scroll.set(0.0);
        } else if p.1 > l.bottom {
            self.scroll.set(l.max_scroll);
        } else {
            let lh = l.bottom - l.top;
            let thumb = (lh * lh / l.content).floor().clamp(32.0, lh - 8.0);
            let rate = (l.max_scroll.max(1.0) / (lh - thumb)).max(1.0);
            self.scroll.set((self.scroll.value + (p.1 - last) * rate).clamp(0.0, l.max_scroll));
        }
    }

    /// The wheel: the tab row 16 pixels a notch, the list 16 (`getScrollStep`).
    pub(super) fn scrolled(&mut self, game: &Game<'_>, w: f32, h: f32, p: (f32, f32), delta: f32) {
        if self.confirm || delta == 0.0 {
            return;
        }
        let l = self.layout(game, w, h);
        if l.tabs && inside(p, [18.0, 41.0, w - 36.0, 24.0]) {
            let t = (self.tab_scroll.target - delta * 16.0).clamp(0.0, l.tab_max);
            self.tab_scroll.set_to(t, 600.0);
            return;
        }
        if p.1 >= l.top && p.1 <= l.bottom {
            let t = (self.scroll.target - delta * 16.0).clamp(0.0, l.max_scroll);
            self.scroll.set_to(t, 600.0);
        }
    }

    fn focused_box(&mut self) -> Option<(&mut EditBox, bool)> {
        match self.focus? {
            Focus::Search => Some((&mut self.search, false)),
            Focus::Field(at) => {
                let e = self.entry_mut(at)?;
                let numeric = matches!(e.kind, Kind::Int | Kind::Float);
                Some((&mut e.field, numeric))
            }
            Focus::Cell(at, c) => {
                let e = self.entry_mut(at)?;
                if !e.expanded {
                    return None;
                }
                Some((e.cells.get_mut(c)?, false))
            }
            Focus::Slider(_) | Focus::Widget(_) => None,
        }
    }

    pub(super) fn key_pressed(&mut self, game: &Game<'_>, w: f32, h: f32, key: &str, mods: u32) {
        let _ = (game, w, h);
        let escape = key == "key.keyboard.escape";
        if self.confirm {
            if escape {
                self.confirm = false;
                self.reinit();
            }
            return;
        }
        // A key field waiting (`keyPressed`): Escape unbinds it.
        if let Some(at) = self.binding {
            if escape {
                if let Some(e) = self.entry_mut(at) {
                    e.value = Value::Key(UNBOUND.into(), 0);
                }
                self.binding = None;
                return;
            }
            let held = |bit: u32| mods & bit != 0;
            if self.started.0 == UNBOUND {
                self.started.0 = key.to_owned();
            } else {
                if self.started.0.starts_with("key.keyboard.") {
                    for bit in [CONTROL, SHIFT, ALT] {
                        if held(bit) {
                            self.started = (key.to_owned(), self.started.1 | bit);
                            return;
                        }
                    }
                }
                for bit in [CONTROL, SHIFT, ALT] {
                    if held(bit) {
                        self.started.1 |= bit;
                        return;
                    }
                }
            }
            return;
        }
        // The focused slider (`AbstractSliderButton.keyPressed`): Enter or
        // Space lets the arrows move it, or stops them; the arrows move the
        // handle a pixel.
        if !escape && let Some(Focus::Slider(at)) = self.focus {
            match key {
                "key.keyboard.enter" | "key.keyboard.space" | "key.keyboard.keypad.enter" => {
                    self.slider_live = !self.slider_live;
                }
                "key.keyboard.left" | "key.keyboard.right" if self.slider_live => {
                    if let Some(e) = self.entry_mut(at) {
                        let step = if key == "key.keyboard.left" { -1.0 } else { 1.0 };
                        let to = e.position + step / f64::from(e.slider[2] - 8.0);
                        e.slide(to);
                    }
                }
                _ => {}
            }
            return;
        }
        let control = mods & CONTROL != 0;
        let shift = mods & SHIFT != 0;
        if !escape && self.focused_box().is_some() {
            let request = match key {
                "key.keyboard.v" if control => Some(Request::Paste),
                "key.keyboard.c" if control => self.focused_box().map(|(b, _)| Request::Copy(b.selected_text())),
                "key.keyboard.x" if control => self.focused_box().map(|(b, _)| {
                    let t = b.selected_text();
                    b.delete_chars(0);
                    Request::Copy(t)
                }),
                _ if !control && writes(key) => None,
                _ => {
                    if let Some(code) = horizon_client::keybinds::key_code(key)
                        && let Some((b, _)) = self.focused_box()
                    {
                        b.key(code, control, shift);
                        b.reset_blink();
                    }
                    None
                }
            };
            self.requests.extend(request);
            return;
        }
        if escape {
            self.quit();
        }
    }

    pub(super) fn key_released(&mut self) {
        // `keyReleased`: what was pressed for a key field is its key
        // (nothing yet unbinds it, as in Cloth Config).
        if let Some(at) = self.binding {
            let started = self.started.clone();
            if let Some(e) = self.entry_mut(at) {
                e.value = Value::Key(started.0, started.1);
            }
            self.binding = None;
        }
    }

    /// Typed text into the focused field (a number field takes digits, `-`
    /// and `.` only, `AbstractNumberListEntry.stripAddText`).
    pub(super) fn typed(&mut self, text: &str) {
        if self.confirm || self.binding.is_some() {
            return;
        }
        if let Some((b, numeric)) = self.focused_box() {
            let text: String = text
                .chars()
                .filter(|c| !c.is_control() && (!numeric || c.is_ascii_digit() || *c == '-' || *c == '.'))
                .collect();
            if !text.is_empty() {
                b.insert_text(&text);
                b.reset_blink();
            }
        }
    }
}

/// A GLFW mouse button's key name.
fn mouse_name(button: i32) -> String {
    match button {
        0 => "key.mouse.left".into(),
        1 => "key.mouse.right".into(),
        2 => "key.mouse.middle".into(),
        n => format!("key.mouse.{}", n + 1),
    }
}
