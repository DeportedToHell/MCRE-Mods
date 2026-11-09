// Cloth Config for Horizon: the screen AutoConfig builds with Cloth Config
// 26.3 (`ClothConfigScreen`) for Bad Wither No Cookie's settings: the title,
// the list with its search field, the collapsible object's
// `SubCategoryListEntry` and the rows in it (`BooleanListEntry`,
// `StringListListEntry`), Cancel and Save & Quit, and the "Changes Not
// Saved" confirmation, laid out with Cloth Config's own numbers in the game's
// widgets and Cloth Config's texture. Rewritten from Cloth Config by
// shedaniel (LGPL-3.0, https://github.com/shedaniel/cloth-config), after our
// ClientSort port's copy, with the sub-category added.
//
// The mod builds one with `Screen::new(title, entries)`, draws it from
// `screen_render_post`, feeds it the screen hooks' input and takes its
// requests from `frame` (what needs `&mut Game`: the click sound, the
// clipboard, closing, saving).
use super::Game;
use super::bwncr_cloth_sprites::{self as sprites, Sprite};
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::text::Text;
use horizon_client::ui::{self, EditBox};
use std::time::Instant;

type Rect = [f32; 4];

/// GLFW's modifier bits, as the screen hooks pass them.
const SHIFT: u32 = 1;
const CONTROL: u32 = 2;

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

// ---- values and entries ---------------------------------------------------------------------

/// An entry's value as the screen edits it.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Value {
    Bool(bool),
    List(Vec<String>),
    /// A sub-category's (it holds entries, not a value).
    None,
}

impl Value {
    pub(super) fn as_bool(&self) -> bool {
        matches!(self, Value::Bool(true))
    }
    pub(super) fn as_list(&self) -> Vec<String> {
        match self {
            Value::List(l) => l.clone(),
            _ => Vec::new(),
        }
    }
}

/// What kind of row an entry is.
enum Kind {
    Bool,
    /// A list of strings (`StringListListEntry`, AutoConfig's for a
    /// `List<String>`): new cells go last.
    List,
    /// A collapsible object (`SubCategoryListEntry`): its entries under it.
    Sub,
}

/// One row (`AbstractConfigListEntry`).
pub(super) struct Entry {
    pub(super) id: &'static str,
    name: String,
    kind: Kind,
    value: Value,
    original: Value,
    default: Option<Value>,
    /// A list's cells, open or not.
    cells: Vec<EditBox>,
    /// A list or a sub-category is open.
    expanded: bool,
    /// A sub-category's entries.
    children: Vec<Entry>,
}

fn cell_box(text: &str) -> EditBox {
    let mut b = EditBox::new(0.0, 0.0, 100.0, 18.0);
    b.max_length = usize::MAX;
    b.bordered = false;
    b.focused = false;
    b.set_value(text);
    b.set_cursor(0, false);
    b
}

impl Entry {
    fn new(id: &'static str, name: &str, kind: Kind, value: Value, default: Option<Value>) -> Entry {
        let cells = value.as_list().iter().map(|s| cell_box(s)).collect();
        Entry {
            id,
            name: name.to_owned(),
            kind,
            original: value.clone(),
            value,
            default,
            cells,
            expanded: false,
            children: Vec::new(),
        }
    }
    pub(super) fn boolean(id: &'static str, name: &str, value: bool, default: bool) -> Entry {
        Entry::new(id, name, Kind::Bool, Value::Bool(value), Some(Value::Bool(default)))
    }
    /// Closed at first, as `AbstractListBuilder` leaves it.
    pub(super) fn list(id: &'static str, name: &str, value: Vec<String>, default: Vec<String>) -> Entry {
        Entry::new(id, name, Kind::List, Value::List(value), Some(Value::List(default)))
    }
    /// `startSubCategory(name, entries).setExpanded(expanded)`.
    pub(super) fn sub(name: &str, children: Vec<Entry>, expanded: bool) -> Entry {
        let mut e = Entry::new("", name, Kind::Sub, Value::None, None);
        e.children = children;
        e.expanded = expanded;
        e
    }

