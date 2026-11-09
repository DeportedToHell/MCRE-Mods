// Cloth Config for Horizon: the screen Cloth Config 26.2 builds
// (`ClothConfigScreen`) with one category, as BetterF3's screens are: the
// title, the list with its search field and the rows BetterF3 uses
// (`BooleanListEntry`, `IntegerListEntry`, `DoubleListEntry`,
// `StringListEntry`, `ColorEntry` with its `ColorDisplayWidget`,
// `DropdownBoxEntry` with its menu), the error line, Cancel and Save & Quit
// and the "Changes Not Saved" confirmation, laid out with Cloth Config's own
// numbers in the game's widgets. Rewritten from Cloth Config by shedaniel
// (LGPL-3.0, https://github.com/shedaniel/cloth-config), after our ClientSort
// port's copy.
use super::Game;
use super::betterf3_cloth_sprites::{self as sprites, Sprite};
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::text::Text;
use horizon_client::ui::{self, EditBox};
use std::time::Instant;

type Rect = [f32; 4];

/// GLFW's modifier bits, as the screen hooks pass them.
pub(super) const SHIFT: u32 = 1;
pub(super) const CONTROL: u32 = 2;

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
        ("config", false) => "Config",
        ("config", true) => "Настройки",
        ("not_int", false) => "Not a valid number! (Integer)",
        ("not_int", true) => "Недопустимое число! (Целое)",
        ("not_double", false) => "Not a valid number! (Double)",
        ("not_double", true) => "Недопустимое число! (Двойное)",
        // `%d` is no format `TranslatableContents` takes: the text stays as it is.
        ("too_large", false) => "Too Large! (Maximum: %d)",
        ("too_large", true) => "Слишком большой! (Максимум: %d)",
        ("too_small", false) => "Too Small! (Minimum: %d)",
        ("too_small", true) => "Слишком маленький! (Минимум: %d)",
        ("no_alpha_allowed", false) => "No Alpha Allowed!",
        ("no_alpha_allowed", true) => "Альфа не допускается!",
        ("invalid_color", false) => "Not a valid color!",
        ("invalid_color", true) => "Недопустимый цвет!",
        ("no_suggestions", false) => "§cNo suggestions",
        ("no_suggestions", true) => "§cНет предложений",
        _ => "",
    }
}

/// The default title (`text.cloth-config.config`).
pub(super) fn default_title(ru: bool) -> &'static str {
    tr(ru, "config")
}

fn russian() -> bool {
    super::betterf3::russian()
}

// ---- values and entries ---------------------------------------------------------------------

/// An entry's value as the screen edits it.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Value {
    Bool(bool),
    /// A number, string, colour or dropdown field's text.
    Text(String),
}

impl Value {
    pub(super) fn as_bool(&self) -> bool {
        matches!(self, Value::Bool(true))
    }
    pub(super) fn as_text(&self) -> &str {
        match self {
            Value::Text(s) => s,
            Value::Bool(_) => "",
        }
    }
}

/// What kind of row an entry is.
#[derive(Clone, Debug)]
pub(super) enum Kind {
    Bool,
    Int,
    Double { min: f64, max: f64 },
    Str,
    /// `ColorEntry`, ARGB with `alpha`, else RGB.
    Color { alpha: bool },
    /// `DropdownBoxEntry`: the selections' names, in order.
    Dropdown(Vec<String>),
}

/// `ColorEntry.getColorValue`: the colour, or the error's key.
pub(super) fn parse_color(s: &str, alpha: bool) -> Result<u32, &'static str> {
    let color = if let Some(hex) = s.strip_prefix('#') {
        if hex.chars().count() > 8 {
            return Err("invalid_color");
        }
        if !alpha && hex.chars().count() > 6 {
            return Err("no_alpha_allowed");
        }
        i64::from_str_radix(hex, 16).map_err(|_| "invalid_color")? as i32
    } else {
        s.parse::<i64>().map_err(|_| "invalid_color")? as i32
    };
    let a = (color >> 24) & 0xFF;
    if !alpha && a > 0 {
        return Err("no_alpha_allowed");
    }
    Ok(color as u32)
}

/// `ColorEntry.getHexColorString`.
pub(super) fn hex_color(color: u32, alpha: bool) -> String {
    let hex = format!("{color:x}");
    let width = if alpha { 8 } else { 6 };
    format!("#{hex:0>width$}")
}