    /// The value now (`getValue`).
    pub(super) fn current(&self) -> Value {
        match self.kind {
            Kind::List => Value::List(self.cells.iter().map(|c| c.value.clone()).collect()),
            _ => self.value.clone(),
        }
    }

    /// `isEdited`: changed from what the screen opened with; a sub-category
    /// when one of its entries is.
    fn edited(&self) -> bool {
        match self.kind {
            Kind::Sub => self.children.iter().any(Entry::edited),
            _ => self.current() != self.original,
        }
    }

    fn matches_default(&self) -> bool {
        self.default.as_ref().is_some_and(|d| self.current() == *d)
    }

    fn reset(&mut self) {
        let Some(d) = self.default.clone() else {
            return;
        };
        match self.kind {
            Kind::List => self.cells = d.as_list().iter().map(|s| cell_box(s)).collect(),
            _ => self.value = d,
        }
    }

    /// `getSearchTags`: the name's words, and a sub-category's entries' too.
    fn tags(&self, out: &mut Vec<String>) {
        out.extend(self.name.split(' ').map(str::to_lowercase));
        for c in &self.children {
            c.tags(out);
        }
    }

    /// `matchesSearch`: every word of the search is in one of the tags.
    fn matches(&self, words: &[String]) -> bool {
        if words.is_empty() {
            return true;
        }
        let mut tags = Vec::new();
        self.tags(&mut tags);
        words.iter().all(|w| tags.iter().any(|t| t.contains(w.as_str())))
    }

    /// The row's own height (`getItemHeight`, a sub-category's without its
    /// entries, which get rows of their own).
    fn height(&self) -> f32 {
        match self.kind {
            Kind::List if self.expanded => 24.0 + 20.0 * self.cells.len() as f32,
            _ => 24.0,
        }
    }

    /// The displayed name (`getDisplayedFieldName`): italic when edited,
    /// grey otherwise.
    fn displayed_name(&self) -> Text {
        let name = Text::literal(self.name.clone());
        if self.edited() { name.italic() } else { name.with_color(horizon_client::text::GRAY) }
    }
}

// ---- the screen -----------------------------------------------------------------------------

/// An entry: its index in the list, and in its sub-category's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Path(usize, Option<usize>);

/// What has the keyboard: the search field, a list's cell, or another of an
/// entry's widgets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Focus {
    Search,
    Cell(Path, usize),
    Widget(Path),
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
    Entry(Path),
}

#[derive(Clone, Copy, Debug)]
struct Row {
    kind: RowKind,
    /// The row's top on the screen, its height, and its entry's left edge
    /// and width (a sub-category's entries 14 pixels in).
    y: f32,
    h: f32,
    x: f32,
    w: f32,
}

/// Where everything is this frame (`init`'s numbers).
struct Layout {
    w: f32,
    h: f32,
    top: f32,
    bottom: f32,
    /// The list's left edge and width (`getRowLeft`, `getItemWidth`).
    x: f32,
    entry_w: f32,
    rows: Vec<Row>,
    content: f32,
    max_scroll: f32,
    footer_w: f32,
}

pub(super) struct Screen {
    title: String,
    entries: Vec<Entry>,
    search: EditBox,
    focus: Option<Focus>,
    scroll: Anim,
    scrolling: Option<f32>,
    /// The hovered row's highlight (`currentBounds`), in list coordinates,
    /// and when the pointer was last over a row.
    hover: [Anim; 4],
    last_touch: Instant,
    last_frame: Instant,
    /// "Changes Not Saved" is up.
    confirm: bool,
    in_world: bool,
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

    /// `init` again (back from the confirmation): a new list, so its scroll
    /// and search start over.
    fn reinit(&mut self) {
        self.scroll.set(0.0);
        self.search.set_value("");
        self.search.focused = false;
        self.focus = None;
        self.scrolling = None;
    }

    fn entry(&self, p: Path) -> Option<&Entry> {
        let e = self.entries.get(p.0)?;
        match p.1 {
            Some(c) => e.children.get(c),
            None => Some(e),
        }
    }

    fn entry_mut(&mut self, p: Path) -> Option<&mut Entry> {
        let e = self.entries.get_mut(p.0)?;
        match p.1 {
            Some(c) => e.children.get_mut(c),
            None => Some(e),
        }
    }