/// `String.valueOf(double)` for the values BetterF3 keeps.
pub(super) fn java_double(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e7 {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}

/// One row (`AbstractConfigListEntry`).
pub(super) struct Entry {
    pub(super) id: String,
    name: String,
    kind: Kind,
    tooltip: Vec<String>,
    value: Value,
    original: Value,
    default: Value,
    /// The number, string, colour or dropdown field.
    field: EditBox,
    /// A dropdown's menu scroll.
    menu_scroll: f32,
}

fn field_box(text: &str, bordered: bool) -> EditBox {
    let mut b = EditBox::new(0.0, 0.0, 100.0, 18.0);
    b.max_length = 999_999;
    b.bordered = bordered;
    b.focused = false;
    b.set_value(text);
    if !bordered {
        b.set_cursor(0, false);
    }
    b
}

impl Entry {
    fn new(id: &str, name: &str, kind: Kind, value: Value, default: Value) -> Entry {
        let bordered = !matches!(kind, Kind::Dropdown(_));
        Entry {
            id: id.to_owned(),
            name: name.to_owned(),
            field: field_box(value.as_text(), bordered),
            kind,
            tooltip: Vec::new(),
            original: value.clone(),
            value,
            default,
            menu_scroll: 0.0,
        }
    }
    pub(super) fn boolean(id: &str, name: &str, value: bool, default: bool) -> Entry {
        Entry::new(id, name, Kind::Bool, Value::Bool(value), Value::Bool(default))
    }
    pub(super) fn int(id: &str, name: &str, value: i64, default: i64) -> Entry {
        Entry::new(id, name, Kind::Int, Value::Text(value.to_string()), Value::Text(default.to_string()))
    }
    pub(super) fn double(id: &str, name: &str, value: f64, default: f64, min: f64, max: f64) -> Entry {
        let k = Kind::Double { min, max };
        Entry::new(id, name, k, Value::Text(java_double(value)), Value::Text(java_double(default)))
    }
    pub(super) fn string(id: &str, name: &str, value: &str, default: &str) -> Entry {
        Entry::new(id, name, Kind::Str, Value::Text(value.into()), Value::Text(default.into()))
    }
    pub(super) fn color(id: &str, name: &str, value: u32, default: u32, alpha: bool) -> Entry {
        let k = Kind::Color { alpha };
        Entry::new(id, name, k, Value::Text(hex_color(value, alpha)), Value::Text(hex_color(default, alpha)))
    }
    pub(super) fn dropdown(id: &str, name: &str, choices: Vec<String>, value: &str, default: &str) -> Entry {
        Entry::new(id, name, Kind::Dropdown(choices), Value::Text(value.into()), Value::Text(default.into()))
    }
    pub(super) fn tooltip(mut self, lines: &[String]) -> Entry {
        self.tooltip = lines.to_vec();
        self
    }

    /// The value now (`getValue`): a field's text.
    pub(super) fn current(&self) -> Value {
        match self.kind {
            Kind::Bool => self.value.clone(),
            _ => Value::Text(self.field.value.clone()),
        }
    }

    /// `ColorEntry.getValue`: the colour (the original's while it is wrong).
    pub(super) fn color_value(&self) -> u32 {
        match self.kind {
            Kind::Color { alpha } => parse_color(&self.field.value, alpha)
                .unwrap_or_else(|_| parse_color(self.original.as_text(), alpha).unwrap_or(0)),
            _ => 0,
        }
    }

    /// `DropdownBoxEntry.getValue`: the choice, or the original while the
    /// text names none.
    pub(super) fn choice(&self) -> String {
        match &self.kind {
            Kind::Dropdown(choices) if choices.iter().any(|c| *c == self.field.value) => self.field.value.clone(),
            _ => self.original.as_text().to_owned(),
        }
    }

    /// `getConfigError`.
    fn error(&self, ru: bool) -> Option<String> {
        let text = &self.field.value;
        match &self.kind {
            Kind::Int if text.parse::<i32>().is_err() => Some(tr(ru, "not_int").into()),
            Kind::Double { min, max } => match text.trim().parse::<f64>() {
                Err(_) => Some(tr(ru, "not_double").into()),
                Ok(v) if v > *max => Some(tr(ru, "too_large").into()),
                Ok(v) if v < *min => Some(tr(ru, "too_small").into()),
                Ok(_) => None,
            },
            Kind::Color { alpha } => parse_color(text, *alpha).err().map(|e| tr(ru, e).to_owned()),
            Kind::Dropdown(choices) if !choices.iter().any(|c| c == text) => Some("Invalid Value!".into()),
            _ => None,
        }
    }

    /// `isEdited`.
    fn edited(&self, error: bool) -> bool {
        match &self.kind {
            Kind::Bool => self.value != self.original,
            Kind::Color { alpha } => {
                error || parse_color(&self.field.value, *alpha).ok() != parse_color(self.original.as_text(), *alpha).ok()
            }
            Kind::Dropdown(_) => error || self.choice() != self.original.as_text(),
            _ => self.current() != self.original,
        }
    }

    fn matches_default(&self) -> bool {
        match &self.kind {
            Kind::Color { alpha } => {
                parse_color(&self.field.value, *alpha).is_ok_and(|c| Ok(c) == parse_color(self.default.as_text(), *alpha))
            }
            _ => self.current() == self.default,
        }
    }

    fn reset(&mut self) {
        match self.kind {
            Kind::Bool => self.value = self.default.clone(),
            _ => {
                let d = self.default.as_text().to_owned();
                self.field.set_value(&d);
                if matches!(self.kind, Kind::Dropdown(_)) {
                    self.field.set_cursor(0, false);
                }
            }
        }
    }

    /// `getSearchTags`: the name's words.
    fn matches(&self, words: &[String]) -> bool {
        if words.is_empty() {
            return true;
        }
        let tags: Vec<String> = self.name.split(' ').map(str::to_lowercase).collect();
        words.iter().all(|w| tags.iter().any(|t| t.contains(w.as_str())))
    }

    /// A dropdown's selections matching the text typed (`search`), most
    /// alike first.
    fn suggestions(&self) -> Vec<String> {
        let Kind::Dropdown(choices) = &self.kind else {
            return Vec::new();
        };
        let keyword = self.field.value.to_lowercase();
        let mut found: Vec<String> = choices.iter().filter(|c| c.to_lowercase().contains(&keyword)).cloned().collect();
        if !keyword.is_empty() {
            found.sort_by(|a, b| similarity(b, &keyword).total_cmp(&similarity(a, &keyword)));
        }
        found
    }
}

/// `DefaultDropdownMenuElement.similarity`.
fn similarity(a: &str, b: &str) -> f64 {
    let (longer, shorter) = if a.chars().count() < b.chars().count() { (b, a) } else { (a, b) };
    let n = longer.chars().count();
    if n == 0 {
        return 1.0;
    }
    (n - edit_distance(longer, shorter)) as f64 / n as f64
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();
    let mut costs: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut last = i;
        for j in 1..=b.len() {
            let mut new = costs[j - 1];
            if a[i - 1] != b[j - 1] {
                new = new.min(last).min(costs[j]) + 1;
            }
            costs[j - 1] = last;
            last = new;
        }
        costs[b.len()] = last;
    }
    costs[b.len()]
}

// ---- the screen -----------------------------------------------------------------------------

/// What has the keyboard: the search field, an entry's field, or another of
/// an entry's widgets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Focus {
    Search,
    Field(usize),
    Widget(usize),
}

/// What the screen asks of the game, for `frame`.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Request {
    Click,
    Paste,
    Copy(String),
    /// Leave the screen: Save & Quit (true) or discarded (false).
    Close(bool),
}

/// `ValueAnimator`'s easing toward a target.
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
    fn update(&mut self, dt: f32) {
        if self.duration == 0.0 || self.value == self.target {
            return;
        }
        let t = (self.start.elapsed().as_secs_f32() * 1000.0 / self.duration * dt * 3.0).min(1.0);
        let (a, b) = (self.value, self.target);
        self.value = if a < b { (a + 2.0 * (b - a) * t).min(b) } else { (a - 2.0 * (a - b) * t).max(b) };
    }
}

#[derive(Clone, Copy, Debug)]
enum RowKind {
    Gap,
    Search,
    Entry(usize),
}

#[derive(Clone, Copy, Debug)]
struct Row {
    kind: RowKind,
    y: f32,
    h: f32,
}

struct Layout {
    w: f32,
    h: f32,
    top: f32,
    bottom: f32,
    x: f32,
    entry_w: f32,
    rows: Vec<Row>,
    content: f32,
    max_scroll: f32,
    footer_w: f32,
}