    /// The entry shows: in a sub-category, only while that is open.
    fn shown(&self, p: Path) -> bool {
        match p.1 {
            Some(_) => self.entries.get(p.0).is_some_and(|e| e.expanded),
            None => p.0 < self.entries.len(),
        }
    }

    /// `isEdited` for the whole screen.
    pub(super) fn edited(&self) -> bool {
        self.entries.iter().any(Entry::edited)
    }

    /// Every entry's id and value, for the mod to save.
    pub(super) fn values(&self) -> Vec<(&'static str, Value)> {
        fn walk(e: &Entry, out: &mut Vec<(&'static str, Value)>) {
            if !e.id.is_empty() {
                out.push((e.id, e.current()));
            }
            for c in &e.children {
                walk(c, out);
            }
        }
        let mut out = Vec::new();
        for e in &self.entries {
            walk(e, &mut out);
        }
        out
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
        let mut push = |rows: &mut Vec<Row>, kind: RowKind, h: f32, x: f32, w: f32| {
            rows.push(Row { kind, y, h, x, w });
            y += h;
        };
        push(&mut rows, RowKind::Gap, 5.0, x, item_w);
        push(&mut rows, RowKind::Search, 24.0, x, item_w);
        push(&mut rows, RowKind::Gap, 5.0, x, item_w);
        let words = self.search_words();
        for (i, e) in self.entries.iter().enumerate() {
            if !e.matches(&words) {
                continue;
            }
            push(&mut rows, RowKind::Entry(Path(i, None)), e.height(), x, item_w);
            // `SubCategoryListEntry.extractRenderState`: the entries that
            // match the search, 24 below its label, 14 in.
            if matches!(e.kind, Kind::Sub) && e.expanded {
                for (j, c) in e.children.iter().enumerate() {
                    if c.matches(&words) {
                        push(&mut rows, RowKind::Entry(Path(i, Some(j))), c.height(), x + 14.0, item_w - 14.0);
                    }
                }
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
        // Scrolling eases toward its target.
        let l = self.layout(w, h);
        self.scroll.target = self.scroll.target.clamp(0.0, l.max_scroll);
        self.scroll.update(dt);
        self.scroll.value = self.scroll.value.clamp(0.0, l.max_scroll);
        let l = self.layout(w, h);
        let cursor = game.cursor;
        let mut tip: Option<Vec<Text>> = None;

        // The list (`DynamicEntryListWidget.extractRenderState`).
        fill(game, batch, [0.0, l.top, w, l.bottom - l.top], 0x4000_0000);
        let start = batch.vertices.len();
        let over_list = cursor.1 >= l.top && cursor.1 <= l.bottom;
        // The hovered row's highlight (`getEntryArea`: the list's width, 20
        // high), moving and fading as Cloth Config's does.
        let scroll = self.scroll.value.floor();
        let mut target = None;
        for row in &l.rows {
            if let RowKind::Entry(_) | RowKind::Search = row.kind {
                let area = [0.0, row.y, w, 20.0];
                if over_list && inside(cursor, area) {
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
                RowKind::Entry(p) => {
                    let row_tip = self.draw_entry(game, batch, &l, row, p, ru, over_list);
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

        // The title.
        let tw = width(game, &self.title);
        batch.text(atlas, &self.title, ((w - tw) / 2.0).floor(), 12.0, u32::MAX, true);

        // Cancel and Save & Quit.
        let edited = self.edited();
        let (cancel, save) = Self::footer(&l);
        let cancel_label = if edited { Text::literal(tr(ru, "discard")) } else { Text::translate("gui.cancel", &[]) };
        button(game, batch, cancel, &cancel_label, true);
        button(game, batch, save, &Text::literal(tr(ru, "save")), edited);

        if let Some(lines) = tip {
            ui::tooltip(batch, atlas, &lines, cursor);
        }
    }

    /// One entry's row; a tooltip when the pointer is over a list's buttons.
    #[allow(clippy::too_many_arguments)]
    fn draw_entry(
        &mut self,
        game: &Game<'_>,
        batch: &mut HudBatch,
        l: &Layout,
        row: &Row,
        p: Path,
        ru: bool,
        over_list: bool,
    ) -> Option<Vec<Text>> {
        let atlas = &game.gui.atlas;
        let cursor = game.cursor;
        let (x, y, ew) = (row.x, row.y, row.w);
        let focus = self.focus;
        let reset_label = Text::literal(tr(ru, "reset"));
        let rw = width(game, tr(ru, "reset")) + 6.0;
        let e = self.entry_mut(p)?;
        let name = e.displayed_name();
        let hovered = over_list && inside(cursor, [0.0, y, l.w, 20.0]);
        let reset_r = [x + ew - rw, y, rw, 20.0];
        let can_reset = e.default.is_some() && !e.matches_default();
        let mut tip = None;
        match e.kind {
            Kind::Bool => {
                batch.text_styled(atlas, &name, x, y + 6.0, u32::MAX, true);
                button(game, batch, reset_r, &reset_label, can_reset);
                let label = Text::legacy(tr(ru, if e.value.as_bool() { "yes" } else { "no" }));
                button(game, batch, [x + ew - 150.0, y, 150.0 - rw - 2.0, 20.0], &label, true);
            }
            Kind::Sub => {
                // The label widget: the arrow, the name yellow under the pointer.
                let in_label = inside(cursor, [x - 15.0, y, ew + 15.0, 24.0]);
                let arrow = match (e.expanded, in_label) {
                    (false, false) => &sprites::CATEGORY,
                    (true, false) => &sprites::CATEGORY_OPEN,
                    (false, true) => &sprites::CATEGORY_HOVER,
                    (true, true) => &sprites::CATEGORY_OPEN_HOVER,
                };
                blit(game, batch, arrow, x - 15.0, y + 5.0);
                let color = if in_label { argb(0xffe6_fe16) } else { u32::MAX };
                batch.text_styled(atlas, &name, x, y + 6.0, color, true);
            }
            Kind::List => {
                let label_r = [x - 15.0, y, ew + 15.0, 24.0];
                let insert = [x - 15.0 + 12.0, y + 3.0, 12.0, 12.0];
                let delete = [x - 15.0 + 25.0, y + 3.0, 12.0, 12.0];
                let in_label = inside(cursor, label_r);
                let in_insert = inside(cursor, insert);
                let in_delete = inside(cursor, delete);
                let focused_cell = match focus {
                    Some(Focus::Cell(fp, c)) if fp == p && e.expanded => Some(c),
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
                let color = if in_label && !on_reset && !in_delete && !in_insert { argb(0xffe6_fe16) } else { u32::MAX };
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
                    tip = Some(tr(ru, "add"));
                } else if hovered && in_delete && focused_cell.is_some() {
                    tip = Some(tr(ru, "remove"));
                }
            }
        }
        // `wrapLinesToScreen`.
        tip.map(|s| wrap(game, s, l.w))
    }

    // ---- input ----------------------------------------------------------------------------

    fn click(&mut self) {
        self.requests.push(Request::Click);
    }

    /// `quit`: "Changes Not Saved" when something changed.
    fn quit(&mut self) {
        if self.edited() {
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
        let (cancel, save) = Self::footer(&l);
        if left && inside(p, cancel) {
            self.click();
            self.quit();
            return;
        }
        if inside(p, save) {
            if left && self.edited() {
                self.click();
                self.requests.push(Request::Close(true));
            }
            return;
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
                RowKind::Entry(path) => {
                    let (x, y, ew) = (row.x, row.y, row.w);
                    let reset_r = [x + ew - rw, y, rw, 20.0];
                    let value_r = [x + ew - 150.0, y, 150.0 - rw - 2.0, 20.0];
                    let focus = self.focus;
                    // The entry through the field, so the focus can change
                    // while a cell is borrowed.
                    let Some(top) = self.entries.get_mut(path.0) else {
                        continue;
                    };
                    let e = match path.1 {
                        Some(c) => match top.children.get_mut(c) {
                            Some(e) => e,
                            None => continue,
                        },
                        None => top,
                    };
                    let can_reset = e.default.is_some() && !e.matches_default();
                    let on_reset = left && can_reset && inside(p, reset_r);
                    match e.kind {
                        Kind::Bool => {
                            if on_reset {
                                e.reset();
                            } else if left && inside(p, value_r) {
                                if let Value::Bool(b) = &mut e.value {
                                    *b = !*b;
                                }
                            } else {
                                continue;
                            }
                            self.focus = Some(Focus::Widget(path));
                            self.click();
                            return;
                        }
                        Kind::Sub => {
                            // `CategoryLabelWidget.mouseClicked`: any button.
                            if inside(p, [x - 15.0, y, ew + 15.0, 24.0]) {
                                e.expanded = !e.expanded;
                                self.focus = Some(Focus::Widget(path));
                                self.click();
                                return;
                            }
                        }
                        Kind::List => {
                            if on_reset {
                                e.reset();
                                self.focus = Some(Focus::Widget(path));
                                self.click();
                                return;
                            }
                            if left && inside(p, [x - 15.0, y, ew + 15.0, 24.0]) && !inside(p, reset_r) {
                                if inside(p, [x - 15.0 + 12.0, y + 3.0, 12.0, 12.0]) {
                                    e.expanded = true;
                                    e.cells.push(cell_box(""));
                                } else if inside(p, [x - 15.0 + 25.0, y + 3.0, 12.0, 12.0]) {
                                    match focus {
                                        Some(Focus::Cell(fp, c)) if fp == path && e.expanded && c < e.cells.len() => {
                                            e.cells.remove(c);
                                        }
                                        _ => return,
                                    }
                                } else {
                                    e.expanded = !e.expanded;
                                }
                                self.focus = Some(Focus::Widget(path));
                                self.click();
                                return;
                            }
                            if e.expanded {
                                for (c, cell) in e.cells.iter_mut().enumerate() {
                                    if inside(p, [cell.x, cell.y - 1.0, cell.width + 12.0, 20.0]) {
                                        cell.click(atlas, p.0, shift, false);
                                        cell.reset_blink();
                                        self.focus = Some(Focus::Cell(path, c));
                                        return;
                                    }
                                }
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

    /// The scroll bar dragged (`mouseDragged`).
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

    /// The wheel: the list 16 pixels a notch (`getScrollStep`).
    pub(super) fn scrolled(&mut self, w: f32, h: f32, p: (f32, f32), delta: f32) {
        if self.confirm || delta == 0.0 {
            return;
        }
        let l = self.layout(w, h);
        if p.1 >= l.top && p.1 <= l.bottom {
            let t = (self.scroll.target - delta * 16.0).clamp(0.0, l.max_scroll);
            self.scroll.set_to(t, 600.0);
        }
    }

    /// The field with the keyboard: the search, or a list's cell while it
    /// shows.
    fn focused_box(&mut self) -> Option<&mut EditBox> {
        match self.focus? {
            Focus::Search => Some(&mut self.search),
            Focus::Cell(p, c) => {
                if !self.shown(p) || !self.entry(p)?.expanded {
                    return None;
                }
                self.entry_mut(p)?.cells.get_mut(c)
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
        let control = mods & CONTROL != 0;
        let shift = mods & SHIFT != 0;
        if !escape && self.focused_box().is_some() {
            let request = match key {
                "key.keyboard.v" if control => Some(Request::Paste),
                "key.keyboard.c" if control => self.focused_box().map(|b| Request::Copy(b.selected_text())),
                "key.keyboard.x" if control => self.focused_box().map(|b| {
                    let t = b.selected_text();
                    b.delete_chars(0);
                    Request::Copy(t)
                }),
                _ if !control && writes(key) => None,
                _ => {
                    if let Some(code) = horizon_client::keybinds::key_code(key)
                        && let Some(b) = self.focused_box()
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

    /// Typed text into the focused field.
    pub(super) fn typed(&mut self, text: &str) {
        if self.confirm {
            return;
        }
        if let Some(b) = self.focused_box() {
            let text: String = text.chars().filter(|c| !c.is_control()).collect();
            if !text.is_empty() {
                b.insert_text(&text);
                b.reset_blink();
            }
        }
    }
}