/// `SelectionCellCreator`: 132 wide, 14 a cell, 7 cells at most.
const CELL_W: f32 = 132.0;
const CELL_H: f32 = 14.0;
const MENU_MAX: f32 = CELL_H * 7.0;

pub(super) struct Screen {
    title: String,
    pub(super) entries: Vec<Entry>,
    search: EditBox,
    focus: Option<Focus>,
    scroll: Anim,
    scrolling: Option<f32>,
    hover: [Anim; 4],
    last_touch: Instant,
    last_frame: Instant,
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

/// A key that writes text when typed (its characters come through the
/// character hook).
fn writes(key: &str) -> bool {
    let Some(name) = key.strip_prefix("key.keyboard.") else {
        return false;
    };
    name.chars().count() == 1 || name == "space" || name.starts_with("keypad.") && name.len() == 8
}

impl Screen {
    pub(super) fn new(title: &str, entries: Vec<Entry>, in_world: bool) -> Screen {
        let mut search = EditBox::new(0.0, 0.0, 100.0, 18.0);
        search.focused = false;
        Screen {
            title: title.to_owned(),
            entries,
            search,
            focus: None,
            scroll: Anim::new(),
            scrolling: None,
            hover: [Anim::new(); 4],
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

    fn reinit(&mut self) {
        self.scroll.set(0.0);
        self.search.set_value("");
        self.search.focused = false;
        self.focus = None;
        self.scrolling = None;
    }

    fn errors(&self, ru: bool) -> Vec<String> {
        self.entries.iter().filter_map(|e| e.error(ru)).collect()
    }

    pub(super) fn edited(&self, ru: bool) -> bool {
        self.entries.iter().any(|e| e.edited(e.error(ru).is_some()))
    }

    pub(super) fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    fn search_words(&self) -> Vec<String> {
        let v = self.search.value.to_lowercase();
        if v.is_empty() { Vec::new() } else { v.split(' ').map(str::to_owned).collect() }
    }

    fn layout(&self, w: f32, h: f32) -> Layout {
        let top = 30.0;
        let bottom = h - 32.0;
        let item_w = w - 80.0;
        let x = (w / 2.0).floor() - (item_w / 2.0).floor() + 2.0;
        let scroll = self.scroll.value.floor();
        let mut rows = Vec::new();
        let mut y = top + 4.0 - scroll;
        let mut push = |rows: &mut Vec<Row>, kind: RowKind, h: f32| {
            rows.push(Row { kind, y, h });
            y += h;
        };
        push(&mut rows, RowKind::Gap, 5.0);
        push(&mut rows, RowKind::Search, 24.0);
        push(&mut rows, RowKind::Gap, 5.0);
        let words = self.search_words();
        for (i, e) in self.entries.iter().enumerate() {
            if e.matches(&words) {
                push(&mut rows, RowKind::Entry(i), 24.0);
            }
        }
        let content: f32 = rows.iter().map(|r| r.h).sum();
        let max_scroll = (content - (bottom - top - 4.0)).max(0.0);
        Layout {
            w,
            h,
            top,
            bottom,
            x,
            entry_w: item_w,
            rows,
            content,
            max_scroll,
            footer_w: 200.0f32.min(((w - 50.0 - 12.0) / 3.0).floor()),
        }
    }

    fn footer(l: &Layout) -> (Rect, Rect) {
        let cx = (l.w / 2.0).floor();
        ([cx - l.footer_w - 3.0, l.h - 26.0, l.footer_w, 20.0], [cx + 3.0, l.h - 26.0, l.footer_w, 20.0])
    }

    /// A dropdown's open menu (`isExpanded`): its entry and where the menu's
    /// cells start (`lastRectangle` + its height).
    fn open_menu(&self, l: &Layout) -> Option<(usize, f32, f32)> {
        let Some(Focus::Field(i)) = self.focus else {
            return None;
        };
        let e = self.entries.get(i)?;
        if !matches!(e.kind, Kind::Dropdown(_)) {
            return None;
        }
        let row = l.rows.iter().find(|r| matches!(r.kind, RowKind::Entry(j) if j == i))?;
        Some((i, l.x + l.entry_w - 150.0 + 1.0, row.y - 1.0 + 20.0))
    }

    // ---- drawing --------------------------------------------------------------------------

    pub(super) fn draw(&mut self, game: &Game<'_>, batch: &mut HudBatch) {
        let ru = russian();
        let atlas = &game.gui.atlas;
        let (w, h) = (batch.width.floor(), batch.height.floor());
        let dt = (self.last_frame.elapsed().as_secs_f32() * 20.0).min(5.0);
        self.last_frame = Instant::now();
        game.native_apply_screen_blur();
        ui::menu_background(batch, atlas, 0.0, 0.0, w, h, self.in_world);
        if self.confirm {
            let title = Text::literal(tr(ru, "quit_config"));
            let message = Text::literal(tr(ru, "quit_config_sure"));
            let c = horizon_client::confirm_screen::Confirm::new(&atlas.font, (w, h), &title, &message);
            let labels = [Text::literal(tr(ru, "quit_discard")), Text::translate("gui.cancel", &[])];
            c.draw(batch, atlas, game.cursor, [&labels[0], &labels[1]]);
            return;
        }
        let l = self.layout(w, h);
        self.scroll.target = self.scroll.target.clamp(0.0, l.max_scroll);
        self.scroll.update(dt);
        self.scroll.value = self.scroll.value.clamp(0.0, l.max_scroll);
        let l = self.layout(w, h);
        let cursor = game.cursor;
        let mut tip: Option<Vec<Text>> = None;

        fill(game, batch, [0.0, l.top, w, l.bottom - l.top], 0x4000_0000);
        let start = batch.vertices.len();
        let over_list = cursor.1 >= l.top && cursor.1 <= l.bottom;
        let scroll = self.scroll.value.floor();
        let menu = self.open_menu(&l);
        let over_menu = menu.is_some_and(|(i, mx, my)| {
            let n = self.entries[i].suggestions().len() as f32;
            let mh = (CELL_H * n).min(MENU_MAX).max(14.0);
            inside(cursor, [mx, my, CELL_W + 1.0, mh + 2.0])
        });
        let mut target = None;
        for row in &l.rows {
            if let RowKind::Entry(_) | RowKind::Search = row.kind {
                let area = [0.0, row.y, w, row.h - 4.0];
                if over_list && !over_menu && inside(cursor, area) {
                    target = Some([area[0], area[1] + scroll, area[2], area[3]]);
                }
            }
        }
        if let Some(t) = target {
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
        let errors: Vec<Option<String>> = self.entries.iter().map(|e| e.error(ru)).collect();
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
                RowKind::Entry(i) => {
                    let error = errors.get(i).cloned().flatten();
                    let row_tip = self.draw_entry(game, batch, &l, row, i, error, ru, over_list && !over_menu);
                    if row_tip.is_some() {
                        tip = row_tip;
                    }
                }
            }
        }
        // `lateRender`: the open dropdown's menu over the rows.
        if let Some((i, mx, my)) = menu {
            self.draw_menu(game, batch, i, mx, my);
        }
        batch.clip_since(start, 0.0, l.top, w, l.bottom);
        ui::separators(batch, atlas, l.top, l.bottom, false);
        if l.max_scroll > 0.0 {
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

        // The title.
        let tw = width(game, &self.title);
        batch.text(atlas, &self.title, ((w - tw) / 2.0).floor(), 12.0, u32::MAX, true);

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
        i: usize,
        error: Option<String>,
        ru: bool,
        over_list: bool,
    ) -> Option<Vec<Text>> {
        let atlas = &game.gui.atlas;
        let cursor = game.cursor;
        let (x, y, ew) = (l.x, row.y, l.entry_w);
        let focus = self.focus;
        let reset_label = Text::literal(tr(ru, "reset"));
        let rw = width(game, tr(ru, "reset")) + 6.0;
        let e = self.entries.get_mut(i)?;
        let edited = e.edited(error.is_some());
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
        let area = [0.0, y, l.w, 20.0];
        let hovered = over_list && inside(cursor, area);
        let tip = hovered.then(|| e.tooltip.clone()).filter(|t| !t.is_empty());
        let reset_r = [x + ew - rw, y, rw, 20.0];
        batch.text_styled(atlas, &name, x, y + 6.0, base, true);
        match &e.kind {
            Kind::Bool => {
                button(game, batch, reset_r, &reset_label, !e.matches_default());
                let label = Text::legacy(tr(ru, if e.value.as_bool() { "yes" } else { "no" }));
                button(game, batch, [x + ew - 150.0, y, 150.0 - rw - 2.0, 20.0], &label, true);
            }
            Kind::Int | Kind::Double { .. } | Kind::Str | Kind::Color { .. } => {
                button(game, batch, reset_r, &reset_label, !e.matches_default());
                let focused = focus == Some(Focus::Field(i));
                let f = &mut e.field;
                f.x = x + ew - 148.0;
                f.y = y + 1.0;
                f.width = 148.0 - rw - 4.0;
                f.height = 18.0;
                f.focused = focused;
                f.text_color = if error.is_some() { rgba(255, 85, 85, 255) } else { rgba(224, 224, 224, 255) };
                f.draw(batch, atlas, None);
                if let Kind::Color { alpha } = e.kind {
                    // `ColorDisplayWidget`: 20 square left of the field.
                    let shown = parse_color(&e.field.value, alpha).ok();
                    let sx = e.field.x - 23.0;
                    fill(game, batch, [sx, y, 20.0, 20.0], if focused { 0xffff_ffff } else { 0xffa0_a0a0 });
                    fill(game, batch, [sx + 1.0, y + 1.0, 18.0, 18.0], 0xffff_ffff);
                    let c = shown.map(|c| if alpha { c } else { 0xff00_0000 | c });
                    let c = c.unwrap_or_else(|| {
                        let o = parse_color(e.original.as_text(), alpha).unwrap_or(0);
                        if alpha { o } else { 0xff00_0000 | o }
                    });
                    fill(game, batch, [sx + 1.0, y + 1.0, 18.0, 18.0], c);
                }
            }
            Kind::Dropdown(_) => {
                let can_reset = e.current() != e.default || error.is_some();
                button(game, batch, reset_r, &reset_label, can_reset);
                // `SelectionElement`: a frame, white while the entry is
                // focused, round black, the text field inside.
                let focused = matches!(focus, Some(Focus::Field(j) | Focus::Widget(j)) if j == i);
                let bx = x + ew - 150.0 + 1.0;
                let bw = 150.0 - rw - 4.0;
                fill(game, batch, [bx, y, bw, 20.0], if focused { 0xffff_ffff } else { 0xffa0_a0a0 });
                fill(game, batch, [bx + 1.0, y + 1.0, bw - 2.0, 18.0], 0xff00_0000);
                let field_focus = focus == Some(Focus::Field(i));
                let f = &mut e.field;
                f.x = bx + 4.0;
                f.y = y + 6.0;
                f.width = bw - 8.0;
                f.height = 9.0;
                f.focused = field_focus;
                f.text_color = if error.is_some() { rgba(255, 85, 85, 255) } else { rgba(255, 255, 255, 255) };
                f.draw(batch, atlas, None);
            }
        }
        tip.map(|lines| lines.iter().flat_map(|s| wrap(game, s, l.w)).collect())
    }

    /// `DefaultDropdownMenuElement.lateRender`.
    fn draw_menu(&mut self, game: &Game<'_>, batch: &mut HudBatch, i: usize, mx: f32, my: f32) {
        let ru = russian();
        let atlas = &game.gui.atlas;
        let cursor = game.cursor;
        let found = self.entries[i].suggestions();
        let total = CELL_H * found.len() as f32;
        let mh = total.min(MENU_MAX).max(14.0);
        let max_scroll = (total - mh).max(0.0);
        let e = &mut self.entries[i];
        e.menu_scroll = e.menu_scroll.clamp(0.0, max_scroll);
        fill(game, batch, [mx, my, CELL_W, mh + 1.0], 0xffff_ffff);
        fill(game, batch, [mx + 1.0, my + 1.0, CELL_W - 2.0, mh - 1.0], 0xff00_0000);
        let start = batch.vertices.len();
        let cw = if max_scroll > 6.0 { CELL_W - 6.0 } else { CELL_W };
        let mut yy = my - e.menu_scroll.floor();
        for name in &found {
            if yy + CELL_H >= my && yy <= my + mh + 1.0 {
                fill(game, batch, [mx + 1.0, yy, CELL_W - 1.0, CELL_H], 0xff00_0000);
                let over = cursor.0 >= mx && cursor.0 <= mx + cw && cursor.1 >= yy && cursor.1 <= yy + CELL_H;
                if over {
                    fill(game, batch, [mx + 1.0, yy + 1.0, cw - 2.0, CELL_H - 2.0], 0xff19_1919);
                }
                batch.text(atlas, name, mx + 6.0, yy + 3.0, if over { u32::MAX } else { argb(0xff88_8888) }, true);
            }
            yy += CELL_H;
        }
        batch.clip_since(start, mx, my + 1.0, mx + CELL_W - 6.0, my + mh);
        if found.is_empty() {
            let t = Text::legacy(tr(ru, "no_suggestions"));
            let tw = atlas.font.width_text(&t);
            batch.text_styled(atlas, &t, (mx + CELL_W / 2.0 - tw / 2.0).floor(), my + 3.0, u32::MAX, true);
        }
        if max_scroll > 6.0 {
            let bar = ((mh * mh / max_scroll).floor()).clamp(32.0, (mh - 8.0).max(32.0)).max(10.0);
            let by = (e.menu_scroll * (mh - bar) / max_scroll + my + 1.0).clamp(my + 1.0, my + 1.0 + mh - bar);
            ui::blit_sprite(batch, atlas, "gui/sprites/widget/scroller", mx + CELL_W - 6.0, by.floor(), 6.0, bar, u32::MAX);
        }
    }

    // ---- input ----------------------------------------------------------------------------

    fn click(&mut self) {
        self.requests.push(Request::Click);
    }

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
                let c = horizon_client::confirm_screen::Confirm::new(
                    &atlas.font,
                    (w, h),
                    &Text::literal(tr(ru, "quit_config")),
                    &Text::literal(tr(ru, "quit_config_sure")),
                );
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
        let left = button == 0;
        let l = self.layout(w, h);
        // The open dropdown's menu first: a cell picks its selection.
        if let Some((i, mx, my)) = self.open_menu(&l) {
            let found = self.entries[i].suggestions();
            let total = CELL_H * found.len() as f32;
            let mh = total.min(MENU_MAX).max(14.0);
            let max_scroll = (total - mh).max(0.0);
            let cw = if max_scroll > 6.0 { CELL_W - 6.0 } else { CELL_W };
            if p.0 >= mx && p.0 <= mx + CELL_W && p.1 >= my && p.1 <= my + mh + 1.0 {
                let mut yy = my - self.entries[i].menu_scroll.floor();
                for name in found {
                    let shown = yy + CELL_H >= my && yy <= my + mh + 1.0;
                    if shown && p.0 >= mx && p.0 <= mx + cw && p.1 >= yy && p.1 <= yy + CELL_H {
                        let e = &mut self.entries[i];
                        e.field.set_value(&name);
                        e.field.set_cursor(0, false);
                        self.focus = Some(Focus::Widget(i));
                        return;
                    }
                    yy += CELL_H;
                }
                return;
            }
        }
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
        if p.1 < l.top || p.1 > l.bottom {
            return;
        }
        if left && l.max_scroll > 0.0 && p.0 >= w - 36.0 && p.0 < w - 30.0 {
            self.scrolling = Some(p.1);
            return;
        }
        let atlas = &game.gui.atlas;
        let shift = mods & SHIFT != 0;
        let rw = width(game, tr(ru, "reset")) + 6.0;
        self.focus = None;
        for row in &l.rows {
            if row.y + row.h < l.top || row.y > l.bottom {
                continue;
            }
            match row.kind {
                RowKind::Gap => {}
                RowKind::Search => {
                    if inside(p, [self.search.x, self.search.y, self.search.width, self.search.height]) {
                        self.focus = Some(Focus::Search);
                        self.search.focused = true;
                        self.search.click(atlas, p.0, shift, false);
                        self.search.reset_blink();
                        return;
                    }
                }
                RowKind::Entry(i) => {
                    let (x, y, ew) = (l.x, row.y, l.entry_w);
                    let reset_r = [x + ew - rw, y, rw, 20.0];
                    let Some(e) = self.entries.get_mut(i) else {
                        continue;
                    };
                    let error = e.error(ru).is_some();
                    let can_reset = match e.kind {
                        Kind::Dropdown(_) => e.current() != e.default || error,
                        _ => !e.matches_default(),
                    };
                    if left && can_reset && inside(p, reset_r) {
                        e.reset();
                        self.focus = Some(Focus::Widget(i));
                        self.requests.push(Request::Click);
                        return;
                    }
                    match e.kind {
                        Kind::Bool => {
                            if left && inside(p, [x + ew - 150.0, y, 150.0 - rw - 2.0, 20.0]) {
                                if let Value::Bool(b) = &mut e.value {
                                    *b = !*b;
                                }
                                self.focus = Some(Focus::Widget(i));
                                self.requests.push(Request::Click);
                                return;
                            }
                        }
                        Kind::Dropdown(_) => {
                            let bx = x + ew - 150.0 + 1.0;
                            if inside(p, [bx, y, 150.0 - rw - 4.0, 20.0]) {
                                let f = &mut e.field;
                                f.click(atlas, p.0, shift, false);
                                f.reset_blink();
                                self.focus = Some(Focus::Field(i));
                                return;
                            }
                        }
                        _ => {
                            let f = &mut e.field;
                            if inside(p, [f.x, f.y, f.width, f.height]) {
                                f.click(atlas, p.0, shift, false);
                                f.reset_blink();
                                self.focus = Some(Focus::Field(i));
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn release(&mut self) {
        self.scrolling = None;
    }

    pub(super) fn dragged(&mut self, w: f32, h: f32, p: (f32, f32)) {
        let Some(last) = self.scrolling else {
            return;
        };
        self.scrolling = Some(p.1);
        let l = self.layout(w, h);
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

    pub(super) fn scrolled(&mut self, w: f32, h: f32, p: (f32, f32), delta: f32) {
        if self.confirm || delta == 0.0 {
            return;
        }
        let l = self.layout(w, h);
        if let Some((i, mx, my)) = self.open_menu(&l) {
            let n = self.entries[i].suggestions().len() as f32;
            let mh = (CELL_H * n).min(MENU_MAX).max(14.0);
            if p.0 >= mx && p.0 <= mx + CELL_W && p.1 >= my && p.1 <= my + mh + 1.0 {
                let max = (CELL_H * n - mh).max(0.0);
                let e = &mut self.entries[i];
                e.menu_scroll = (e.menu_scroll - delta * 16.0).clamp(0.0, max);
                return;
            }
        }
        if p.1 >= l.top && p.1 <= l.bottom {
            let t = (self.scroll.target - delta * 16.0).clamp(0.0, l.max_scroll);
            self.scroll.set_to(t, 600.0);
        }
    }

    fn focused_box(&mut self) -> Option<(&mut EditBox, bool)> {
        match self.focus? {
            Focus::Search => Some((&mut self.search, false)),
            Focus::Field(i) => {
                let e = self.entries.get_mut(i)?;
                let numeric = matches!(e.kind, Kind::Int | Kind::Double { .. });
                Some((&mut e.field, numeric))
            }
            Focus::Widget(_) => None,
        }
    }

    pub(super) fn key_pressed(&mut self, key: &str, mods: u32) {
        let escape = key == "key.keyboard.escape";
        if self.confirm {
            if escape {
                self.confirm = false;
                self.reinit();
            }
            return;
        }
        // A dropdown's field: Enter and Escape pick the first suggestion.
        if let Some(Focus::Field(i)) = self.focus
            && matches!(self.entries.get(i).map(|e| &e.kind), Some(Kind::Dropdown(_)))
            && (escape || key == "key.keyboard.enter" || key == "key.keyboard.keypad.enter")
        {
            if let Some(first) = self.entries[i].suggestions().into_iter().next() {
                let e = &mut self.entries[i];
                e.field.set_value(&first);
                e.field.set_cursor(0, false);
                self.focus = Some(Focus::Widget(i));
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

    /// Typed text into the focused field (a number field takes digits, `-`
    /// and `.` only, `AbstractNumberListEntry.stripAddText`).
    pub(super) fn typed(&mut self, text: &str) {
        if self.confirm {
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

